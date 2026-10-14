# Implementation Plan: Scene ownership and the role gates

**Branch**: `093-scene-ownership-and-role-gates` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

## Summary

Closes MVP Phase 10 with the smallest set of changes that fills what the
re-verification of 2026-10-13 found still missing. In order:

1. **Server, authz**: `Role::runs_the_table()` and `Capability::RunTheTable`
   in `thunderforge-authz`; `scene_visible` learns a Viewer grant.
2. **Server, schema**: migration for `world_scene_permissions`; migration
   backfilling the creator's Owner row; migration dropping `policies`;
   `schema.rs` regenerated.
3. **Server, validators**: `scene` becomes the fifth arm of
   `permissioned_entities!`; `is_dm_of_scene` becomes `may_author_scene`;
   a `may_run_scene` helper beside it; the 23 Trusted Player gates and the
   36 scene-grant gates move to those helpers; `createWorld` inserts the Owner
   row; position-only `updateToken`.
4. **Server, GraphQL**: `setScenePermission`, `removeScenePermission`,
   `GraphQLScenePermission`, `GraphQLScene.permissions`; `GraphQLPolicy` and
   the export fields go; schema.graphql regenerated.
5. **Rust tests** per gate: a refused mutation for every row that must stay
   refused, an accepted one for every row that opens.
6. **Web**: `runsTheTable` in `types/world.ts` and `useWorldRole`; the scene
   ownership block on the scenes page; the table-running controls shown to a
   Trusted Player; nothing filtered client-side.
7. **Documents**: ADR-099 amendment, MVP.md Phase 10 to `[x]`,
   `docs/SECURITY_RBAC.md` corrected.
8. **Proof**: three new `scene-*.spec.ts` files, `pnpm e2e:scenes` plus the
   neighbours `pnpm e2e:which` names, `pnpm verify`.

**There is no policy engine in this plan.** The matrix in spec.md is the
policy; each row is a fixed predicate in Rust. No resource strings, no
allow/deny rows, nothing decided by data at request time. That is the
decision the dead `policies` table records the opposite of, and removing it
is part of the same change.

### What ADR-099 already covered

ADR-099 (accepted 2026-09-13) delivered the role itself: the `TrustedPlayer`
value, its rank between Player and GM, the `manages_content()` predicate, the
`ManageBookMaterial` capability, the DB CHECK, the web `WORLD_ROLES` tuple
and label, grant/revoke bounded by the caller's own rank (`can_assign`), and
the book, world-entry and staged-content mutations it unlocks. None of that
is redone here. What ADR-099 deliberately left out, and said should come
from a spec, is any table-running power; this spec is that exception, and
amends the ADR rather than replacing it.

## Technical Context

**Language/Version**: Rust 2024 edition (server, authz), TypeScript 5 / React
(web). **Primary Dependencies**: Axum, async-graphql, Diesel (PostgreSQL),
Vite. **Storage**: PostgreSQL; one new table, one dropped, one backfill.
**Testing**: `cargo test` (gate tests next to each validator), Playwright
slices. **Target Platform**: the server and the web app; the engine does not
change (it renders what the store holds, and the store holds what the server
returned). **Performance Goals**: no new round trip per request;
`effective_scene_permission` is one indexed lookup, the same cost as the
actor one, and is only consulted when the world role did not already decide.
**Constraints**: role stays a string over GraphQL; the level reuses
`ActorPermissionLevel`; every refusal keeps the shape it has today so no
client learns a scene exists from a new error. **Scale/Scope**: 227
mutations audited, 23 widened for the Trusted Player, 36 reached by the
scene grant, one new table, one dropped.

## Constitution and AGENTS.md check

- **I. Agnostic core** — the predicate lives in `thunderforge-authz`, which
  has no Diesel or network dependency; the server adapts it. Unchanged
  shape.
- **II. Circular flow** — scene permissions are world data: a grant is a
  mutation, persisted, broadcast as a world event, and dispatched into the
  store. The switcher and dock read the store. No component calls the
  client.
- **III. Ownership server-side** — every gate in the matrix is enforced in a
  resolver or validator, with a Rust test that calls the mutation as the
  refused role. The new table carries `created_by`/`updated_by`. The web
  hides what the server refuses but never decides.
- **IV. ADR before divergence** — ADR-099 is amended by a dated paragraph
  (task T047); ADR-050's macro gains a fifth arm, no new ADR needed for a
  pattern it was written to extend.
- **V. Verify per target** — `cargo test -p thunderforge-authz -p
thunderforge-server`, `pnpm verify`, and the slices named below.
- **VI. Proven by its own slice** — the slice is **scenes**
  (`pnpm e2e:scenes`): it owns `mutations_scenes.rs`, `mutations_levels.rs`,
  `mutations_party.rs`, `SceneSwitcher/**`, `pages/world/scenes/**` and the
  `scene-` prefix the three new specs use. Integration slice: **scenes** plus
  **interactive** (doors), **combat** (turns and conditions), **tokens**
  (`updateToken`), **actors** (the block component, `purge_member_grants`),
  **accounts** (`invite-membership.spec.ts`, the Owner row, the user data
  export). `thunderforge-authz/**`, `auth/**`, `schema.rs`, the migrations
  and `schema.graphql` are cross-cutting, so `pnpm e2e:which` will name
  these on its own; T055 records what it named and what ran.
- **VII. Telemetry** — refusals go through the existing authorization span
  attributes (`authz.capability`, `authz.role`); the new capability is one
  more value, no new exporter. A counter is not warranted: a refusal is a
  trace event, and the dashboards already chart those.
- **AGENTS.md** — no network in Bevy systems (the engine is untouched), no
  component imports the GraphQL client, the store stays the client's single
  source of truth. Vocabulary: "collection", never "pack" or "bundle"; no AI
  game master anywhere in this spec.

## Decisions

- **Widen the Trusted Player; no fifth role.** The rank is total, so a new
  rank above Trusted Player would inherit content powers anyway, and the
  only difference a table would notice is the name. Rejected for the cost it
  adds in two Rust enums, the cache-core role, the DB CHECK, the web tuple
  and every adapter.
- **`runs_the_table()` is a third predicate, not a widening of
  `runs_the_world()`.** Widening the existing one would open every GM gate
  at once, which is exactly what ADR-099 forbids. A new predicate means every
  opened gate is an explicit edit with a test.
- **The scene grant goes through the macro.** ADR-050 made the macro so that
  the fifth kind would not add a fifth hand-written purge block; the
  `purge_member_grants` arm is generated with the resolvers.
- **`may_author_scene` replaces `is_dm_of_scene` by rename, not by a second
  function.** Every caller is touched once, which is the audit. A second
  function next to the old one would leave callers on the old one.
- **Position-only `updateToken` is a field check in the resolver, not a new
  mutation.** The engine already sends position-only updates on drag; a new
  `moveAnyToken` would duplicate the sync bridge for no gain. The resolver
  refuses the whole call if a non-position field is present for a caller
  admitted only by the table or scene-Owner path.
- **Scene visibility stays one function.** `scene_visible` gains a
  `holds_viewer: bool` argument rather than a second function, so no caller
  can keep the old rule by accident. The server computes `holds_viewer` with
  one query per list, not per scene.
- **`createWorld` gets the Owner row; the fallback stays for now.** The
  backfill migration plus the insert make `worlds.created_by` and
  `world_members` agree; removing the fallback is a separate, later change
  once every deployment has migrated (noted in SECURITY_RBAC.md).
- **`policies` goes in this spec, `thunderforge_core::policies` with it.**
  The core module is a matcher with tests and no caller; keeping it would
  keep a policy engine's seed in the tree while this plan says there is none.

## Project Structure

```
crates/thunderforge-authz/src/
├── role.rs                     +runs_the_table(); test message updated       (~20 lines)
├── capability.rs               +RunTheTable; matrix test row                 (~25 lines)
└── scene.rs                    scene_visible(.., holds_viewer)               (~15 lines)

crates/thunderforge-server/
├── migrations/
│   ├── 2026-10-14-100000-0000_world_scene_permissions/{up,down}.sql      (~40 lines)
│   ├── 2026-10-14-100100-0000_backfill_world_owner_members/{up,down}.sql (~25 lines)
│   └── 2026-10-14-100200-0000_drop_policies/{up,down}.sql                (~20 lines)
└── src/
    ├── schema.rs                  regenerated (+world_scene_permissions, -policies)
    ├── auth/
    │   ├── permissioned_entities.rs   fifth arm: scene                     (~15 lines)
    │   ├── world_membership.rs        may_author_scene, may_run_scene      (~60 lines)
    │   └── scene_visibility.rs        holds_viewer lookup, one query       (~50 lines, new)
    ├── graphql/
    │   ├── mutations_scene_permissions.rs   set/remove, mirrors actor      (~120 lines, new)
    │   ├── mutations_scenes.rs        gates to may_author/may_run          (~30 lines changed)
    │   ├── mutations_tokens.rs        position-only path                   (~40 lines changed)
    │   ├── mutations_interactives.rs  activate/approve/refuse/lock gates   (~20 lines changed)
    │   ├── mutations_combat.rs        runs_the_table                        (~16 lines changed)
    │   ├── mutations_levels.rs, mutations_party.rs, mutations_walls.rs,
    │   │   mutations_lighting.rs, mutations_shapes.rs, mutations_roll.rs,
    │   │   actor_conditions.rs, exploration.rs, scene_levels.rs            (one gate each)
    │   ├── mutations_worlds.rs        Owner row at create                   (~20 lines)
    │   ├── queries/scene.rs           visibility with the grant             (~30 lines)
    │   ├── types_scene.rs             permissions field                     (~30 lines)
    │   ├── input_types.rs, mutations_user_data.rs   -policies fields
    │   └── helpers.rs, queries/user.rs              dead comments removed
    ├── admin.rs                   -total_policies
    ├── users/mod.rs               -policies in the export
    └── models.rs                  dead comment removed
crates/thunderforge-core/src/{lib.rs, policies/}   module removed

apps/thunderforge/schema.graphql   regenerated

apps/web/src/
├── types/world.ts                 runsTheTable()                            (~10 lines)
├── hooks/useWorldRole.ts          runsTheTable flag                         (~10 lines)
├── api/scenes.ts                  setScenePermission, removeScenePermission (~40 lines)
├── engine/world/sync/scenes.ts    permissions on the scene record           (~20 lines)
├── pages/world/scenes/SceneOwnershipBlock.tsx   from ActorOwnershipBlock    (~120 lines, new)
├── components/world/SceneSwitcher/SceneSwitcher.tsx   launch for runsTheTable (~10 lines)
├── components/world/PlayDock/**   combat/condition controls for runsTheTable (~30 lines)
└── components/canvas-tools/**     door lock/approve for runsTheTable         (~20 lines)

apps/web/e2e/
├── scene-trusted-player.spec.ts   SC-001, SC-002                            (~200 lines, new)
├── scene-ownership.spec.ts        SC-003, SC-005, SC-008                    (~180 lines, new)
└── scene-visibility.spec.ts       SC-004                                    (~120 lines, new)

docs/adrs/20260913-099-the_trusted_player_role.md   amendment paragraph
docs/SECURITY_RBAC.md, MVP.md                        corrected
```

## Risks

- **A caller of `is_dm_of_scene` missed.** Mitigated by renaming, so a
  missed caller does not compile.
- **A scene list that bypasses `scene_visible`.** T004 greps for every
  `hidden` filter in `queries/` and `subscriptions.rs` and lists the ones
  found before any gate moves; the visibility spec (SC-004) checks the
  switcher, the dock and the direct query.
- **The position-only check drifts from the engine's drag payload.** The
  Rust test sends the exact field set the engine's `updateToken` bridge
  sends (`apps/web/src/engine/world/sync/tokens.ts`), and the e2e moves a
  token through the real engine.
- **Backfill on a large deployment.** One `INSERT ... SELECT ... ON CONFLICT
DO NOTHING` over `worlds`; the index on `(world_id, user_id)` exists.
  Idempotent, so a re-run is safe.
- **Dropping `policies` breaks a user data export contract.** The fields were
  always 0 and `[]`; `user-data-export.spec.ts` is in the **accounts**
  neighbour and will say if a client depended on their presence.
- **ADR-099 amendment versus a new ADR.** Spec open question; the task
  appends a paragraph and the owner may ask for a number instead. Either is
  a document edit, not a code change.
