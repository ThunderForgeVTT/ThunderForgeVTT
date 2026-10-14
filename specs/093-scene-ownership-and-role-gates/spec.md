# Feature Specification: Scene ownership and the role gates

**Feature Branch**: `093-scene-ownership-and-role-gates`
**Created**: 2026-10-13
**Status**: Draft
**Input**: Close MVP Phase 10 (Permissions model). Two gaps are named there:
no world-wide role between Player and Game Master that can run a scene, move
any token and open doors without being able to change membership or delete
the world; and no ownership block for scenes and maps, where actors, lore,
items and abilities already have one. Also: decide whether the
`policies`/`permission_grants` tables are really dead, and remove them if so.
The MVP re-verification of 2026-10-13 corrected the first gap: the role
exists (ADR-099); what is missing is the gates. It also confirmed that
`createWorld` inserts no Owner row for the creator. This spec is scoped to
what is still missing: scene ownership, the Trusted Player's table gates,
the creator's membership row, and the dead table.

## Why

A table does not always have one person running it. A long-running group has
a player who runs the fight while the GM handles the story, a co-GM who takes
the second half of the evening, or a player who built the dungeon and should
be the one to open its doors. Today that person is either a Player, who cannot
move another player's token or launch the next scene, or a Game Master, who
can also remove members and change the world's game system. There is nothing
between, so a GM who wants help has to over-trust.

The same group also has scenes that belong to somebody. A player who drew the
tavern should be able to keep editing it without becoming a GM, and a GM should
be able to let one player see the hidden scene for their solo session without
unhiding it for everybody. Actors, lore, items and abilities each have a
per-member Viewer/Editor/Owner block for exactly this (spec 010 FR-013 to
FR-019, ADR-050). Scenes have `hidden` and nothing else.

## What exists

Read on 2026-10-13 against `main` at `da60b0fa`.

**The fourth role is already there.** MVP.md line 386 says there is "no
world-wide 'trusted player' or 'assistant DM' role beyond the fixed three".
That is stale: ADR-099 (`docs/adrs/20260913-099-the_trusted_player_role.md`,
accepted 2026-09-13) added **Trusted Player**, stored as `TrustedPlayer`,
ranked `Player < TrustedPlayer < GM < Owner`. Migration
`2026-09-13-110000-0000_trusted_player_role` widened the
`world_members.role` CHECK to the four values. A Trusted Player is a "co-DM
for content": they may switch books on and off, change world entries and
adopt staged content (`manages_content()`), and are otherwise shown what a
Player is shown. ADR-099 says in so many words that a Trusted Player "may not
do what makes a Game Master a Game Master ... scenes, walls, lighting,
membership and the rest", and that "any exception belongs in a spec, not in a
component". This is that spec.

**How roles are decided.** `crates/thunderforge-authz/src/role.rs` has
`Role {Player, TrustedPlayer, GameMaster, Owner}` with two predicates:
`runs_the_world()` (`>= GameMaster`) and `manages_content()`
(`>= TrustedPlayer`). `capability.rs` maps eight capabilities onto them:
`ViewWorld` (everyone), `ManageBookMaterial` (manages_content), `EditContent`,
`ChangeFogOfWar`, `SeeHiddenContent`, `ManageMembers` (runs_the_world),
`DeleteWorld`, `TransferOwnership` (Owner). The web app mirrors this in
`apps/web/src/types/world.ts` (`WORLD_ROLES`, `runsTheWorld`,
`managesContent`, the label "Trusted Player") and
`apps/web/src/hooks/useWorldRole.ts` (`isGm`, `managesContent`). Roles travel
over GraphQL as `role: String!` on `GraphQLWorldMember` (schema.graphql
line 5722) and `UpdateMemberRoleInput` (line 7780), not as an enum.

**How mutations are gated.** `crates/thunderforge-server/src/auth/world_membership.rs`
offers `require_world_member`, `actor_in_world`, `is_dm_of_world`,
`is_dm_of_scene` (line 368: the scene's world role, "never a Player"),
`is_owner_of_world`, `require_dm_of_world`, `require_world_owner` and
`require_manages_content`. Scene content (walls, lights, tokens, levels,
interactives, party arrival, fog mask, `updateScene`, `deleteScene`) gates on
`is_dm_of_scene`; scene state (`updateSceneHidden`, `updateSceneAmbientLight`,
`launchScene`, exploration) on `is_dm_of_world`; combat, conditions, roll
reveal, world settings and GM-only chat on `is_dm_of_world`; `renameWorld`
(`mutations_worlds.rs:828`) and `deleteWorld` (line 886) on the world's
creator; membership on `require_world_member` plus `Role::can_assign`;
shapes on `shape_authority` (a player may edit their own). Token movement by
a player goes through `combat::controllers::may_move` (token owner, actor
Owner grant, or runs the world).

**Per-object blocks.** `auth/permissioned_entities.rs` generates
`effective_<kind>_permission` / `require_<kind>_permission` and
`purge_member_grants` for actor, lore, item and ability from one declaration
(ADR-050). Each kind has a `world_<kind>_permissions` table
(`id, <kind>_id, user_id, level CHECK IN ('Viewer','Editor','Owner'),
created_at, updated_at, UNIQUE(<kind>_id, user_id)`; actors also carry
`granted_by_claim` from spec 063). The GM is implicitly Owner on everything;
an un-granted member is Viewer. GraphQL exposes
`set<Kind>Permission` / `remove<Kind>Permission` and a
`GraphQL<Kind>Permission` type (schema lines 6907 to 6943).

**Scenes.** `scenes` (schema.rs line 805) carries `owner_id` (set to the
world's creator at `createScene`, `mutations_scenes.rs:280`, and never read
for authorization), `hidden`, `created_by`, `updated_by`. The world's
`active_scene_id` (line 1849) names the launched scene.
`thunderforge_authz::scene::scene_visible(runs_the_world, scene)` is
`runs || !hidden || is_world_active_scene`: a hidden scene is invisible to
every Player except when it is the active one. There is no `maps` table. A
"map" in this product is a scene's background (`background_asset_id`,
`background_image_path`) and its levels (`scene_levels`, spec 075); it has no
identity of its own to own.

**The web surfaces a player sees.** `apps/web/src/components/world/SceneSwitcher/SceneSwitcher.tsx`
lists the scenes the server returned; `apps/web/src/components/world/PlayDock/WorldDock.tsx`
and its panels show the scene the player is on. Both already hide
GM-only controls through `useWorldRole().isGm`.

**The creator has no membership row.** `create_world_impl`
(`mutations_worlds.rs:143`) and `create_world_with_map_impl` (line 115)
insert the world and its Starting Scene and nothing into `world_members`.
`require_world_member` (`world_membership.rs:51`) documents the gap and
falls back to `worlds.created_by`, answering `"Owner"`. Every gate works
because of that fallback, but the members list, `purge_member_grants`, the
authoring-tool grants keyed on `world_member_id` (spec 031) and anything that
joins on `world_members` see a world with no Owner in it.
`docs/SECURITY_RBAC.md` has recorded this as a known gap since 2026-08-20.

**Dead tables.** `policies` exists (migration
`2026-05-02-022200-0000_create_policies`, later given `created_by`,
`updated_by`, `world_id`) and is in `schema.rs` line 731. Its only live
readers are `admin.rs:126`, which counts its rows for the admin dashboard,
and the user data export, which reports `policies: Int!` (always 0) and
`policies: [GraphQLPolicy!]!` (always empty, `mutations_user_data.rs:68`).
Every other reference is commented out (`models.rs:518`,
`graphql/helpers.rs:303`, `queries/user.rs:226`, `users/mod.rs:482`).
`crates/thunderforge-core/src/policies/mod.rs` is a standalone
allow/deny matcher with its own tests and no caller outside itself.
`permission_grants` has no migration and no code: it was deleted on
2026-08-20 along with `rbac.rs` (history note in `docs/SECURITY_RBAC.md`).
The only places it still appears are MVP.md and that document.

## Decisions

### Made by the owner

- ADR-099: the role is **Trusted Player**, stored `TrustedPlayer`; the rank is
  total; only the Owner or a GM grants or revokes it; a per-member "trusted"
  flag outside the ranking was rejected.
- Spec 010 FR-013 to FR-019: the ownership block is Viewer/Editor/Owner per
  member, only the GM edits it, default Viewer, the GM always retains
  Owner-equivalent control.
- Spec 045 FR-033 and spec 048 FR-037: content trust is not a licence to see
  hidden tokens, fog or anything the server withholds from players.

### Made by this spec

- **No fifth role.** The "assistant GM" the MVP asks for is the Trusted
  Player, widened to run the table. A new rank between Trusted Player and GM
  would inherit the content powers anyway (the rank is total), and would touch
  the DB CHECK, two Rust enums, `thunderforge_cache_core`'s role, the web
  `WORLD_ROLES` tuple and every adapter between them for no difference a
  table would notice. Rejected.
- **Amend ADR-099, do not contradict it.** ADR-099's "may not" line is
  narrowed by this spec: a Trusted Player may now _run_ a scene (switch it,
  move tokens on it, work its doors, run combat) but still may not _author_
  one (walls, lights, levels, scene fields), change fog, see hidden content,
  or touch membership, invites, settings, or the world's existence. A
  one-paragraph amendment is appended to the ADR with a pointer here.
- **One new capability, one new predicate.** `Capability::RunTheTable`,
  granted by `Role::runs_the_table()` (`>= TrustedPlayer`). Nothing else in
  `capability.rs` moves. There is no general policy engine, no resource
  strings, no allow/deny rows: the matrix in this spec is the policy.
- **Scene ownership is a fifth permissioned entity**, declared through the
  existing `permissioned_entities!` macro, with its own
  `world_scene_permissions` table, resolvers and GraphQL mutations shaped
  exactly like the actor ones. Levels are:
  - **Viewer**: may see the scene even while it is hidden.
  - **Editor**: Viewer, plus may author on the scene (everything
    `is_dm_of_scene` gates today).
  - **Owner**: Editor, plus may run the scene (launch it, move any token on
    it, work its doors, bring the party to it) and delete it.
- **`scenes.owner_id` stays but is not the block.** It is a creator record,
  set once and never consulted for authorization. Nothing here reads or
  changes it; renaming or dropping it is not worth a migration.
- **A player who may not view a scene is told nothing.** The scene is absent
  from `scenes(worldId)`, from the switcher, and from the dock; `scene(id)`
  returns null; a mutation naming it is refused the same way a nonexistent
  scene is. This is today's `scene_visible` behaviour with the Viewer grant
  added as a second way to pass it.
- **The creator gets an Owner row.** `createWorld` inserts a
  `world_members` row with role `Owner` in the same transaction as the world,
  and a migration backfills one for every existing world whose creator has
  none. The `worlds.created_by` fallback in `require_world_member` stays for
  one release as a belt and is removed in a follow-up once the backfill has
  run everywhere; this spec does not change which user owns a world.
- **Dead code goes.** The `policies` table, its `schema.rs` entry, the admin
  dashboard count, the two user-export fields and the `thunderforge_core::policies`
  module are removed. `permission_grants` needs no task beyond correcting
  the documents that still name it.

## User Scenarios & Testing

### User Story 1 - A Trusted Player runs the fight (Priority: P1)

The GM steps away mid-session and hands the table to a Trusted Player. That
player launches the next scene, moves the goblins, opens the locked door for
the party, starts combat and advances turns. They cannot remove a member,
change the game system or delete the world.

**Why this priority**: it is the gap MVP Phase 10 names first, and it needs
no new table: only a predicate and a set of gates widen.

**Independent Test**: sign in as a Trusted Player; `launchScene`,
`updateToken` (position), `activateInteractive` on a locked door,
`startCombat`, `advanceTurn` succeed; `updateMemberRole`, `removeMember`,
`updateWorldGameSystem`, `deleteWorld`, `createWall` are refused server-side.

**Acceptance Scenarios**:

1. **Given** a Trusted Player and a hidden scene the GM has not launched,
   **When** they call `launchScene`, **Then** the world's active scene changes
   and every client follows.
2. **Given** a Trusted Player and a token owned by another player, **When**
   they move it, **Then** the move lands, and **When** they change its label
   or hidden flag instead, **Then** the server refuses.
3. **Given** a Trusted Player at a locked door, **When** they activate it,
   **Then** it opens as it would for a GM, and **When** a Player asks for the
   same door, **Then** the Trusted Player may approve or refuse the request.
4. **Given** a Trusted Player, **When** they call `removeMember`,
   `updateMemberRole`, `generateInviteCode`, `renameWorld` or `deleteWorld`,
   **Then** each is refused with the answer a Player gets today.
5. **Given** a Trusted Player, **When** they call `createWall`,
   `updateFogMask`, `setSceneExploration` or `updateSceneHidden`, **Then**
   each is refused: running is not authoring.

---

### User Story 2 - A player owns their scene (Priority: P1)

A player built the tavern. The GM grants them Editor on it; they keep drawing
walls and placing lights without becoming a GM. Later the GM grants Owner,
and the player can launch it and run it.

**Why this priority**: the second gap MVP Phase 10 names, and the one with a
schema change.

**Independent Test**: as GM, `setScenePermission(sceneId, userId, EDITOR)`;
as that player, `createWall` on the scene succeeds and `createWall` on
another scene is refused; raise to OWNER and `launchScene` succeeds.

**Acceptance Scenarios**:

1. **Given** a scene with no grants, **When** a Player calls `createWall` on
   it, **Then** the server refuses (today's behaviour, unchanged).
2. **Given** the GM grants a Player Editor on scene A, **When** that Player
   creates a wall on A, **Then** it lands; **When** they create a wall on
   scene B, **Then** it is refused.
3. **Given** Editor on A, **When** that Player calls `launchScene(A)` or
   `deleteScene(A)`, **Then** both are refused; **When** the GM raises them to
   Owner, **Then** both succeed.
4. **Given** a Player holding Owner on A, **When** they call
   `setScenePermission` on A for somebody else, **Then** it is refused: only
   the GM edits the block.
5. **Given** a member holding Editor on A, **When** the GM removes them from
   the world, **Then** their grant is purged with their actor, lore, item and
   ability grants, and re-adding them does not restore it.

---

### User Story 3 - A hidden scene shown to one player (Priority: P2)

The GM hides the dream sequence from everyone but grants one player Viewer.
That player sees it in the switcher and can open it; the rest of the table
does not know it exists.

**Why this priority**: it completes the block with its lowest level and
settles what a player sees of a scene they may not view.

**Independent Test**: hide scene A; grant player P1 Viewer; P1's
`scenes(worldId)` includes A and P2's does not; P2's `scene(A)` is null and
`updateToken` on a token of A by P2 is refused as not found.

**Acceptance Scenarios**:

1. **Given** hidden scene A and no grants, **When** a Player lists scenes,
   **Then** A is absent from the list, the switcher and the dock.
2. **Given** Viewer granted to P1 on A, **When** P1 lists scenes, **Then** A
   is present and openable; **When** P2 lists scenes, **Then** A is still
   absent.
3. **Given** the GM launches A, **When** any member lists scenes, **Then** A
   is present (the active scene is always visible, unchanged).
4. **Given** P2 learns A's id, **When** P2 queries `scene(A)` or mutates a
   token on A, **Then** the answer is the one a nonexistent scene gets.

---

### Edge Cases

- A Trusted Player who also holds Editor on a scene may author that scene:
  the block adds to the role, never subtracts.
- A Trusted Player is demoted to Player mid-session: their next `launchScene`
  is refused; a scene grant they hold survives (grants are keyed on the
  member, not the role).
- A scene Owner grant on the active scene does not let its holder hide it
  from themselves: `updateSceneHidden` is authoring (Editor), and the active
  scene stays visible to everyone regardless.
- A member removed from the world loses every scene grant with their other
  grants (`purge_member_grants`); the macro guarantees there is no fifth
  hand-written block to forget.
- Deleting a scene cascades its grants (`ON DELETE CASCADE`), as for actors.
- A site admin is treated as the world's Owner everywhere, as today.
- The GM's own row in the block is shown as Owner and cannot be lowered, as
  for actors (spec 010 FR-017).

## Requirements

### Functional Requirements

**The Trusted Player runs the table**

- **FR-001**: `thunderforge_authz::Role` MUST gain `runs_the_table()`, true
  for `TrustedPlayer`, `GameMaster` and `Owner`, and `Capability` MUST gain
  `RunTheTable`, granted by it. No other capability's mapping changes.
- **FR-002**: The mutations marked **TP after = ✓** in the matrix below MUST
  accept a Trusted Player; every mutation marked **✗** in that column MUST
  refuse one with the same answer it gives a Player today.
- **FR-003**: `updateToken` from a Trusted Player (or a scene Owner, FR-013)
  MUST accept only the position fields (`x`, `y`, `z`, `rotation`, `level`)
  and MUST refuse the call outright if any other field is present.
- **FR-004**: `activateInteractive` from a Trusted Player MUST resolve with
  the GM outcome (a locked or GM-only interactive fires), and
  `approveRequest`/`refuseRequest`/`setDoorLock` MUST accept a Trusted
  Player.
- **FR-005**: Membership (`updateMemberRole`, `removeMember`), invites
  (`generateInviteCode`, `revokeInviteCode`, `rotateInviteCode`), world
  settings, `renameWorld`, `deleteWorld`, fog (`updateFogMask`,
  `setSceneExploration`, `resetSceneExploration`), hidden-content reveals
  (`setTokenDisclosure`, `setTokenNameVisibility`, `setActorVisibleToPlayers`),
  GM-only chat and `rollBackActor` MUST stay refused to a Trusted Player.
- **FR-006**: The web app MUST expose `runsTheTable` beside `isGm` and
  `managesContent` in `useWorldRole` and `types/world.ts`, and the controls
  for the mutations in FR-002 MUST render for a Trusted Player. No other
  GM-gated control opens.
- **FR-007**: ADR-099 MUST be amended with a dated paragraph naming this spec
  and the new line between running and authoring; `role.rs`'s test message
  ("trusted with content, not with the table") MUST be updated to match.

**Scene ownership**

- **FR-008**: A table `world_scene_permissions` MUST exist with
  `id, scene_id (FK scenes ON DELETE CASCADE), user_id (FK users ON DELETE
CASCADE), level CHECK IN ('Viewer','Editor','Owner'), created_by, updated_by,
created_at, updated_at, UNIQUE(scene_id, user_id)`.
- **FR-009**: `scene` MUST be declared as the fifth entity in
  `permissioned_entities!`, producing `effective_scene_permission`,
  `require_scene_permission` and its purge arm, so a removed member loses
  scene grants with the rest.
- **FR-010**: GraphQL MUST offer `setScenePermission(input:
SetScenePermissionInput!)` and `removeScenePermission(sceneId, userId)`,
  a `GraphQLScenePermission` type, and a `permissions` field on
  `GraphQLScene`, each shaped like the actor equivalents. Both mutations
  MUST refuse anyone who does not run the world, including a scene Owner.
- **FR-011**: The GM (Owner or Game Master, or a site admin) MUST resolve to
  Owner on every scene regardless of the block; a member with no row MUST
  resolve to Viewer-by-default, which for scenes means today's
  `scene_visible` rule.
- **FR-012**: `is_dm_of_scene` MUST be replaced by `may_author_scene`, true
  when the caller runs the world or holds Editor or Owner on the scene. Every
  call site that gates scene authoring today MUST go through it: walls,
  lights, shapes (the GM branch), tokens (create, delete, full update),
  levels, interactives, `updateScene`, `updateSceneHidden`,
  `updateSceneAmbientLight`, `updateFogMask`, exploration, `uploadCanvasImage`.
- **FR-013**: `launchScene`, `deleteScene`, `bringPartyToScene`,
  `moveTokensToLevel`, position-only `updateToken`, `activateInteractive`
  (GM outcome), `approveRequest`, `refuseRequest` and `setDoorLock` MUST
  accept a holder of Owner on that scene, in addition to whoever FR-002
  admits.
- **FR-014**: Scene visibility MUST become
  `runs_the_world || !hidden || is_world_active_scene || holds Viewer-or-above`,
  applied in `scenes(worldId)`, `scene(id)`, the scene subscription payloads,
  and every scene-scoped query that filters on `scene_visible` today.
- **FR-015**: A member refused by FR-014 MUST receive the answer a nonexistent
  scene gets (null, empty, or the not-found refusal), never a distinct
  "forbidden", so a scene's existence is not disclosed.
- **FR-016**: The scene switcher and the dock MUST render only the scenes the
  server returned; no client-side filtering by role or grant MAY be added or
  relied on.
- **FR-017**: The scenes page MUST show the ownership block for a scene to
  whoever runs the world, listing every member with their level (the GM's row
  fixed at Owner), using the same component shape as the actor block.

**The creator's membership row**

- **FR-021**: `createWorld` (both the plain and the with-map path) MUST
  insert a `world_members` row (`role = 'Owner'`) for the creator in the
  transaction that inserts the world.
- **FR-022**: A migration MUST backfill an Owner row for every existing world
  whose `created_by` has no `world_members` row, idempotently
  (`ON CONFLICT DO NOTHING` on `(world_id, user_id)`), and its `down.sql`
  MUST remove only the rows it added.
- **FR-023**: The members list for a new world MUST show its creator as
  Owner; `updateMemberRole` and `removeMember` MUST keep refusing to change
  or remove the Owner row, as `can_assign` already does for the rank.

**Dead tables**

- **FR-018**: A migration MUST drop `policies` and the `PolicyEffect` type;
  `schema.rs`, `admin.rs`'s `total_policies`, the `policies` fields of the
  user data export, `GraphQLPolicy`, and `thunderforge_core::policies` MUST
  be removed with it, with the commented-out references deleted rather than
  left.
- **FR-019**: `MVP.md` Phase 10 and `docs/SECURITY_RBAC.md` MUST be corrected
  to describe the four roles, the five ownership blocks and the absence of
  `policies`/`permission_grants`; Phase 10 MUST be marked `[x]`.

**Telemetry**

- **FR-020**: Every refusal added or widened here MUST be recorded through the
  existing authorization refusal span attributes (`authz.capability`,
  `authz.role`), so a Trusted Player hitting a GM gate is visible in traces.

### The mutation matrix

Every mutation in `apps/thunderforge/schema.graphql`'s `AppMutationRoot`
(227 on 2026-10-13), grouped by area. Columns: **Player** and **GM** are
today's behaviour and do not change; **TP today** is the Trusted Player
today; **TP after** is the Trusted Player once this spec lands; **Scene grant**
names the lowest scene level that admits a Player who holds it (`–` means the
block does not apply). **Owner** differs from GM only where shown.

Key: `✓` accepted, `✗` refused, `own` accepted under the mutation's own
per-item rule (the caller's actor, token, shape, collection or request; the
rule is unchanged), `admin` site admin only, `any` any signed-in account,
`member` any world member. Rows whose **TP after** differs from **TP today**
are marked **changed**.

**World settings and existence**

| Mutation                            | Player | TP today | TP after | Scene grant | GM  | Owner |
| ----------------------------------- | ------ | -------- | -------- | ----------- | --- | ----- |
| createWorld                         | any    | any      | any      | –           | any | any   |
| renameWorld                         | ✗      | ✗        | ✗        | –           | ✗   | ✓     |
| deleteWorld                         | ✗      | ✗        | ✗        | –           | ✗   | ✓     |
| updateWorldSessionNotes             | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldGameSystem               | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldInterfacePack            | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldAllowPlayerCreatedActors | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldAllowPlayerActorArt      | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldGenieResourceCarryover   | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldDefaultSceneGridType     | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateWorldAutoApplyNpcDamage       | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| setCombatAutoApply                  | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| setWorldSystemSetting               | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| updateRollForShoesWorldSettings     | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| setRollForShoesTableDifficulty      | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| clearRollForShoesTableDifficulty    | ✗      | ✗        | ✗        | –           | ✓   | ✓     |

**Membership and invites**

| Mutation                  | Player | TP today | TP after | Scene grant | GM                       | Owner |
| ------------------------- | ------ | -------- | -------- | ----------- | ------------------------ | ----- |
| joinWorld                 | any    | any      | any      | –           | any                      | any   |
| generateInviteCode        | ✗      | ✗        | ✗        | –           | ✓                        | ✓     |
| revokeInviteCode          | ✗      | ✗        | ✗        | –           | ✓                        | ✓     |
| rotateInviteCode          | ✗      | ✗        | ✗        | –           | ✓                        | ✓     |
| updateMemberRole          | ✗      | ✗        | ✗        | –           | ✓ (at or below own rank) | ✓     |
| removeMember              | ✗      | ✗        | ✗        | –           | ✓                        | ✓     |
| setAuthoringToolGrant     | ✗      | ✗        | ✗        | –           | ✓                        | ✓     |
| setPlayerCharacterBinding | own    | own      | own      | –           | ✓                        | ✓     |

**Scenes** (the scene grant column is the heart of US2 and US3)

| Mutation                    | Player | TP today | TP after      | Scene grant | GM  | Owner |
| --------------------------- | ------ | -------- | ------------- | ----------- | --- | ----- |
| createScene                 | ✗      | ✗        | ✗             | –           | ✓   | ✓     |
| updateScene                 | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| updateSceneHidden           | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| updateSceneAmbientLight     | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| launchScene                 | ✗      | ✗        | ✓ **changed** | Owner       | ✓   | ✓     |
| deleteScene                 | ✗      | ✗        | ✗             | Owner       | ✓   | ✓     |
| updateFogMask               | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| setSceneExploration         | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| resetSceneExploration       | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| setScenePermission (new)    | ✗      | ✗        | ✗             | –           | ✓   | ✓     |
| removeScenePermission (new) | ✗      | ✗        | ✗             | –           | ✓   | ✓     |
| uploadCanvasImage           | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |

**Levels**

| Mutation           | Player | TP today | TP after      | Scene grant | GM  | Owner |
| ------------------ | ------ | -------- | ------------- | ----------- | --- | ----- |
| createSceneLevel   | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| updateSceneLevel   | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| reorderSceneLevels | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| deleteSceneLevel   | ✗      | ✗        | ✗             | Editor      | ✓   | ✓     |
| moveTokensToLevel  | ✗      | ✗        | ✓ **changed** | Owner       | ✓   | ✓     |

**Walls, lights and shapes**

| Mutation          | Player | TP today | TP after | Scene grant        | GM  | Owner |
| ----------------- | ------ | -------- | -------- | ------------------ | --- | ----- |
| createWall        | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| updateWall        | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| deleteWall        | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| createLightSource | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| updateLightSource | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| deleteLightSource | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |
| createShape       | own    | own      | own      | Editor (GM branch) | ✓   | ✓     |
| updateShape       | own    | own      | own      | Editor (GM branch) | ✓   | ✓     |
| deleteShape       | own    | own      | own      | Editor (GM branch) | ✓   | ✓     |
| clearShapes       | ✗      | ✗        | ✗        | Editor             | ✓   | ✓     |

**Interactives and doors**

| Mutation            | Player            | TP today | TP after                 | Scene grant | GM  | Owner |
| ------------------- | ----------------- | -------- | ------------------------ | ----------- | --- | ----- |
| createInteractive   | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| updateInteractive   | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| deleteInteractive   | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| resetInteractive    | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| setDoorDesignation  | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| setDoorSecret       | ✗                 | ✗        | ✗                        | Editor      | ✓   | ✓     |
| setDoorLock         | ✗                 | ✗        | ✓ **changed**            | Owner       | ✓   | ✓     |
| approveRequest      | ✗                 | ✗        | ✓ **changed**            | Owner       | ✓   | ✓     |
| refuseRequest       | ✗                 | ✗        | ✓ **changed**            | Owner       | ✓   | ✓     |
| activateInteractive | own (canActivate) | own      | ✓ GM outcome **changed** | Owner       | ✓   | ✓     |

**Tokens and the party**

| Mutation                | Player | TP today | TP after                    | Scene grant                           | GM  | Owner |
| ----------------------- | ------ | -------- | --------------------------- | ------------------------------------- | --- | ----- |
| createToken             | ✗      | ✗        | ✗                           | Editor                                | ✓   | ✓     |
| deleteToken             | ✗      | ✗        | ✗                           | Editor                                | ✓   | ✓     |
| updateToken             | ✗      | ✗        | ✓ position only **changed** | Owner (position); Editor (all fields) | ✓   | ✓     |
| moveOwnToken            | own    | own      | own                         | –                                     | ✓   | ✓     |
| setOwnPrimaryTokenPhoto | own    | own      | own                         | –                                     | ✓   | ✓     |
| setTokenDisclosure      | ✗      | ✗        | ✗                           | Editor                                | ✓   | ✓     |
| setTokenNameVisibility  | ✗      | ✗        | ✗                           | Editor                                | ✓   | ✓     |
| setTokenLink            | ✗      | ✗        | ✗                           | Editor                                | ✓   | ✓     |
| setActorUnique          | ✗      | ✗        | ✗                           | –                                     | ✓   | ✓     |
| bringPartyToScene       | ✗      | ✗        | ✓ **changed**               | Owner                                 | ✓   | ✓     |
| pickUpPlacedItem        | own    | own      | own                         | –                                     | ✓   | ✓     |

**Combat, conditions, rolls and chat**

| Mutation               | Player              | TP today | TP after                          | Scene grant | GM  | Owner |
| ---------------------- | ------------------- | -------- | --------------------------------- | ----------- | --- | ----- |
| startCombat            | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| addCombatant           | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| updateCombatant        | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| removeCombatant        | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| advanceTurn            | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| endCombat              | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| changeHitPoints        | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| addLairCombatant       | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| makeAttack             | own (turn)          | own      | own                               | –           | ✓   | ✓     |
| resolveOffer           | own                 | own      | own                               | –           | ✓   | ✓     |
| applyActorCondition    | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| clearActorCondition    | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| rollDice               | member              | member   | member                            | –           | ✓   | ✓     |
| rollCheck              | member              | member   | member                            | –           | ✓   | ✓     |
| rerollRoll             | own                 | own      | own                               | –           | ✓   | ✓     |
| revealRoll             | ✗                   | ✗        | ✓ **changed**                     | –           | ✓   | ✓     |
| clearWorldRolls        | ✗                   | ✗        | ✗                                 | –           | ✓   | ✓     |
| sendChatMessage        | member (not gmOnly) | member   | member (not gmOnly)               | –           | ✓   | ✓     |
| pauseWorldPlay         | ✗                   | ✗        | ✗                                 | –           | ✓   | ✓     |
| decidePlayPauseRequest | ✗                   | ✗        | ✗                                 | –           | ✓   | ✓     |
| liftWorldPlayPause     | ✗                   | ✗        | ✗                                 | –           | ✓   | ✓     |
| reconcileQueuedChanges | member (per item)   | member   | member (per item, TP after rules) | –           | ✓   | ✓     |
| heartbeat              | member              | member   | member                            | –           | ✓   | ✓     |
| sendPeerSignal         | member              | member   | member                            | –           | ✓   | ✓     |

**Actors**

| Mutation                 | Player                      | TP today | TP after | Scene grant | GM  | Owner |
| ------------------------ | --------------------------- | -------- | -------- | ----------- | --- | ----- |
| createActor              | ✗ (✓ when the world allows) | same     | same     | –           | ✓   | ✓     |
| updateActor              | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| updateActorSystemData    | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| uploadActorImage         | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| removeActorImage         | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| setActorVisibleToPlayers | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| setActorArtLocked        | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| setActorPermission       | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| removeActorPermission    | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| createActorShareLink     | own (actor Owner)           | own      | own      | –           | ✓   | ✓     |
| revokeActorShareLink     | own (actor Owner)           | own      | own      | –           | ✓   | ✓     |
| copySharedActorToWorld   | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| attachAbilityToActor     | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| detachAbilityFromActor   | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| claimActor               | own                         | own      | own      | –           | ✓   | ✓     |
| createAndClaimActor      | own (when the world allows) | own      | own      | –           | ✓   | ✓     |
| unclaimActor             | own                         | own      | own      | –           | ✓   | ✓     |
| setActorAvailability     | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |
| applySheetImport         | own (actor Editor)          | own      | own      | –           | ✓   | ✓     |
| rollBackActor            | ✗                           | ✗        | ✗        | –           | ✓   | ✓     |

**Items and inventory**

| Mutation                | Player             | TP today | TP after | Scene grant | GM  | Owner |
| ----------------------- | ------------------ | -------- | -------- | ----------- | --- | ----- |
| createItem              | ✗                  | ✗        | ✗        | –           | ✓   | ✓     |
| updateItem              | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| deleteItem              | own (item Owner)   | own      | own      | –           | ✓   | ✓     |
| addItemEffect           | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| updateItemEffect        | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| removeItemEffect        | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| setItemAttack           | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| setItemPrice            | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| clearItemPrice          | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| setItemPermission       | ✗                  | ✗        | ✗        | –           | ✓   | ✓     |
| removeItemPermission    | ✗                  | ✗        | ✗        | –           | ✓   | ✓     |
| createItemShareLink     | own (item Owner)   | own      | own      | –           | ✓   | ✓     |
| revokeItemShareLink     | own (item Owner)   | own      | own      | –           | ✓   | ✓     |
| copySharedItemToWorld   | ✗                  | ✗        | ✗        | –           | ✓   | ✓     |
| attachAbilityToItem     | own (item Editor)  | own      | own      | –           | ✓   | ✓     |
| addItemToInventory      | own (actor Editor) | own      | own      | –           | ✓   | ✓     |
| adjustInventoryQuantity | own (actor Editor) | own      | own      | –           | ✓   | ✓     |
| removeInventoryEntry    | own (actor Editor) | own      | own      | –           | ✓   | ✓     |

**Abilities**

| Mutation                 | Player               | TP today | TP after | Scene grant | GM  | Owner |
| ------------------------ | -------------------- | -------- | -------- | ----------- | --- | ----- |
| createAbility            | ✗                    | ✗        | ✗        | –           | ✓   | ✓     |
| updateAbility            | own (ability Editor) | own      | own      | –           | ✓   | ✓     |
| deleteAbility            | own (ability Owner)  | own      | own      | –           | ✓   | ✓     |
| addAbilityEffect         | own (ability Editor) | own      | own      | –           | ✓   | ✓     |
| updateAbilityEffect      | own (ability Editor) | own      | own      | –           | ✓   | ✓     |
| removeAbilityEffect      | own (ability Editor) | own      | own      | –           | ✓   | ✓     |
| setAbilityAttack         | own (ability Editor) | own      | own      | –           | ✓   | ✓     |
| setAbilityGmOnly         | ✗                    | ✗        | ✗        | –           | ✓   | ✓     |
| setAbilityPermission     | ✗                    | ✗        | ✗        | –           | ✓   | ✓     |
| removeAbilityPermission  | ✗                    | ✗        | ✗        | –           | ✓   | ✓     |
| createAbilityShareLink   | own (ability Owner)  | own      | own      | –           | ✓   | ✓     |
| revokeAbilityShareLink   | own (ability Owner)  | own      | own      | –           | ✓   | ✓     |
| copySharedAbilityToWorld | ✗                    | ✗        | ✗        | –           | ✓   | ✓     |

**Lore**

| Mutation                         | Player            | TP today | TP after | Scene grant | GM  | Owner |
| -------------------------------- | ----------------- | -------- | -------- | ----------- | --- | ----- |
| createLoreEntry                  | ✗                 | ✗        | ✗        | –           | ✓   | ✓     |
| updateLoreEntry                  | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| deleteLoreEntry                  | own (lore Owner)  | own      | own      | –           | ✓   | ✓     |
| restoreLoreRevision              | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| uploadLoreImage                  | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| moveLoreEntry                    | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| addLoreTag                       | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| removeLoreTag                    | own (lore Editor) | own      | own      | –           | ✓   | ✓     |
| setLorePermission                | ✗                 | ✗        | ✗        | –           | ✓   | ✓     |
| removeLorePermission             | ✗                 | ✗        | ✗        | –           | ✓   | ✓     |
| beginLoreRepositoryConnection    | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| completeLoreRepositoryConnection | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| acknowledgeLoreSyncNotice        | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| resolveLoreSyncDivergence        | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| acceptLoreIncomingChange         | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| declineLoreIncomingChange        | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| deactivateLoreSync               | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |
| removeLoreRepositoryConnection   | ✗                 | ✗        | ✗        | –           | ✗   | ✓     |

**Books, collections and staged content** (ADR-099's ground; unchanged)

| Mutation                      | Player        | TP today | TP after | Scene grant | GM  | Owner |
| ----------------------------- | ------------- | -------- | -------- | ----------- | --- | ----- |
| switchOnCompendium            | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| switchOffCompendium           | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| changeWorldEntry              | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| hideWorldEntry                | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| addWorldEntry                 | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| removeKeptAddition            | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| restoreWorldEntry             | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| syncBackToCollection          | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| adoptStagedContent            | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| adoptAllStagedContent         | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| declineStagedContent          | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| revisitStagedContent          | ✗             | ✓        | ✓        | –           | ✓   | ✓     |
| createCompendiumFromImport    | own (account) | own      | own      | –           | own | own   |
| removeCompendium              | own (account) | own      | own      | –           | own | own   |
| createCollection              | own (account) | own      | own      | –           | own | own   |
| updateCollection              | own (account) | own      | own      | –           | own | own   |
| deleteCollection              | own (account) | own      | own      | –           | own | own   |
| addCollectionMember           | own (account) | own      | own      | –           | own | own   |
| removeCollectionMember        | own (account) | own      | own      | –           | own | own   |
| createCollectionShareLink     | own (account) | own      | own      | –           | own | own   |
| revokeCollectionShareLink     | own (account) | own      | own      | –           | own | own   |
| copySharedCollectionToWorld   | ✗             | ✗        | ✗        | –           | ✓   | ✓     |
| createShelfCollection         | own (account) | own      | own      | –           | own | own   |
| writeShelfCollectionEntry     | own (account) | own      | own      | –           | own | own   |
| removeShelfCollectionEntry    | own (account) | own      | own      | –           | own | own   |
| restoreShelfCollectionVersion | own (account) | own      | own      | –           | own | own   |

**Game-system packs** (Genie and Roll for Shoes; unchanged, GM-side ones
stay GM until a system spec asks otherwise)

| Mutation                   | Player | TP today | TP after | Scene grant | GM  | Owner |
| -------------------------- | ------ | -------- | -------- | ----------- | --- | ----- |
| startGenieSession          | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| spendWish                  | own    | own      | own      | –           | ✓   | ✓     |
| advanceDoomClock           | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| createPuzzleClock          | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| advancePuzzleClock         | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| configurePuzzleClockReward | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| grantSessionResource       | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| createShopListing          | ✗      | ✗        | ✗        | –           | ✓   | ✓     |
| purchaseFromShop           | own    | own      | own      | –           | ✓   | ✓     |
| proposeResourceTrade       | own    | own      | own      | –           | ✓   | ✓     |
| acceptResourceTrade        | own    | own      | own      | –           | ✓   | ✓     |
| declineResourceTrade       | own    | own      | own      | –           | ✓   | ✓     |
| spendResourceOnPuzzleClock | own    | own      | own      | –           | ✓   | ✓     |
| rollForShoesRollSkill      | own    | own      | own      | –           | ✓   | ✓     |

**Instance, moderation, legal and the account** (no world role applies)

| Mutation                     | Player | TP today | TP after | Scene grant | GM    | Owner |
| ---------------------------- | ------ | -------- | -------- | ----------- | ----- | ----- |
| updateOauthProvider          | admin  | admin    | admin    | –           | admin | admin |
| updateManifestKey            | admin  | admin    | admin    | –           | admin | admin |
| updateTwoFactorPolicy        | admin  | admin    | admin    | –           | admin | admin |
| setInstanceAccessPolicy      | admin  | admin    | admin    | –           | admin | admin |
| createInstanceInvitation     | admin  | admin    | admin    | –           | admin | admin |
| revokeInstanceInvitation     | admin  | admin    | admin    | –           | admin | admin |
| setGithubApplication         | admin  | admin    | admin    | –           | admin | admin |
| checkGithubApplication       | admin  | admin    | admin    | –           | admin | admin |
| updateInstanceSetting        | admin  | admin    | admin    | –           | admin | admin |
| sendTestMail                 | admin  | admin    | admin    | –           | admin | admin |
| retryOutboxMessage           | admin  | admin    | admin    | –           | admin | admin |
| abandonFeedbackDelivery      | admin  | admin    | admin    | –           | admin | admin |
| resumeFeedbackDelivery       | admin  | admin    | admin    | –           | admin | admin |
| setLegalEnquiryStatus        | admin  | admin    | admin    | –           | admin | admin |
| resolveModerationCase        | admin  | admin    | admin    | –           | admin | admin |
| resolveAppeal                | admin  | admin    | admin    | –           | admin | admin |
| executeTermination           | admin  | admin    | admin    | –           | admin | admin |
| submitTakedownNotice         | any    | any      | any      | –           | any   | any   |
| submitCounterNotice          | ✗      | ✗        | ✗        | –           | ✓     | ✓     |
| submitFeedback               | any    | any      | any      | –           | any   | any   |
| submitLegalEnquiry           | any    | any      | any      | –           | any   | any   |
| fileAppeal                   | any    | any      | any      | –           | any   | any   |
| acknowledgeOperatorStatement | any    | any      | any      | –           | any   | any   |
| endSession                   | own    | own      | own      | –           | own   | own   |

Rows that change for the Trusted Player: 23 (`launchScene`,
`moveTokensToLevel`, `setDoorLock`, `approveRequest`, `refuseRequest`,
`activateInteractive`, `updateToken`, `bringPartyToScene`, eight combat
mutations, two condition mutations, `revealRoll`, and
`reconcileQueuedChanges` only insofar as it replays those). Rows the scene
block reaches: 36. Everything else is listed so the reader can see it does
not move.

### Key Entities

- **Trusted Player**: the existing fourth world role, now also the one who
  runs the table. Rank unchanged. New predicate `runs_the_table()`.
- **Scene permission**: a row of `world_scene_permissions` (`scene_id`,
  `user_id`, `level`), the fifth permissioned entity. The GM is Owner
  implicitly; an un-granted member is Viewer, which on a scene means "sees it
  if it is not hidden or is the active scene".
- **Scene** (existing): gains a `permissions` field; `hidden` and the world's
  `active_scene_id` keep their meaning.
- **`policies`** (removed): the dead table and the dead core module.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A Trusted Player launches a scene, moves another player's
  token, opens a locked door, starts combat and advances a turn, each
  confirmed by the server and seen by a second client.
- **SC-002**: The same Trusted Player is refused by the server on
  `removeMember`, `updateMemberRole`, `generateInviteCode`,
  `updateWorldGameSystem`, `renameWorld`, `deleteWorld`, `createWall`,
  `updateFogMask`, `updateSceneHidden` and `updateToken` with a non-position
  field; the web app shows them no control for any of those.
- **SC-003**: A Player granted Editor on scene A creates a wall on A and is
  refused on scene B; raised to Owner, they launch and then delete A; the GM
  removes them from the world and `world_scene_permissions` holds no row for
  them afterwards.
- **SC-004**: A Player granted Viewer on a hidden scene sees it in the
  switcher and opens it; a Player without the grant does not see it in the
  switcher or the dock, gets null from `scene(id)`, and is refused as
  not-found on a token mutation naming it.
- **SC-005**: A Player holding Owner on a scene is refused on
  `setScenePermission` for that scene.
- **SC-006**: After the migration, `policies` is absent from the database and
  from `schema.rs`; `cargo build` and `pnpm verify` pass; the admin dashboard
  and the user data export no longer carry a `policies` figure.
- **SC-007**: MVP.md Phase 10 reads `[x]` and describes four roles and five
  ownership blocks; `docs/SECURITY_RBAC.md` no longer names
  `permission_grants` as live.
- **SC-008**: A freshly created world lists its creator as Owner on the
  players page without any fallback, and a seeded world that had no Owner row
  shows one after the migration.

### Proof

The slice is **scenes** (`pnpm e2e:scenes`), which already owns
`mutations_scenes.rs`, `mutations_levels.rs`, `mutations_party.rs`,
`SceneSwitcher/**` and `pages/world/scenes/**`. New specs land there under
the `scene-` prefix: `scene-trusted-player.spec.ts` (SC-001, SC-002),
`scene-ownership.spec.ts` (SC-003, SC-005) and `scene-visibility.spec.ts`
(SC-004). The integration slice adds the neighbours whose seams this
crosses: **interactive** (doors), **combat** (turns), **tokens**
(`updateToken`), **actors** (the block component and
`purge_member_grants`), and **accounts** (`invite-membership.spec.ts`, the
role change and user data export). `crates/thunderforge-authz/**`,
`auth/**`, `schema.rs` and the migrations are cross-cutting in
`scripts/e2e/slices.json`, so `pnpm e2e:which` will name those neighbours on
its own; the proof task records what it named and what ran.

## Assumptions

- The `permissioned_entities!` macro takes a fifth arm without change; the
  scene's parent table is `scenes` and its world column is `world_id`, as
  the macro expects.
- `scene_visible` has one definition (`thunderforge_authz::scene`) and every
  scene list and subscription goes through it; the task that widens it will
  grep for callers and list any that bypass it.
- The role travels as a string, so no GraphQL enum changes for the role; the
  level travels as the existing `ActorPermissionLevel` enum, reused as the
  other four kinds reuse it.
- `reconcileQueuedChanges` replays mutations through the same validators, so
  it needs no gate of its own beyond `role_from_membership` continuing to map
  a Trusted Player to its `Player` side for the table-pick logic it feeds.

## Out of scope

- A general policy engine, resource strings, allow/deny rows, or any gate
  decided by data rather than by the matrix above.
- A fifth role, a per-member capability flag, or more than one Trusted
  Player kind.
- Ownership blocks for walls, lights, shapes, levels or background images:
  they belong to their scene.
- Widening the game-system pack mutations (Genie clocks, shops, Roll for
  Shoes) to the Trusted Player; a system spec may ask for that.
- Transfer of world ownership (`TransferOwnership` capability exists and
  stays unused here).
- Renaming or dropping `scenes.owner_id`.

## Open questions

- ADR-099's "may not" paragraph is narrowed by this spec. The owner accepted
  that ADR; the amendment task appends a dated paragraph rather than
  rewriting it, and the owner may instead ask for a new ADR number. Decision
  needed only if the owner prefers the new number.
