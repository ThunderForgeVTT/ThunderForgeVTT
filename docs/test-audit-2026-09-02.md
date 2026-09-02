# Tests that cannot fail — audit, 2026-09-02

Every finding below was proven by **mutation**: the thing under test was
broken, the test re-run, and the result recorded. A test that stayed green with
its subject broken is a confirmed finding. Suspicions that failed the mutation
are listed at the end, because knowing a test is real is worth as much as
knowing one is not.

This is a list, not a change. Fix them deliberately — several are load-bearing
and one is a security primitive.

## Why this was done

The pattern has bitten this repository repeatedly:

- Spec 031 found two e2e specs asserting an NPC form was *absent*. The form had
  been deleted from the codebase, so both passed while testing nothing.
- 2026-09-02: an e2e asserted a response did not contain `Unknown field
  "fraction"`. The endpoint returned an empty 401 body, and an empty string
  contains nothing — it passed, and would have passed with the field deleted.
- 2026-09-02: `a_system_with_no_declared_resources_publishes_none` was written
  against Fate Core, which declared none at the time. Fate declares them now
  and the test stayed green, because its fixture happened to supply no resource
  data. The assertion was still true; its stated reason had become false.
- `ability_modifier` was tested at 10, 12, 8 and 16 — even scores plus one odd
  score above ten, exactly the set that cannot detect the truncation bug the
  function had for every odd score below ten.

## Fixed so far

| Finding | Fixed in | Mutation now fails |
|---|---|---|
| 1. Deny-by-default never tested | `src/core/src/policies/mod.rs` | ✓ `Default` → `Allow` |
| 2. Rate-limit bypass tested a copy | `src/server/src/auth_middleware.rs` | ✓ `bypass_requested` → `true` |
| 5. Boundary-blind validators (3 packs) | `yze`, `genie`, `dnd5e` validators | ✓ range narrowed one step each end |
| 7. Name promised coverage it lacked | `src/core/src/policies/mod.rs` | ✓ `remove` gutted to a no-op |
| Web: `missing` reported for a loaded pack | `appearance-context.test.ts` | ✓ `missing = requestedId` |
| Web: CSRF never asserted | `api/__tests__/graphqlClient.test.ts` | ✓ both `withCsrf(...)` call sites stripped |
| 6. Vacuous e2e absence assertions | seven spec files | nine replaced/removed; **all 35 tests re-run green** |
| 3. Engine suite never built | `src/engine/` — see below | **done: 0 tests → 192, all passing** |
| 4. Green ticks asserting nothing | `tests_f1_unit.rs`, `tests_f2_f4_integration.rs` | deleted |
| 8. Fifteen empty test bodies | 7 packs × server + engine, `map_import` | ✓ submission removed; ✓ `rules` dropped; ✓ plugin reads an uninserted resource |

**A note on fixing these.** The first attempt at the Year Zero boundary test
used `super::ABILITY_MIN` and `super::ABILITY_MAX`, which made it assert that
the rule accepts whatever the rule is written against — true of every range,
and it passed the very mutation it was written to catch. A boundary test has to
name the boundary as a literal. Worth knowing that writing a test for this
class of bug is itself prone to it.

## Confirmed, ranked

### 1. Deny-by-default is never tested — `src/core/src/policies/mod.rs`

`it_should_deny_on_default` claims the authorisation primitive denies by
default. **Mutation**: `Default for Policy` changed to `effect: Allow`. **All
five policy tests still passed.** The fixture is a policy with *zero
resources*, so `can_i` returns false through "nothing matched", never through
the effect. Every test that would observe `can_i == true` is commented out.

The authorisation primitive can be wholly inverted with a green suite. Fix
first.

### 2. The rate-limit bypass test tests a copy of the code — `src/server/src/auth_middleware.rs`

`the_bypass_stays_shut_unless_the_variable_says_otherwise` defines a local
closure that **re-implements** the env-var parsing and asserts against that.
**Mutation**: `rate_limit_disabled()` forced to `true` — brute-force protection
permanently off in every debug build. **Both bypass tests passed.** Nothing
exercises `rate_limit_auth_requests` at all.

Its companion `a_release_build_cannot_be_bypassed_at_all` has its whole body
inside `#[cfg(not(debug_assertions))]`, so under `cargo test` it is an empty
test asserting a property no normal run observes.

### 3. The engine's Rust test suite has never run — `src/engine/**`

`cargo test -p thunderforge_engine` fails to build: winit does not support the
host. Roughly 45 tests across `tests_f1_unit.rs`, `tests_f2_f4_integration.rs`,
`sync_test.rs` and `integration_tests.rs` are uncompilable here. Individual
mutation proofs were impossible *because the suite cannot be built*, which is
the stronger finding.

`scenario_mutation_timeout_and_rollback` inside it has **zero assertions**: it
binds `check_timeouts(6.0)`, never uses it, and unconditionally prints
"✅ Scenario 4 passed".

### 4. Green ticks that assert nothing — `tests_f1_unit.rs`, `tests_f2_f4_integration.rs`

`test_suite_coverage` and `test_suite_coverage_f2_f4` are pure `eprintln!`.
The first prints "✅ 50+ unit tests implemented"; the file contains 33. The
message is already false and would stay green if every test it names were
deleted.

### 5. Boundary-blind validators — the `ability_modifier` pattern, three more times

Each proven by narrowing the accepted range one step at both ends; every test
in each pack still passed.

| File | Declared range | Mutation | Result |
|---|---|---|---|
| `packs/systems/year_zero_engine/server/src/validators.rs` | 1–5 | 2–4 | 3/3 pass |
| `packs/systems/genie/server/src/validators.rs` | level 1–10 | 2–9 | 20/20 pass |
| `packs/systems/dnd5e/server/src/validators.rs` | ability 1–20 | 2–19 | 37/37 pass |

Each has reject-below, reject-above and accept-in-range, but the accept fixture
uses interior values only. `pathfinder2e` and `blades_in_the_dark` test their
exact boundaries — copy those.

### 6. Six e2e assertions whose subject exists nowhere

**Mutation**: all seven run against `page.setContent("<html><body></body></html>")`
— the entire product deleted. **All passed, in 3.4 seconds.**

| Assertion | Sites |
|---|---|
| `getByTestId("compendium-coming-soon")).toHaveCount(0)` | `world-compendium.spec.ts:123,133`; `abilities-compendium.spec.ts:95` |
| `getByTestId("staging-player-list")).toHaveCount(0)` | `players-section.spec.ts:99`; `gm-staging-page.spec.ts:108` |
| `getByText("Lore — coming soon")).toHaveCount(0)` | `session-notes.spec.ts:89`; `world-staging-route.spec.ts:90` |
| `getByText("Placeholder domain")`, `getByText(/Awaiting a later phase/i)` | `onboarding-flow.spec.ts:189,190` |
| `locator("#world-interface-pack")).toHaveCount(0)` | `onboarding-flow.spec.ts:173` |
| `getByRole("link", { name: "Return to dashboard" })).toHaveCount(0)` | `gm-staging-page.spec.ts:114` |

Two of them sit **directly beside a comment congratulating the file for
avoiding exactly this**. The comment guards `new-npc-link`, which does still
exist; the adjacent line is the vacuous one.

### 7. A name promising coverage the body lacks — `src/core/src/policies/mod.rs`

`it_should_modify_amd_remove_existing_access_when_resource_found` never calls
`remove`. **Mutation**: `Policy::remove` gutted to a no-op — this test passed.
The same mutation showed the `remove(id, None)` branch has no test at all.

### 8. Fifteen tests that cannot fail — **fixed**

`test_loader_stub` and `test_module_loads` had empty bodies in all seven
packs, server and engine, plus one honestly-named no-op in `map_import`.
(Counted as seventeen on the first pass; there are seven packs, not eight —
`basic-game-system` has neither file.)

Each now asserts what a compile cannot. Server-side that is linkage:
`inventory` collects through the linker, so a deleted `submit!` block or a
validator quietly dropped to `None` is invisible to every other test in the
crate. Engine-side it is plugin self-sufficiency: a Bevy plugin reading a
resource it never inserts builds cleanly and panics on the first update,
which is the `WallPlugin`/`LightingPlugin` bug this repo has shipped twice.
`map_import`'s prose is a comment now — worth keeping, not worth a green
tick beside it.

### 9. Long-standing skips

`map-editor-tooling.spec.ts:534` — a skipped test with an empty body, blocked
on two pre-existing bugs. `auth-providers.spec.ts` skips on absent OAuth env
vars, so the login-button feature has no coverage on a default stack.

## Web unit tests

26 confirmed, all mutation-proven. Highest value:

- **`appearance/__tests__/appearance-context.test.ts`** (spec 032, mine):
  `missing = requestedId !== null && chosen === null ? requestedId : null`
  simplified to `missing = requestedId` is **green** — a pack that loaded
  perfectly can be reported missing and nothing fails.
- **`api/__tests__/graphqlClient.test.ts`**: CSRF is never asserted — both
  `withCsrf(...)` call sites replaced with plain literals stay green. And
  `extensions.code` is never read off the wire.
- **`engine/world/sync/__tests__/connectivity.test.ts`**: `isDisconnected` and
  `isServerIsolated` have no production call sites; the real queueing decision
  is `offlineQueue.shouldQueue()`. The test names claim otherwise.
- **`engine/world/sync/__tests__/discrepancies.test.ts`**: deleting the
  `noteDiscrepancy` call from `drainQueue` is green — the one seam that file's
  docstring exists to protect.
- **`layout/__tests__/sheetLayout.test.tsx`** (spec 032, mine): a pack's
  `columns` can be hardcoded to 3 and stay green; track clamping is untested.

## Disproved — these are real tests

Recorded because knowing a test works is worth as much as knowing one does not.

- `declared_values_tests.rs`'s two "publishes none" assertions: probed with
  `assert!(!values.is_empty())`; both non-empty, so the `all(...)` is not
  vacuous. The earlier remediation held.
- `no_bundled_system_stores_a_pool_it_never_declares`: its `continue` guard
  could have skipped every system; all eight reach the assert.
- `world_sync_plan.rs` and `pack_system_spec/interface_tests.rs`: both preceded
  by explicit guards that the subject is non-empty.
- `pathfinder2e` and `blades_in_the_dark` validators: both test their exact
  boundaries.
- The e2e payload-leak negatives (`status-disclosure`, `status-gm-control`,
  `interactive-secrets`, `interactive-regions`, `item-pickup-race`): each is
  paired with a positive assertion on the same payload from the other
  viewpoint, which is what makes an absence check meaningful.
- `world-appearance.spec.ts`: carries the CSRF-empty-body remediation already.
- `dnd5e/engine/src/plugin.rs`'s `ability_modifier`: fixed the same day.
