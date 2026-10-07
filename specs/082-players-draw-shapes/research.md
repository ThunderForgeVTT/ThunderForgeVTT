# Research: Players Draw Shapes

## R1. Where the player defaults live

**Decision**: in code, as `PLAYER_DEFAULT_TOOLS: [&str; 2] = ["select", "shapes"]`
beside `AUTHORING_TOOLS` in `crates/thunderforge-server/src/auth/authoring_tools.rs`.
`effective_authoring_tools` becomes, for a non-DM member:

```text
(PLAYER_DEFAULT_TOOLS − revocations) ∪ grants, in AUTHORING_TOOLS order
```

A DM still resolves to all six. A non-member resolves to nothing (today a
non-member also resolves through `granted_authoring_tools` to nothing,
because the join finds no membership; the new rule keeps that by asking for
the membership first).

**Why**: the owner chose "on by default" for existing worlds too. A default
in code reaches every world on the next request with no data migration. A
backfill of grant rows would miss every member who joins later.

**Alternatives rejected**:

- Grant rows inserted on join. Needs a backfill, and a hook on every path
  that creates a membership (invite, claim, demo seed, test support).
- A `granted BOOLEAN` column on the grants table. The grants module already
  explains why "not held" must have one spelling; a false row for a default
  tool would be a second one.

## R2. Revocations in their own table

**Decision**: `world_authoring_tool_revocations`, one row per
`(world_member_id, tool)`, cascading from `world_members`, with a check that
`tool IN ('select', 'shapes')` (data-model.md).

**Why**: a revocation is the opposite fact of a grant and lives in the
opposite direction from the default. Keeping it in its own table keeps both
tables single-meaning: a grant row always widens, a revocation row always
narrows. The cascade gives "a player who rejoins starts at the defaults"
for free, as the grants already do.

`setAuthoringToolGrant` keeps its signature (FR-003). For a default tool,
`granted: false` upserts a revocation and `granted: true` deletes it; for
the other four it writes grants as today. A stray grant row for a default
tool (none exists today, since players held nothing) is deleted by the
migration so the rule "a grant never names a default tool" holds from the
first request.

## R3. One ownership check, synchronous

**Decision**: a new `crates/thunderforge-server/src/auth/shape_authority.rs`:

```rust
pub enum ShapeAuthority { Dm, Creator, None }

pub fn shape_authority(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    created_by: Option<Uuid>, // None: creating
) -> QueryResult<ShapeAuthority>
```

It answers `Dm` when `is_dm_of_scene` does. Otherwise it reads the member's
effective tools through a synchronous `effective_tools_on(conn, …)` (the
async `effective_authoring_tools` becomes a wrapper around it), and answers
`Creator` when `shapes` is held and `created_by` is `None` or the caller.
Anything else is `None`.

**Why synchronous**: every shape mutation runs inside `spawn_blocking`
holding a `PgConnection`, exactly as `is_dm_of_scene` is synchronous for.
One check, used by `createShape`, `updateShape`, `deleteShape` and
`clearShapes` (FR-006), and one resolver for tools that the GraphQL query
and the shape mutations share.

**Refusals stay what they are today** (spec correction, see below):
`updateShape` answers its existing "not found or not owned by you" error,
`deleteShape` answers `false`, `createShape` its existing "scene not found
or not owned by you" error. A player cannot tell a shape they may not touch
from one that does not exist.

## R4. Forced visibility

**Decision**: when the authority is `Creator`, `createShape` stores
`visible_to_players = true` whatever the input says, and `updateShape`
drops a `visibleToPlayers` field from the update (it cannot set it to
false, and setting it to true is already the stored value).

## R5. The bulk clear

**Decision**: `clearShapes(sceneId, createdBy)` deletes with one
`DELETE … WHERE scene_id = $1 [AND created_by = ANY($2)] RETURNING shape_id`
inside a transaction, then records one `EVENT_CODE_SHAPE_CHANGED` event per
returned id with `action: "deleted"` in the same transaction. `createdBy`
present and empty deletes nothing and records nothing.

**Why per-shape events**: clients already handle `deleted` (spec FR-009).
`record_world_event` issues one insert and one `NOTIFY` per event; at 200
shapes that is 200 small inserts in one transaction, well inside SC-003's
2 s on a local stack. The task list measures it (T042) rather than assuming.

**The client's half**: a new store command `clear_shapes { sceneId,
createdBy? }`. The shape mutation bridge sends `clearShapes` and does not
remove anything locally; the shapes leave when the events arrive
(FR-014). No optimistic removal, so nothing needs rolling back.

## R6. `shapeCreators` and the picker

**Decision**: `shapeCreators(sceneId)` groups `shapes` by `created_by` on
the scene, joins `users` for the display name, left-joins `world_members`
for "still a member", and drops any user who is a member of the world with
the Owner or GM role today. A site admin who drew without such a membership
is listed like a player. A non-DM caller gets `NotFound`.

The picker is a list of checkboxes in a confirmation dialog, using the
existing `Dialog` wrapper. Names come from `users.username`, as the grants
card shows them.

## R7. The engine learns who it is

**Decision**: a new external command, `SetViewerUser { user_id: Option<String> }`,
sent like `SetIsGameMaster` (`set_viewer_user` in `sdk.rs`, `setViewerUser`
in `apps/web/src/engine/bevy/index.ts`), re-sent whenever the engine
becomes ready. It fills a `ViewerUserId(Option<String>)` resource. The
engine's `Shape` (canvas-core) gains `created_by: Option<String>`, filled
from `WorldShapePayload.createdBy`.

The engine's rule, as a pure function in `systems/shape.rs` so a host test
reaches it:

```rust
fn may_edit_shape(is_gm: bool, viewer: Option<&str>, shape: &Shape) -> bool {
    is_gm || matches!((viewer, shape.created_by.as_deref()), (Some(v), Some(c)) if v == c)
}
```

The five `IsGameMaster` gates in `systems/shape.rs` and the one in
`plugins/context_menu.rs` become `tool_is_allowed(AuthoringMode::Shapes)`
(or `Select`) plus `may_edit_shape` for anything that touches an existing
shape. Hit-testing skips shapes the viewer may not edit.

## R8. The rail for a player

**Decision**: `WorldPage.tsx` renders `GmToolRail` when `sceneId` is set and
`permittedTools(…, allowedTools)` is non-empty, not only for
`isSceneOwner`. `permittedTools` with an unresolved (`null`) answer shows
everything, which for a player would flash the GM's six tools, so the
player's rail waits for a resolved answer: unresolved shows nothing to a
non-owner (a new `railTools(tools, allowed, isOwner)` in
`apps/web/src/lib/authoringTools.ts`, unit-tested).

The Escape-returns-to-Select shortcut (`WorldPage.tsx` around line 993)
applies to anyone with a rail. `AssetPasteTool` stays GM-only.

The shapes panel (`ShapeTool.tsx`) receives `isGm`. A player's panel hides
the "visible to players" toggle (FR-015) and both clear actions (FR-013).

## R9. Account deletion and a player's shapes

**Today**: `shapes.created_by` and `shapes.updated_by` reference `users(id)`
with no `ON DELETE` action. `delete_user_data_on`
(`crates/thunderforge-server/src/users/mod.rs`) deletes the account's own
worlds, then the user row. A shape the user drew or edited in a world they
did not create still references them, so the final `DELETE FROM users`
fails with a foreign key violation and the whole deletion rolls back. Until
now only GMs drew, so only a GM of someone else's world could hit it. Once
players draw by default, every player who ever drew would be unable to
delete their account.

**Decision**: inside `delete_user_data_on`'s transaction, before the user
row goes:

1. Delete the shapes the user created in worlds that remain, recording one
   `deleted` shape event per shape, attributed to the world's owner
   (`worlds.created_by`), so live boards drop them. A player's drawing is
   a scribble on someone else's map; it does not outlive the player, as
   their chat does not need to.
2. For shapes the user only edited (`updated_by` = user, `created_by` ≠
   user), set `updated_by = created_by`.

`UserDataDeleteSummary` gains `shapes_deleted`.

**Not in scope, flagged**: `walls.created_by` and `light_sources.created_by`
have the same constraint. A GM of someone else's world who drew a wall
cannot delete their account today. That is a separate defect for its own
change, since walls are not a player's to draw.

## R10. The demo

**Decision**: the demo's viewer is per tab (`viewerUser`, `viewerIsGm` in
`apps/demo/src/backend/actors.ts`).

- `authoringTools` answers all six for the GM and `["select", "shapes"]`
  for the player.
- `authoringToolGrants` answers the player's effective tools.
- `createShape` stamps `createdBy`/`updatedBy` with `viewerUser(state).id`
  instead of the fixed `by`, and forces `visibleToPlayers: true` for the
  player.
- `updateShape`/`deleteShape` refuse a player's write to a shape they did
  not create, the same way the server does.
- `clearShapes` and `shapeCreators` follow R5 and R6.
- The demo has no revocations: `setAuthoringToolGrant` stays unanswered
  there, as today.

The handlers move to `apps/demo/src/backend/handlers/shapes.ts` with a
`shapes.test.ts`, following `dice.ts`.

## R11. Spec corrections made while planning

- **FR-005 / US2 AS3**: `deleteShape` refuses with `false` today, not
  `NotFound`; `updateShape` and `createShape` refuse with their existing
  errors. The spec now says "the same refusal it gives today for a shape
  that does not exist".
- **FR-003**: `authoringToolGrants` lists only grant rows, and the card
  reads an absent member as "nothing". It must now answer each non-DM
  member's effective tools, so the card shows Select and Shapes ticked
  for a member with no rows.
- **FR-014**: `ShapeTool.tsx` creates a text shape by calling
  `createShape` itself. That is the anti-pattern AGENTS.md names, and it
  would bypass the store for a player. Text creation moves to the
  `create_shape` store command.
- **Rollback**: the shape mutation bridge logs a refused update or delete
  and does nothing else, so a refused move stays on the mover's board. It
  now restores the shape from the server on a refusal (AGENTS.md §4).
