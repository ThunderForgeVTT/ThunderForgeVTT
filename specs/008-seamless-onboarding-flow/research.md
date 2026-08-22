# Phase 0 Research: Seamless Sign-Up-to-Canvas Onboarding Flow

## 1. Default scene creation: atomic server-side insert, not sequential client calls

**Decision**: Extend `create_world`'s existing GraphQL resolver (`src/server/src/graphql.rs:1182`) to insert one default `Scene` row in the same DB transaction as the world insert, reusing `create_scene`'s (`graphql.rs:969`) exact default values (`type: "battlemap"`, `grid_size: 5`, `grid_type: "square"`, `width`/`height: 100`, scene name = the world's name). No GraphQL schema/response change is needed — `createWorld`'s response type (`GraphQLWorld`) stays as-is.

**Evidence**: `create_world` currently does one bare `diesel::insert_into(worlds::table)...execute(...)` with no transaction wrapper. `create_scene` is a separate resolver with its own defaulting logic already proven in production (used by `SceneSwitcher`'s "New scene" dialog). `WorldPage.tsx`'s scene-gating logic (`scenes.length > 0 || isSceneOwner`, `WorldPage.tsx:678-679`) already does exactly the right thing the moment a world has ≥1 scene — it doesn't need to know *why* a scene exists, only that one does.

**Rationale**: A two-step client-side approach (call `createWorld`, then call `createScene` with the returned world ID) would leave a real window where a world exists with zero scenes if the second call fails (network drop, tab close) — reintroducing the exact "world with nothing to show" state this feature exists to eliminate, and doing so silently since nothing currently detects that state as an error. Wrapping both inserts in one DB transaction makes "a world without a scene" structurally impossible to create through this path, matching Constitution Principle III's data-boundary-integrity posture.

**Alternatives considered**: Client-side sequential calls with manual rollback-on-failure (delete the world if scene creation fails) — rejected as strictly worse than a DB transaction: more network round-trips, a real (if narrow) window of inconsistent state, and rollback logic to write and test that a transaction gets for free.

## 2. `/welcome` becomes a smart router, not a new route or an async `redirectAfterLogin`

**Decision**: `WelcomePage.tsx` queries `getMyWorlds()` (`apps/web/src/api/world.ts:89`, already used by `WorldListPage`) on mount. If the result is empty, it issues `navigate("/worlds/create", { replace: true })` immediately (no rendered hub content, no extra click — the user's browser briefly shows `/welcome` in the address bar before the replace, but nothing is rendered there). If non-empty, it renders the existing hub layout (fixed per Decision 4/5 below).

**Evidence**: `useAuth.ts`'s `redirectAfterLogin` (`useAuth.ts:122-127`) is a synchronous, role-only function called from multiple places in `AppRoutes.tsx`'s route-guard branches. Making it async (to check world count) would ripple into every one of those call sites and their surrounding render logic.

**Rationale**: Keeping `redirectAfterLogin`'s existing contract (`isAdmin ? "/admin" : "/welcome"`) untouched and pushing the world-count-aware branching into `/welcome`'s own component is the smallest-blast-radius way to satisfy FR-001/FR-001a — no new route, no `AppRoutes.tsx` changes, no signature change to a widely-called function.

**Alternatives considered**: Add a `hasWorlds` field to the session/auth response so `redirectAfterLogin` itself could branch synchronously — rejected as a larger, cross-cutting backend change (touching the session payload every authenticated request receives) for a decision only relevant immediately after login/registration.

## 3. Engine-load staged status: instrument the existing promise chain, no new dependency

**Decision**: Add an optional `onStageChange` callback parameter to `mountEngine`/`getWasmModule` (`apps/web/src/engine/bevy/index.ts`), firing `"downloading"` before the dynamic `import("@thunderforge/engine/engine")` and `"starting"` after it resolves but before `module.start(...)`. `useCanvasEngine.ts` exposes the current stage as part of its existing return object, alongside `engineReady`/`error`. `WorldPage.tsx` adds one new conditional render block for `!engineReady && !engineError`, showing the stage text — styled identically to the existing `data-testid="scene-load-indicator"` block (`WorldPage.tsx:632-645`) it sits alongside.

**Evidence**: `getWasmModule()`'s two await points (`await import(...)`, `await wasm.default()`) are the only real phase boundaries available — the browser's dynamic `import()` and wasm-bindgen's `default()` init function don't expose byte-level download progress through the APIs already in use here.

**Rationale**: The spec's own acceptance bar (FR-002: "visible loading indicator with status information," Assumptions: performance/duration is explicitly out of scope) is satisfied by staged status text + a spinner, not a numeric progress bar. True byte-progress would require replacing the dynamic `import()` with a manual `fetch()` + `ReadableStream` read loop and a custom instantiation path — a materially larger change for a requirement the spec doesn't ask for.

**Alternatives considered**: Manual streaming fetch with byte-count progress — rejected as disproportionate scope for this spec; noted as a possible future enhancement, not blocking here.

## 4. Create-world form: drop the fields, don't relabel them

**Decision**: `CreateWorldPage.tsx` removes the `gameSystemId`/`interfacePackId` state and their two `Select` components entirely (lines ~19-30, ~146-185, per plan.md's file map). The form becomes name + description only. `createWorld()`'s call site simply stops passing those two fields (the existing `GraphQLCreateWorldInput`/`prepare_world_input` already treat them as `Option<String>`, defaulting to `None` — confirmed no backend change needed).

**Evidence**: `GraphQLCreateWorldInput` (`input_types.rs:14-19`) already has `game_system_id`/`interface_pack_id` as optional; `prepare_world_input` (`helpers.rs:153+`) already handles `None` gracefully (only validates format *if* present). Per spec.md's clarification session, these were explicitly resolved to "removed from this flow entirely," not "kept but disabled."

**Rationale**: Directly implements the spec's own resolved clarification — no further decision needed here, just confirming zero backend changes are required to execute it.

## 5. Dashboard placeholder panels: fix per-panel, not a page rewrite

**Decision**: `WorldDashboardPage.tsx`'s six panels (`WorldDashboardPage.tsx:269-312` per the earlier audit) are addressed individually: panels backed by real, queryable data (Scenes — now always ≥1 after Decision 1; the world's own metadata) stay and show that real data; panels with no real backing data source yet (Actors, Tokens, Events, Game system, Interface pack) are removed from this screen rather than left as empty placeholders, consistent with US2's "no panel exists purely as an unfilled placeholder" requirement. The dashboard's *reachability* changes (Decision 2 — no longer a forced creation-time stop) but its own internal structure otherwise stays a single page, not a redesign.

**Rationale**: Spec.md's FR-006 explicitly scopes this to "every panel it shows MUST continue to reflect real, current data," not a broader dashboard feature build-out — removing dead panels satisfies that without inventing new backend capability (Actors/Tokens/Events panels would need real data sources this spec doesn't introduce).

## 6. Invite-code flow: one missing link in an otherwise-complete chain

**Decision**: Fix `LoginView.tsx`'s "Register" link (`LoginView.tsx:308`, currently a bare `to="/register"`) to preserve the current `location.search` (i.e. `to={`/register${location.search}`}`), so a `?returnTo=/join/xyz` query param survives the Login→Register hop. Add a manual invite-code entry field to `/welcome`'s hub (replacing the dead `to="/counter"` CTA), submitting via `navigate(`/join/${code}`)`.

**Evidence**: `/join/:code` (`AppRoutes.tsx`) is wrapped in `RequireAuthenticated`, which already redirects an unauthenticated visitor to `/login?returnTo=/join/xyz` (`AppRoutes.tsx`'s `RequireAuthenticated` component). Both `LoginView.tsx` (`:169, :205, :244, :480`) and `RegisterPage.tsx` (`:90, :230`) already independently honor `redirectTarget(location.search)` — a `returnTo` param present on either page correctly routes back to it after success. The **only** broken link in this otherwise-complete chain is that clicking "Register" from the login page drops the query string entirely.

**Rationale**: This is the minimal fix — one link's `to` prop — rather than building new invite-preservation plumbing from scratch; the existing `returnTo` convention (already used for the unrelated `RequireAuthenticated` redirect case) already does everything FR-012 needs once the one missing hop is patched.
