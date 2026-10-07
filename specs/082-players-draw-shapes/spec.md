# Feature Specification: Players Draw Shapes

**Feature Branch**: `082-players-draw-shapes`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "players should have access to the shapes tool and select tool the gm should have a button that is clean up all shapes or cleanup a specific players shapes letting them select players shapes to clean up." Then, choosing between options: "On by default, own shapes". Every player can draw, select, move and delete their own shapes, never anyone else's. The GM can still take the tools away from a player through the existing tool grants. The GM gets "Clear all shapes" and "Clear a player's shapes…".

## Why

At a table, players draw on the map. They sketch the route they mean to
take, circle the door they want to listen at, or mark where the fireball
lands. Today only the Game Master can draw. A player who wants to point at
something has to describe it in chat, and the GM has to draw it for them.

Spec 031 made the tools a permission the GM hands out. No GM hands out the
shapes tool before the first session, though, and a player does not know
to ask for it. So the two tools a player needs to point at the map, select
and shapes, are now on by default. A player owns what they draw and cannot
touch anyone else's drawing, the GM's included.

Drawing is cheap, so the board fills up. The GM needs a way to wipe it,
either all at once between encounters or one player's scribbles at a time.

## What exists

Counted on 2026-10-07:

- **Tools as a permission** (spec 031).
  - `AUTHORING_TOOLS` and `effective_authoring_tools` live in
    `crates/thunderforge-server/src/auth/authoring_tools.rs`.
  - A DM holds all six tools. Anyone else holds only their
    `world_authoring_tool_grants` rows. A world with no rows gives a player
    no tools at all, which is FR-045 of spec 031.
  - The GM sets the rows from `AuthoringToolGrantsCard.tsx` through
    `setAuthoringToolGrant(worldId, worldMemberId, tool, granted)` in
    `mutations_authoring_tools.rs`.
  - The rows hang off the membership, so removing a member removes them.
- **The rail.**
  - `WorldPage.tsx` renders `GmToolRail` only when `isSceneOwner && sceneId`
    (around line 3064). It filters the tools through `permittedTools`.
  - A player holding a grant therefore still gets no rail, so spec 031's
    grants reach the engine but have no button.
- **The engine.**
  - The shape systems in `crates/thunderforge-engine/src/systems/shape.rs`
    gate on `IsGameMaster` (lines 251, 285, 456, 536 and 641), and so does
    the shape context menu.
  - `plugins/authoring_mode.rs` holds the per-viewer allow list
    (`tool_is_allowed`). The shape systems do not consult it yet.
  - The core `Shape` (`crates/thunderforge-canvas-core/src/shape.rs:50`)
    has no creator.
- **The server.**
  - `createShape`, `updateShape` and `deleteShape` (`mutations_shapes.rs`)
    each require `is_dm_of_scene`. A refused caller gets `NotFound`.
  - `shapes.created_by` (not null, references `users`) and `updated_by`
    are already written, and GraphQL exposes them. The web carries
    `createdBy` in `types/shape.ts` and `api/shapes.ts`.
  - Each write records `EVENT_CODE_SHAPE_CHANGED` (12) with
    `{action, shape_id, scene_id}`.
  - The `shapes` query (`queries/scene.rs`) shows a player only the shapes
    with `visible_to_players`.
  - There is no bulk delete.
- **The pause gate.** `play_pause_surface_tables.rs` classifies
  `createShape`, `updateShape` and `deleteShape` as gated.
- **The demo.** `apps/demo/src/backend/handlers.ts` answers `createShape`,
  `updateShape` and `deleteShape` with no permission check at all.

## Ownership

A shape belongs to whoever drew it (`created_by`).

| Who                         | Draws | Selects, moves, edits, deletes | Clears in bulk |
| --------------------------- | ----- | ------------------------------ | -------------- |
| **The GM** (owner or GM)    | yes   | every shape on the scene       | yes            |
| **A player with the tools** | yes   | their own shapes only          | no             |
| **A player without them**   | no    | nothing                        | no             |

A player's shape is always visible to the whole table. Pointing at the map
is the reason a player draws, so a drawing only they could see would serve
no purpose. The GM's shapes keep `visible_to_players` as they do today.

## User Scenarios & Testing

### User Story 1 - A player draws on the map (Priority: P1)

A player joins a world and opens the play view. Without the GM granting
anything, a rail shows Select and Shapes. The player draws a rectangle
around a door. The GM and every other player see it.

**Why this priority**: It is the reason for the spec.

**Independent Test**: A GM and a player in two browser contexts, in a
world with no grant rows. The player draws a rect, and it appears on both
boards and in the server's `shapes` answer with the player as `createdBy`.

**Acceptance Scenarios**:

1. **Given** a world where the GM has granted nothing,
   **When** a player opens the play view,
   **Then** the rail shows Select and Shapes and no other tools.
2. **Given** that player,
   **When** they draw a shape,
   **Then** the server records it with `created_by` set to them and
   `visible_to_players` true, and it appears on every member's board.
3. **Given** a player who asks for `visibleToPlayers: false`,
   **When** they create or update a shape,
   **Then** the server stores `true` anyway.
4. **Given** a paused scene,
   **When** a player draws,
   **Then** the server refuses it as it refuses the GM's.

### User Story 2 - A player cannot touch anyone else's shape (Priority: P1)

A player can select, move, restyle and delete what they drew, and nothing
else. The GM's shapes and the other players' shapes do not respond to the
player's pointer or keys.

**Why this priority**: Without it, US1 lets any player erase the GM's map
notes.

**Independent Test**: The GM draws one shape, player A draws one, player B
draws one. Player A can move and delete their own shape. Player A's
`updateShape` and `deleteShape` calls on the other two are refused by the
server, whatever the engine allowed.

**Acceptance Scenarios**:

1. **Given** player A's shape,
   **When** player A drags it, recolours it or deletes it,
   **Then** the server accepts it and every board follows.
2. **Given** the GM's shape or player B's,
   **When** player A clicks it,
   **Then** the engine does not select it.
3. **Given** player A calling `updateShape` or `deleteShape` on a shape
   they did not draw, straight at the API,
   **When** the server answers,
   **Then** it refuses with the same `NotFound` refusal a non-member gets,
   and nothing changes.
4. **Given** the GM,
   **When** they select, move or delete any shape on the scene,
   **Then** it works as it does today.

### User Story 3 - The GM clears the board (Priority: P1)

Between encounters, the GM wipes the scene's shapes with one button, after
a confirmation.

**Why this priority**: Players drawing by default fills the board, and
deleting shapes one at a time does not keep up.

**Independent Test**: The GM and two players each draw shapes. The GM
chooses "Clear all shapes" and confirms. Every board is empty of shapes,
and the server's `shapes` answer for the scene is empty.

**Acceptance Scenarios**:

1. **Given** shapes from the GM and from players on the current scene,
   **When** the GM chooses "Clear all shapes" and confirms,
   **Then** every shape on that scene, on every level, is deleted, and
   every board removes them.
2. **Given** the same,
   **When** the GM cancels the confirmation,
   **Then** nothing is deleted.
3. **Given** shapes on another scene,
   **When** the GM clears the current scene,
   **Then** the other scene's shapes stay.
4. **Given** a player,
   **When** they look at the shapes tool,
   **Then** neither clear action is offered, and calling the mutation
   directly is refused.

### User Story 4 - The GM clears one player's shapes (Priority: P2)

One player has drawn all over the map. The GM picks that player, or
several, and clears only their shapes.

**Why this priority**: Useful, but "Clear all" covers the common case.

**Independent Test**: The GM, player A and player B each draw. The GM
chooses "Clear a player's shapes…", picks player A and confirms. Player A's
shapes are gone, and the GM's and player B's remain.

**Acceptance Scenarios**:

1. **Given** the GM opens "Clear a player's shapes…",
   **When** the picker shows,
   **Then** it lists each player who has at least one shape on the
   current scene, with how many, and the GM is not in the list.
2. **Given** the GM picks one or more players and confirms,
   **When** the clear runs,
   **Then** exactly those players' shapes on the current scene are deleted.
3. **Given** a player who left the world but still has shapes on the
   scene,
   **When** the picker shows,
   **Then** that player is listed by name and can be cleared.
4. **Given** no player has shapes on the scene,
   **When** the GM looks at the shapes tool,
   **Then** "Clear a player's shapes…" is disabled with a short reason.

### User Story 5 - The GM takes the tools away from a player (Priority: P2)

A player is misusing the shapes tool. In the world's settings, the GM
unticks Shapes, or Select, for that player. The tool leaves that player's
rail, and their engine and the server stop accepting it. Ticking it again
gives it back.

**Why this priority**: The owner asked to keep this control. It reuses the
card that spec 031 built.

**Independent Test**: The GM unticks Shapes for a player. The player's
rail drops Shapes within one refetch, and the player's `createShape` is
refused. The GM ticks it again, and the player can draw.

**Acceptance Scenarios**:

1. **Given** the grants card,
   **When** it lists a player,
   **Then** Select and Shapes show as ticked unless the GM has taken them
   away, and the other four show as unticked unless granted.
2. **Given** the GM unticks Shapes for a player,
   **When** the player next draws,
   **Then** the rail no longer offers Shapes, the engine refuses the mode,
   and the server refuses `createShape`, `updateShape` and `deleteShape`
   from that player.
3. **Given** a player whose Shapes was taken away,
   **When** the GM ticks it again,
   **Then** the player can draw and edit their own shapes again. The
   shapes they drew before were never touched.

### User Story 6 - The demo does the same (Priority: P3)

In the demo, **View as player** gives the Select and Shapes tools. The
player edits only their own shapes, and the GM's view has both clear
actions.

**Why this priority**: The demo mirrors the server's rules (spec 081 set
the precedent), but nobody's data is at stake in it.

**Independent Test**: In the demo, as a player, draw a shape and try to
delete one of the GM's shapes. Only the player's own is affected. As the
GM, "Clear a player's shapes…" removes the player's shape.

## Edge Cases

- **"A player's shapes" never includes the GM's.** The picker lists
  players only. The server applies the creator filter literally, so a GM's
  shape is never cleared by naming a player. A GM's id passed to the
  mutation is cleared only if the GM named it explicitly, which the UI
  never does. An owner who is also listed as a player is still treated as
  the GM.
- **A player leaves the world, or is removed.** Their shapes stay on the
  board, still created by them. Nobody but the GM can edit them now, since
  the player is no longer a member. The picker still lists them by name.
  Their grant and revocation rows go with the membership, so a player who
  rejoins starts at the defaults again.
- **A player is promoted to GM.** They hold every tool and may edit every
  shape. Their old shapes are simply shapes on the scene.
- **A GM is demoted to player.** The shapes they drew stay theirs and stay
  editable by them, and their visibility is left as it was. Any shape they
  edit from then on becomes visible to players (FR-004).
- **Two people edit one shape.** A player and the GM move the same shape
  at once, and the last write wins, as it does for the GM today.
- **Undo.** A player's undo stack holds only their own edits, so undo never
  re-issues a mutation on a shape they do not own. If the GM has deleted
  the shape since, the undo is refused like any other write to a missing
  shape.
- **Clearing during a pause.** "Clear all shapes" and "Clear a player's
  shapes…" are gated by the pause, like every other shape write.
- **A clear while someone is drawing.** A shape created after the clear's
  transaction is not cleared. The clear deletes what exists when it runs.
- **Levels.** A clear covers every level of the current scene, because the
  GM is clearing "the board", not one floor of it.
- **A very large clear.** Hundreds of shapes clear in one transaction and
  one event per shape. If that proves slow on a real table, a batched event
  is a later change; see Assumptions.

## Requirements

### Functional Requirements

**Tools by default**

- **FR-001**: `effective_authoring_tools` MUST resolve a non-DM member to
  the **player defaults**, minus anything the GM has taken away, plus
  anything the GM has granted. The player defaults are `select` and
  `shapes`, declared once beside `AUTHORING_TOOLS`. A DM still resolves to
  all six. This supersedes spec 031 FR-045's empty default.
- **FR-002**: A tool is taken away by a row in a new table,
  `world_authoring_tool_revocations`. It is keyed on the membership like
  the grants, with `ON DELETE CASCADE`. Only a tool in the player defaults
  can be revoked. A grant row for a default tool is meaningless and MUST
  NOT be written.
- **FR-003**: `setAuthoringToolGrant` keeps its signature. For a default
  tool, `granted: false` writes a revocation and `granted: true` deletes
  it. For any other tool it behaves as it does today. The card shows the
  effective answer for each player. Its intro text says that players can
  select and draw by default.

**Ownership on the server**

- **FR-004**: `createShape` MUST accept a caller who is the scene's DM, or
  a member of the scene's world whose effective tools include `shapes`. A
  shape created by a non-DM is stored with `visible_to_players = true`,
  whatever the input asked for.
- **FR-005**: `updateShape` and `deleteShape` MUST accept a DM of the
  scene, or a non-DM member who holds `shapes` and whose id is the shape's
  `created_by`. Anyone else gets the existing `NotFound` refusal. A non-DM
  update cannot set `visible_to_players` to false.
- **FR-006**: The ownership check MUST live in one function beside
  `is_dm_of_scene`, used by all three mutations and the bulk clear. No
  mutation may restate it.
- **FR-007**: Every shape write MUST still record `EVENT_CODE_SHAPE_CHANGED`
  with the same payload, so every client's existing shape event sync keeps
  working unchanged.

**The bulk clear**

- **FR-008**: A new mutation,
  `clearShapes(sceneId: UUID!, createdBy: [UUID!]): Int!`, MUST:
  - be accepted from a DM of the scene only, and refused with `NotFound`
    otherwise;
  - delete every shape on the scene, on every level, when `createdBy` is
    absent;
  - delete only the shapes whose `created_by` is in the list when it is
    given, and nothing when the list is empty;
  - run in one transaction;
  - record one `EVENT_CODE_SHAPE_CHANGED` event per deleted shape, with
    `action: "deleted"`, inside that transaction;
  - answer the number of shapes deleted.
- **FR-009**: One event per shape is chosen over one batch event. Clients
  already apply `deleted` for a single shape, so the clear needs no new
  client handling and no new event code, and catch-up replays it like any
  other delete.
- **FR-010**: `clearShapes` MUST be listed in the GATED table of
  `play_pause_surface_tables.rs`, so the pause surface test covers it. The
  test fails on any mutation that is in no table.
- **FR-011**: A query, `shapeCreators(sceneId: UUID!)`, answers a DM with
  each creator of a shape on the scene: user id, display name, whether
  they are still a member, and how many shapes they have there. It excludes
  DMs of the world. A non-DM gets `NotFound`. The picker in US4 reads this.

**The web app**

- **FR-012**: The rail MUST render for any member whose effective tools
  are not empty, not only for the scene's owner. It offers exactly those
  tools (`permittedTools`), as it already does for the GM.
- **FR-013**: The shapes tool's panel MUST show "Clear all shapes" and
  "Clear a player's shapes…" to a DM only. Each opens a confirmation that
  names what will go. For the second, the confirmation first shows a
  multi-select of the players `shapeCreators` returned.
- **FR-014**: The web dispatches into the world store and lets the
  mutation bridge send the mutations, per AGENTS.md. No component calls
  `clearShapes` directly. Shapes leave the boards when the server's events
  arrive, not before.
- **FR-015**: A player's shape panel MUST NOT offer the GM-only/visible
  toggle.

**The engine**

- **FR-016**: The core `Shape` MUST carry `created_by`, filled from the
  server's `createdBy` on every upsert.
- **FR-017**: The engine MUST know the viewer's user id, passed in the same
  way as the viewer's token, and never synced.
- **FR-018**: The shape systems and the shape context menu MUST gate on
  `tool_is_allowed(AuthoringMode::Shapes)`, or `Select` for selecting,
  instead of `IsGameMaster`. Selection, dragging, restyling and deleting
  MUST additionally require that the viewer is a GM or the shape's
  `created_by`. A shape the viewer may not edit is not hit-tested for
  selection at all.
- **FR-019**: The engine's checks are a courtesy. The server's (FR-004 to
  FR-006) are the rule.

**The demo**

- **FR-020**: The demo's `createShape`, `updateShape` and `deleteShape`
  MUST apply FR-004 and FR-005 for the tab's current viewer. It MUST
  implement `clearShapes` and `shapeCreators` with FR-008 and FR-011's
  rules. Demo unit tests MUST cover the same cases as the server tests.

### Key Entities

- **Player defaults**: the constant list `["select", "shapes"]` of tools a
  non-DM member holds unless revoked.
- **Tool revocation** (`world_authoring_tool_revocations`), new:
  - `world_member_id` (references `world_members`, cascades);
  - `tool` (one of the player defaults);
  - `revoked_by` and `revoked_at`;
  - a unique constraint on `(world_member_id, tool)`.
- **Shape** (`shapes`), existing: unchanged in the database. The engine's
  core `Shape` gains `created_by`.
- **Shape creator**: the read model behind the picker, made of user id,
  display name, whether they are still a member, and a shape count.

## Success Criteria

### Measurable Outcomes

- **SC-001**: In a world with no grant rows, a newly joined player can
  draw a shape within one click of reaching the play view, and every other
  member sees it within 1 s on a local stack.
- **SC-002**: Across the e2e run, every `updateShape`, `deleteShape` and
  `clearShapes` a player sends against a shape they do not own is refused,
  and the server's shape rows are unchanged afterwards.
- **SC-003**: "Clear all shapes" on a scene of 200 shapes empties every
  member's board within 2 s on a local stack.
- **SC-004**: "Clear a player's shapes…" leaves every shape not drawn by
  the chosen players byte-for-byte unchanged.
- **SC-005**: A revoked tool disappears from the player's rail without a
  reload, and the server refuses it at once.

### Proof

- **Server tests** cover:
  - `effective_authoring_tools` for a DM, a default player, a revoked
    player and a granted player;
  - the revocation cascade on member removal;
  - each shape mutation as the DM, the creator, another player, a revoked
    creator and a non-member;
  - forced visibility for a player's shape;
  - `clearShapes` with no filter, with a filter, with an empty list, as a
    player and on another scene;
  - its events, one per shape with `deleted`;
  - `shapeCreators` excluding DMs and including former members;
  - the pause surface test with `clearShapes` in GATED.
- **Engine tests** cover:
  - a viewer who is not a GM cannot select another creator's shape;
  - a viewer without `shapes` in the allow list cannot enter the mode.
- **Demo tests** mirror the server's ownership and clear cases.
- **An e2e spec, `apps/web/e2e/canvas-shapes-by-players.spec.ts`**, in the
  existing `canvas` slice (`pnpm e2e:canvas`), covers US1 to US5 with a GM
  and two players in three browser contexts. That includes the direct-API
  refusals for SC-002. The `canvas-` prefix puts it in the slice's `own`
  list. The slice already owns the shape and authoring-tool sources this
  spec touches.
- **The demo's e2e** covers US6.

## Assumptions

- **Defaults apply to existing worlds.** A world made before this spec
  gives its players Select and Shapes from the next request. That is what
  the owner chose, and no migration rows are needed for it, because the
  default lives in code.
- **The other four tools stay opt-in.** Walls, lights, tokens and
  interactions remain grant-only. This spec adds no default for them.
- **Select means shapes only, for a player.** The Select tool gives a
  player no new reach over walls, lights or tokens. Token dragging keeps
  its own rules (spec 045). Selecting walls or lights still needs the DM
  role or the matching grant.
- **Account deletion.** `shapes.created_by` references `users`. Account
  deletion (`auth/account_ownership.rs`) MUST delete or reassign a
  deleted player's shapes in worlds it does not delete. Planning checks
  what it does today and closes the gap if there is one.
- **Batch event later, if needed.** One event per shape is the choice
  (FR-009). If a clear of many shapes is measurably slow, a batched
  `deleted_many` action is a later change.
- **No per-player colour.** A player's shapes use the palette the GM's do.
  Telling drawings apart by player is out of scope. The picker in US4
  reports who drew what.
