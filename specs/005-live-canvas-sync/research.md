# Phase 0 Research: Live Cross-Client Canvas Sync via GraphQL Subscriptions

## 1. Server-side transport is already fully built — confirms the spec's assumption, with one important correction

**Decision**: Treat the server as substantially done, per the spec's assumption — but with one required fix (research.md §2), not zero changes as originally assumed.

**Evidence**:
- `src/server/src/main.rs:71-83` (`graphql_ws_handler`) already upgrades `/api/ws` to a `GraphQLWebSocket`, supporting `ALL_WEBSOCKET_PROTOCOLS` (both `graphql-ws` and the older `subscriptions-transport-ws` protocol), and already wires `on_connection_init` to inject the authenticated user (`AuthenticatedUser`, from the same `require_authenticated_user` middleware that gates `/graphql`) into the subscription's `Data` context.
- `main.rs:253-258`: the `/ws` route sits under the identical `route_layer(from_fn_with_state(..., auth_middleware::require_authenticated_user))` as `/graphql` — so the WebSocket upgrade itself is already cookie-authenticated before `on_connection_init` even runs. No client-side auth token/handshake payload is needed; the browser's normal same-origin cookie is sent automatically on the WS upgrade request, exactly like today's `fetch`-based GraphQL calls (`apps/web/src/api/walls.ts`'s `credentials: "same-origin"`).
- `src/server/src/graphql.rs:1423-1500` (`SubscriptionRoot::world_events_created`) already streams from `AppState.world_event_sender` (a `tokio::sync::broadcast::Sender<WorldEvent>`), filtering by `world_id`, and already handles a lagging/backpressured subscriber gracefully (drops and logs rather than crashing).
- `src/server/src/world_events.rs` already defines `EVENT_CODE_WALL_CHANGED` (10), `EVENT_CODE_LIGHT_SOURCE_CHANGED` (11), `EVENT_CODE_SHAPE_CHANGED` (12), `EVENT_CODE_MAP_IMPORTED` (13), and `EVENT_CODE_TOKEN_CHANGED` (14) — all four entity types this feature's User Stories 1 and 3 need are already emitted today, on every relevant mutation.

**Rationale**: This confirms specs 003/004's finding precisely: there is a fully working, already-authenticated, already-tested-shape subscription server sitting unused because no client anywhere in `apps/web` ever opens a WebSocket to `/api/ws`. The fix is entirely additive on the client.

## 2. Found during research: `world_events_created` has no world-membership check — a real authorization gap that must be closed as part of this feature

**Decision**: Add a world-membership check to `world_events_created` before this feature activates real client usage of it. This is a required correction, not optional hardening — it's a Constitution Principle III violation waiting to happen the moment a real client starts calling this resolver.

**Evidence**: `graphql.rs:1452-1462` — the subscription's only validation is `uuid::Uuid::parse_str(&world_id).ok()` (a format check) and matching `event.world_id == world_uuid` on the stream filter. It never checks that the authenticated user (available in `ctx` via `on_connection_init`'s injected `AuthenticatedUser`, exactly like `graphql_handler`'s query/mutation path already does) is actually a member of that world. Today this is harmless dead code — nothing calls it. The moment this feature makes every connected client subscribe to `worldEventsCreated(worldId)`, an authenticated user could pass *any* world's UUID and receive that world's wall/light/shape/token change stream, regardless of membership.

**Rationale**: Every other data-boundary path in this codebase (queries, mutations) enforces ownership/membership server-side per Constitution Principle III — `mutations_walls.rs`'s scene-owner filter, `world_members`-based checks elsewhere. A subscription is exactly as much a data boundary as a query, and must be held to the same bar before this feature makes it load-bearing.

**Fix shape**: Inside `world_events_created`, before subscribing, check the requesting user (from `ctx.data::<AuthenticatedUser>()`) is a member of `world_id` (reusing whatever existing `world_members` lookup query already backs query/mutation authorization elsewhere in `graphql.rs`/`mutations_*.rs`), returning an authorization error (matching the resolver's existing `Err(Error::new(...))` early-return pattern for the format-validation case) if not.

**Alternatives considered**: Filtering client-side after receipt (client silently discards events for worlds it shouldn't see) — rejected outright; that leaks other worlds' scene data to the network/browser of a user who shouldn't have it, which is exactly the class of bug Principle III exists to prevent, not just a UX nicety.

## 3. Client transport: no existing GraphQL client library in `apps/web` — cookie auth simplifies the choice

**Decision**: Add a minimal subscription client (e.g. `graphql-ws`'s client half, or an equivalently small hand-rolled WebSocket wrapper) as the one new client dependency this feature introduces. Because auth is already cookie-based and flows automatically with the WS upgrade request (research.md §1), the client needs no `connectionParams`/token-passing logic — simpler than the typical graphql-ws auth setup tutorials assume.

**Evidence**: `apps/web/src/api/walls.ts:1-40` and its siblings (`lights.ts`, `shapes.ts`, `tokens.ts` presumably) are all plain `fetch`-based POST helpers with no GraphQL client library anywhere in `apps/web/package.json` (confirmed absent: no `apollo`, `urql`, `graphql-ws`, `subscriptions-transport-ws`, or any `WebSocket`/`ws://` usage anywhere in `apps/web/src`).

**Rationale**: Matches the spec's own Assumption that library choice is an implementation-time decision; this research narrows it to "whichever minimal client speaks `graphql-ws` (or the legacy) protocol against `/api/ws`," since that's what the server already implements via `ALL_WEBSOCKET_PROTOCOLS`.

**Alternatives considered**: Full Apollo Client — rejected as introducing an entire GraphQL client/cache layer this codebase's `fetch`-based convention doesn't use anywhere else, when only the subscription half is actually needed.

## 4. Reconnect/resync: reuse spec 004's per-scene loader functions, not new fetch logic

**Decision**: The "full re-fetch on reconnect" clarified in spec.md reuses the exact same `loadWallsIntoStore`/`loadLightsIntoStore`/`loadShapesIntoStore`/`loadTokensIntoStore` functions spec 004's research already identified in `apps/web/src/pages/world/WorldPage.tsx` (lines ~282, ~298, ~314, ~340) — the same functions a manual page reload already triggers. No new fetch/query code is needed for the resync path itself, only the retry/backoff orchestration around *when* to call them.

**Rationale**: Avoids a second, parallel "refetch everything" implementation diverging from the one a plain reload already uses; keeps exactly one code path responsible for "load a scene's full state," consistent with not duplicating logic across spec 004 and this feature.

**Coordination note**: This feature and spec 004 both touch `WorldPage.tsx` (spec 004 for its loading/error state machine, User Story 4; this feature for wrapping reconnect-triggered re-invocation of the same loaders). These should be sequenced or coordinated at implementation time to avoid the same file diverging under two specs simultaneously — not a blocking dependency (either can be built first), but a real file-overlap to flag.

## 5. Reconnect backoff and idempotent event application — standard patterns, no novel design needed

**Decision**: Standard exponential backoff (e.g. 1s, 2s, 4s, 8s... capped at some ceiling like 30s) for reconnect attempts, per the clarified "retry indefinitely" answer. Idempotent event application: the existing `upsert_wall`/`upsert_light`/`upsert_shape`/`upsert_token` world-store dispatch actions (already used by the outbound mutation bridges per spec 003/004 research) are naturally idempotent — applying the same upsert twice with identical data is a no-op in effect, so no new deduplication mechanism is needed beyond what already exists.

**Evidence**: `walls.ts`'s `applyWallWorldEvent` (per spec 003/004 research) already dispatches `upsert_wall`/`remove_wall` — the same actions the outbound bridge's confirmed-mutation dispatch uses. Both paths converging on the same idempotent upsert action is what makes FR-007 (no double-apply/flicker) achievable without new reconciliation logic.

**Rationale**: This is exactly why FR-005 requires reusing the existing inbound consumer functions rather than writing new ones — they already have the right idempotency property built in from spec 001's original design.

## 6. Constitution Principle IV: ADR recommended for the new client-side transport subsystem

**Decision**: Author an ADR documenting the adoption of a GraphQL subscription client transport in `apps/web` — this is the first real-time client transport dependency this codebase has ever had client-side (the server has had subscription support since an earlier phase per `graphql.rs`'s doc comments, but no client has used it).

**Rationale**: Per Constitution Principle IV, a new subsystem/dependency of this kind (the app's first live WebSocket client) is exactly the trigger condition named — even though the change is additive and low-risk, the *category* of capability (a persistent, reconnecting network channel independent of the request/response `fetch` pattern used everywhere else) is architecturally new for the frontend and should be recorded, not merely implemented.

**Scope of the ADR**: which client library was chosen and why, the auth-via-cookie-on-upgrade approach (research.md §1/§3), the full-refetch-on-reconnect resync strategy (research.md §4), and the `world_events_created` authorization fix (research.md §2) as a corequisite.

## 7. Invite/membership fix (User Story 4, folded in post-planning): two concrete, already-diagnosed bugs

**Decision**: Fix both bugs directly; no design work needed, since both are confirmed, narrow defects against an already-correct intended design.

**Evidence**:
- `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`'s `handleGenerateInvite` sends `generateInviteCode(worldId: $worldId, maxUses: $maxUses)` as flat top-level GraphQL arguments; `src/server/src/graphql/mutations_invites.rs:134`'s `generate_invite_code(ctx, input: GenerateInviteCodeInput)` expects a single `input` object. Every invite-generation call from the UI fails today with an argument-shape error.
- `mutations_invites.rs`'s authorization check queries `world_members::table.filter(world_id).filter(user_id)` for the caller — but `src/server/src/graphql.rs:1182-1213`'s `create_world` never inserts a `world_members` row for the world's own owner. Even with the client fixed, the world's own owner would fail their own world's invite-authorization check.

**Rationale**: These are the reason spec 003's live verification found T006 (a genuine non-owner player test) unreachable, and the reason both the unit-test gap analysis (zero coverage on invite mutations) and the e2e gap analysis (no test anywhere exercises a real second account) flagged the same root cause independently. Fixing both directly — not redesigning the invite/membership model — closes a project-wide test-coverage blind spot, not just this feature's own testing.

**Alternatives considered**: Leaving this as a separately-tracked bug ticket instead of folding into this spec — rejected per direct instruction, since this feature's own User Story 1 benefits directly from being verifiable with a real second account, and the fix is small enough not to warrant its own full spec/plan/tasks cycle.
