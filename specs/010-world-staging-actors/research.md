# Phase 0 Research: World Staging Route and Actor Ownership

All items below were resolved by reading the actual current implementation (`apps/web/src/pages/world/WorldPage.tsx`, `apps/web/src/layouts/world-layout/WorldStagingPage.tsx`, `apps/web/src/pages/user/WelcomePage.tsx`, `apps/web/src/pages/world/WorldDashboardPage.tsx`, `apps/web/src/routes/AppRoutes.tsx`, `src/server/src/schema.rs`, `src/server/src/graphql.rs`, `src/server/src/graphql/queries/actor.rs`, `src/server/src/graphql/mutations_tokens.rs`, `src/server/src/graphql/mutations_invites.rs`, `src/server/src/auth/world_membership.rs`), not by assumption. No `[NEEDS CLARIFICATION]` markers remain in `plan.md`'s Technical Context.

## 1. How does "staging" move from a UI state inside `/play` to its own route without breaking the WASM engine mount invariant spec 009 established?

**Decision**: `/world/:id/staging` becomes a new, ordinary routed page (`pages/world/WorldStagingRoutePage.tsx`), nested inside the app's normal `MainLayout` route tree (same tier as `/world/:id`, `/worlds`, etc.) — it never touches the Bevy canvas container at all. `/world/:id/play` (`WorldPage.tsx`) drops its `playView: "staging" | "playing"` state entirely and always renders the full-screen canvas shell (`WorldLayout`) directly, exactly as it does today when `playView === "playing"`. The existing `<WorldStagingPage>` *component* in `layouts/world-layout/WorldStagingPage.tsx` (props-driven, no routing of its own) is reused as-is as the presentational body of the new route page — only its `onPlay` prop changes, from `setPlayView("playing")` to `navigate(`/world/${worldId}/play`)`.

**Rationale**: Spec 009's hard constraint (`research.md` §1 there) was that the `#game-canvas-container` DOM node must never unmount/remount because Bevy's `mountEngine` only calls `module.start(canvasSelector)` once per page load and has no re-attach path. That constraint is about the canvas container specifically — it says nothing about the staging UI, which today lives in the same component tree only because spec 009 chose a UI-state toggle over a route change *for the canvas's sake*, not the staging page's. Since the new design puts staging at an earlier, separate route (reached from `/welcome`, not from `/play`), the canvas container is never mounted until the user actually navigates to `/play` — so there is no remount risk at all; the constraint is trivially satisfied by staging simply not sharing a component tree with the canvas anymore.

**Alternatives considered**:
- Keep spec 009's UI-state toggle and just add a redirect wrapper at `/staging` that renders `WorldPage` with an initial state: rejected — this keeps the canvas-container-mounting cost (and the ~190MB WASM module's lifecycle) tied to a page the user may never advance past in a given visit, and re-adds exactly the coupling spec 009's own migration note in spec.md says to remove.
- Route-based sub-state (`/world/:id/play?view=staging`): rejected as unnecessary complexity now that staging has its own real route; no reason to keep a second staging concept nested under `/play`.

## 2. Does the new `/staging` route conflict with the existing `/world/:id` dashboard page?

**Decision**: No — they stay distinct and both remain. `/world/:id` (`WorldDashboardPage.tsx`) is the world's administrative home (metadata, `CampaignSettingsPanel`'s invite/roster/role management, delete world) — untouched by this feature. `/world/:id/staging` is specifically the actor-catalog-plus-"Play" pre-session screen this spec defines. `/welcome`'s "Enter {world.name}" link changes from `/world/:id/play` to `/world/:id/staging` (FR-002); the World Archive's link to `/world/:id` for management is unaffected.

**Rationale**: Read directly from `WorldDashboardPage.tsx` (315 lines, hosts `CampaignSettingsPanel`) — confirmed no actor-roster content lives there today, so there's no existing responsibility to migrate or deprecate.

**Alternatives considered**: Folding the actor catalog into `WorldDashboardPage` instead of a new route: rejected — the spec explicitly calls for a dedicated `/staging` route reached from "Enter," distinct from the dashboard's admin/settings framing, and reused as the "Play" launch point (FR-001, FR-006), which the dashboard has no reason to become.

## 3. How is "DM" (Owner or GM) checked, and does an existing gap need fixing?

**Decision**: Reuse `require_world_member`/`world_members.role` exactly as spec 009 already fixed it (research.md §3 there): role string `"Owner"` or `"GM"` (with the `worlds.created_by` fallback for a world owner missing a `world_members` row) is "the DM" for every FR-021-governed check in this feature (actor creation, ownership-block edits, share-link revocation-by-DM). No new role, no new check — a single shared helper (`is_dm_of_world(state, user_id, world_id) -> bool`) wraps the existing role lookup so every new mutation in this feature calls one function instead of re-deriving the Owner/GM check.

**Rationale**: `src/server/src/auth/world_membership.rs::require_world_member` already returns the role string with the correct owner-fallback; `graphql/queries/invite.rs`'s Owner/GM check is the established precedent spec 009's own Assumptions cited. No gap remains to fix here (unlike spec 009, which found and fixed a frontend-only gap) — this feature only needs to consume the existing, already-correct backend behavior consistently across several new mutations.

**Alternatives considered**: A new `is_dm` boolean column on `world_members`: rejected — redundant with the existing role string and would create two sources of truth for the same fact.

## 4. What replaces `world_actors.owned_by` (single, non-null `Uuid`) as the authorization source for actor edits?

**Decision**: A new table, `world_actor_permissions` (actor_id, user_id, level), is the sole source of truth for Viewer/Editor/Owner going forward. The existing `world_actors.owned_by` column is kept (still `NOT NULL`, still populated — set to the creating DM's `user_id` at creation, per `NewWorldActor`) purely for backward-compatible read/display purposes (it's already exposed as `ownedBy` on `GraphQLWorldActor`, which nothing currently un-exposes), but it stops being consulted for any authorization decision. The one existing mutation that currently gates on it — `update_actor_system_data`'s "Verify user owns the actor" check (`world_actors::owned_by.eq(user_id)`, `src/server/src/graphql.rs:860`) — is changed to call a new shared helper, `require_actor_permission(state, user_id, actor_id, minimum: ActorPermissionLevel)`, which resolves the effective level as: DM of the actor's world → always `Owner` (FR-017); else the caller's explicit `world_actor_permissions` row for that actor, if any; else `Viewer` (FR-016) — and rejects if that's below `minimum`.

**Rationale**: Confirmed via grep that `world_actors.owned_by` has exactly one authorization consumer in the whole codebase (`update_actor_system_data`) plus two read-only exposures (the Rust struct field and the GraphQL field) — a small, well-contained migration surface. Keeping the column (rather than dropping it) avoids a breaking schema change to a `NOT NULL` field with existing rows and avoids touching the `NewWorldActor` insert path's required-fields shape more than necessary.

**Alternatives considered**:
- Repurpose the existing (but dead/commented-out) `policies` table (`effect`, `resources: Text[]`, generic ABAC-style) as the permission store: rejected — its Rust struct is entirely commented out, nothing in the codebase reads or writes it today, and adapting a generic allow/deny-list engine to a specific "one of three ordered levels per (actor, member)" shape would be *more* code than a small, purpose-built table, for a system with zero current runtime behavior to build on.
- Make `owned_by` nullable and repurpose it as "the current default Owner" while adding a separate multi-row table only for extra grants: rejected — the spec explicitly allows *multiple* simultaneous Owners (clarification: "Allow multiple Owners"), so a single-value column can't represent the Owner set at all, even as a partial answer; better to have one clear source of truth.

## 5. How does FR-018 (only an Owner-level actor permission — or the DM — controls the actor's token in live play) fit the *existing*, already-shipped token-drag authorization?

**Decision**: `tokens.owner_user_id` (nullable `Uuid`) already gates token-drag/move today (`mutations_tokens.rs`: "a player may drag any token whose `owner_user_id` is them"). This feature does not replace that mechanism or add a second one — it *extends* the existing check with an OR-condition: a caller may also act on a token if `tokens.actor_id IS NOT NULL` and the caller holds effective `Owner` permission (via `require_actor_permission`, §4) on that `actor_id`. Tokens with no linked actor (props, lights, hazards) are completely unaffected — they keep exactly today's `owner_user_id`-only behavior.

**Rationale**: `tokens` already has a nullable `actor_id` FK (confirmed in `schema.rs`) sitting unused by the drag-authorization path today — it's already the join key needed to connect "which actor does this token represent" to "who holds Owner on that actor," so no schema change to `tokens` is needed. This also cleanly resolves the "multiple simultaneous Owners" case (clarification, `spec.md`) without needing `owner_user_id` to become multi-valued: any of the actor's current Owner-level members satisfies the check independently, and "most recent action wins" (the spec's stated conflict resolution) falls out for free from each drag mutation re-checking permission independently rather than caching a single controller.

**Alternatives considered**: Writing `tokens.owner_user_id` to reflect the actor's (single) Owner whenever the ownership block changes, keeping the existing check unmodified: rejected — breaks as soon as an actor has more than one Owner-level member (which the spec explicitly allows), since `owner_user_id` can only hold one value.

## 6. Where does a staging-page-created NPC get its required (`NOT NULL`) `scene_id`?

**Decision**: `createActor(worldId, ...)` assigns the new actor to the world's earliest-created scene (`scenes` ordered by `created_at asc`, `LIMIT 1`), the same scene `create_world`'s own invariant test (`create_world_always_yields_exactly_one_scene`) guarantees exists for every world at creation time. The DM can move/reassign the actor to a different scene later via the existing scene-editing surface (out of scope for this feature to add a picker to the creation form itself, since the spec's roster is explicitly world-scoped, not scene-scoped).

**Rationale**: `world_actors.scene_id` is `NOT NULL` in `schema.rs`; `create_world` is proven (by an existing test) to always leave at least one scene behind, so this default is always resolvable — no "world with zero scenes" edge case exists to handle for a freshly-created actor.

**Alternatives considered**: Adding a scene picker to the "add NPC" control on the staging page: rejected as unnecessary scope for this pass — the spec's roster and its "add NPC" action are explicitly world-scoped (FR-003/FR-004), and scene assignment is a pre-existing, separately-editable concern.

## 7. How is FR-022's cascade-delete (ownership entries removed when a member leaves the world) implemented?

**Decision**: Application-level, not a database `ON DELETE CASCADE` FK. `world_actor_permissions.user_id` references `users(id)` (a user can belong to many worlds), not a specific `world_members` row — there is no direct FK from a world membership to this table for the database to cascade through. Instead, the existing `removeMember`/leave-world mutation path (`mutations_invites.rs`) is extended to, in the same transaction, delete every `world_actor_permissions` row where `user_id = <removed user>` AND `actor_id IN (SELECT id FROM world_actors WHERE world_id = <this world>)`.

**Rationale**: `world_actor_permissions.actor_id` does carry a real FK to `world_actors(id)`, which does carry `ON DELETE CASCADE` value for the *actor-deleted* case (deleting an actor should always delete its permission rows — that part *is* a plain DB-level cascade) — but "world member removed" and "actor deleted" are different triggers requiring different cleanup, so only the actor-FK cascade is DB-level; the membership-removal cleanup is an explicit deletion statement colocated with the existing removal mutation.

**Alternatives considered**: Adding a `world_id` + `user_id` composite FK on `world_actor_permissions` pointing at a hypothetical `(world_id, user_id)` unique constraint on `world_members`, to get a DB-level cascade: rejected — `world_members` today is keyed by its own surrogate `id`, and re-keying it (or adding a new unique constraint plus a second FK path) is a much larger, riskier schema change than one additional `DELETE ... WHERE` in the existing removal mutation's transaction.

## 8. Where does "worlds where I hold DM-level access" (for the Copy-to-World destination picker, FR-025) come from?

**Decision**: A new query, `myDmWorlds`, added alongside the existing `myWorlds` (`load_owned_worlds`, `graphql/queries/user.rs`) rather than changing `myWorlds`'s behavior. `myDmWorlds` returns the union of: worlds where `worlds.created_by = caller` (Owner), and worlds where an accepted `world_members` row exists for the caller with `role = 'GM'`.

**Rationale**: Confirmed by reading `load_owned_worlds` that today's `myWorlds` — the same query backing `/welcome`'s "Enter" list — filters strictly on `worlds.created_by`, so it already does not include worlds where the caller is only an invited GM (a real, pre-existing gap, but outside this spec's stated scope to fix for the general welcome-hub list). Rather than risk changing `myWorlds`'s existing semantics for callers who depend on today's behavior, this feature adds one small, purpose-built query scoped to exactly what the Copy-to-World picker needs.

**Alternatives considered**: Widening `myWorlds` itself to include GM-role worlds: rejected for this pass — it's a larger, cross-cutting behavior change to an existing, already-consumed query (`WelcomePage.tsx`) that this spec does not need and was not asked to make; flagged here only as a known adjacent gap, not fixed.

## 9. How does the public-by-code shared-actor view avoid leaking the source world's identity?

**Decision**: The `sharedActor(code)` query returns a narrow, purpose-built `SharedActorPreview` type containing only the actor's own content fields (label, classification, `actorType`, `gameSystemId`, and its `world_actor_system_data` payloads) — it does not include `worldId`, `sceneId`, `createdBy`, or `ownedBy`. The share-code itself is an opaque random token (same generation approach as `world_invites.invite_code`), carrying no encoded world/actor id.

**Rationale**: Nothing in the spec requires the viewer to know which world/DM an actor came from, and the whole point of sharing (per the spec's framing — "if I build a great NPC I want to share it") is the actor's content, not its origin; keeping the origin world unaddressable through this path is a small, free privacy property consistent with Principle III's authorization-at-the-boundary spirit even though no requirement forced it.

**Alternatives considered**: Reusing the existing `GraphQLWorldActor` type directly for the preview: rejected — it already includes `worldId`/`sceneId`/`createdBy`/`ownedBy`, which would leak the source world's identifier to an arbitrary logged-in stranger holding the link.
