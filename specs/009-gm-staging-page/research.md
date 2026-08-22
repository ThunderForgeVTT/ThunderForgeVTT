# Phase 0 Research: GM Staging Page and Full-Screen Play Canvas

All items below were resolved by reading the actual current implementation (`apps/web/src/pages/world/WorldPage.tsx`, `apps/web/src/layouts/world-layout/WorldLayout.tsx`, `apps/web/src/engine/bevy/`, `src/server/src/graphql/queries/`, `src/server/src/schema.rs`), not by assumption. No `[NEEDS CLARIFICATION]` markers remain in `plan.md`'s Technical Context.

## 1. How does the staging↔full-screen toggle avoid breaking the already-booted WASM engine?

**Decision**: The staging↔full-screen toggle is a local UI-state boolean in `WorldPage.tsx` (`"staging" | "playing"`), not a route change and not a conditional unmount of the canvas container. The `<div id="game-canvas-container">` element stays permanently mounted in the DOM; only its CSS layout (and the staging page's own DOM) changes around it — e.g. the canvas container is styled to fill the viewport and be visually on top when `"playing"`, and styled/positioned behind or hidden-but-present when `"staging"`.

**Rationale**: `apps/web/src/engine/bevy/index.ts`'s `mountEngine` guards the actual Bevy start call with `if (!state.started) { module.start(options.canvasSelector); state.started = true; }` — this only ever runs once per page load, and `module.start` binds to the DOM element matching `canvasSelector` at that moment. If the container were conditionally rendered away (e.g. `{stagingMode ? <Staging/> : <div id="game-canvas-container">}`), React would unmount and later remount a *new* DOM node with the same id, but the already-started Bevy instance has no re-attach path — `useCanvasEngine.ts`'s own comment already documents this general expectation ("Engine persists across component unmounts / Cleanup on unmountEngine() call if needed"), confirming the container is meant to be a stable, long-lived mount point, not something toggled in and out of the tree.

**Alternatives considered**:
- Route-based toggle (`/world/:id/play` → `/world/:id/play/live` or similar): rejected — a route change unmounts the page component tree by default, which would either force a costly full engine re-init or require lifting engine state above the router (much larger change than this feature's scope).
- Conditionally rendering the container: rejected for the reason above (stale DOM handle risk) — confirmed by reading `mountEngine`'s `state.started` guard rather than assumed.

## 2. Where does the staging page's NPC roster data come from, given `world_actors` has no read query today?

**Decision**: Add `worldActors(worldId: Uuid) -> [WorldActor]` to a new `graphql/queries/actor.rs` module (`ActorQuery`), filtered to `world_actors.world_id = $worldId`, authorized with the same `require_visible_world` check `scenes(worldId)` already uses (`graphql/queries/scene.rs`). The staging page and sidebar's NPC section then client-side-filter (or the query itself filters) to `is_npc = true` — the underlying table already covers both NPCs and player characters via `is_npc`/`actor_type`, so no new column or table is needed.

**Rationale**: Confirmed via `src/server/src/schema.rs`'s `world_actors` table definition (`is_npc: Bool`, `world_id: Uuid`, `scene_id: Uuid` all present) and via grep across `src/server/src/graphql*.rs`/`graphql/*.rs` that the only existing `world_actors` touchpoint is `update_actor_system_data` (a mutation gated on `owned_by`, requiring the actor id already be known) — there is no query that lists actors for a world or scene at all. `scenes(worldId)` (world-scoped, not scene-scoped, despite scenes themselves being sub-world objects) is the closest existing precedent and is followed exactly: same auth helper, same "list by world_id" shape, same file-per-domain organization (`graphql/queries/<domain>.rs` merged into `QueryRoot`).

**Alternatives considered**:
- Scene-scoped query (`actorsForScene(sceneId)`), matching how tokens/walls/lights are scene-scoped: considered, since `world_actors.scene_id` is NOT NULL (every actor belongs to exactly one scene). Rejected as the *only* query in favor of world-scoped, because the spec's staging page shows "the world's NPC roster" as one roster, not scoped to whichever scene happens to be selected — a GM staging a session wants to see all of a world's NPCs, not just one scene's. World-scoped filtering by `world_id` returns this directly without requiring N scene-scoped calls, and matches the `scenes(worldId)` precedent exactly.
- New dedicated `Npc`/`NpcRoster` entity: rejected — `world_actors.is_npc` already fully represents this distinction; a new entity would duplicate existing data (spec's own FR-003 and the user's original request both explicitly warn against this).

## 3. How does the frontend determine whether the current user is GM/Owner (for gating staging-page and sidebar controls)?

**Decision**: Reuse the existing `worldMembers(worldId)` data (already fetched via `useWorldMembers.ts`'s RxDB-backed hook, used for the player roster) — find the entry where `user_id === current user's id` and read its `role`. Treat `role === "Owner" || role === "GM"` as GM-equivalent for gating, with the same `world.createdBy === user.id` fallback `WorldPage.tsx`'s existing `isSceneOwner` already uses for the case where a `world_members` row doesn't exist yet for the owner (mirroring `require_world_member`'s own server-side fallback, per `graphql/queries/invite.rs`'s comment on `require_world_member`).

**Rationale**: `WorldPage.tsx`'s current `isSceneOwner = Boolean(world && user && world.createdBy === user.id)` only recognizes the world's original creator, not an invited co-GM (a `world_members` row with `role = "GM"`) — this is a real, narrower-than-backend gap discovered while reading the code (the backend's `world_invites` query already treats `role != "Owner" && role != "GM"` as insufficient, i.e. it already supports co-GMs; the frontend's owner-only check does not). This feature's staging-page role gating is the first place that needs a correct GM check, so it is fixed here rather than deferred, by widening the check to also match a `world_members` row with `role === "GM"`. This is a small, targeted correction of an existing gap the spec's FR-012 depends on — not a new authorization surface (the server remains authoritative for every actual mutation regardless of what the client shows/hides).

**Alternatives considered**:
- Add a new dedicated `currentUserRole(worldId)` GraphQL query: rejected — `worldMembers(worldId)` already returns every member including the current user; a second round-trip for the same data already in hand is unnecessary.

## 4. What does "engine-load feedback" (spec 008's downloading/starting indicator) do differently in this feature's two-state layout?

**Decision**: Unchanged behavior, relocated only. The existing `engine-load-indicator`/`scene-load-indicator`/`scene-load-error` overlays in `WorldPage.tsx` (spec 008) already render inside the canvas container and are keyed off `engineReady`/`sceneLoadState`, independent of any layout wrapper. Since the canvas container stays permanently mounted (Decision 1), these indicators continue to work unmodified in full-screen mode; they are simply never visible while the user is on the staging page (the container sits behind/hidden then), and become visible the moment the user transitions to full-screen mode if the engine/scene are still loading at that point.

**Rationale**: Read directly from `WorldPage.tsx`'s existing JSX — these overlays are children of `#game-canvas-container`, not of `WorldLayout`, so they are unaffected by the layout restructuring as long as Decision 1 holds (container never unmounts).

**Alternatives considered**: None needed — this falls out directly from Decision 1 rather than requiring an independent choice.
