---
description: "Task list for The Instance That Asks Only What It Needs"
---

# Tasks: The Instance That Asks Only What It Needs

**Input**: Design documents from `/specs/064-the-instance-that-asks-only-what-it-needs/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [spec 052](../052-access-mode-and-legal-duty/spec.md), [ADR-109](../../docs/adrs/20261002-109-an_instance_states_whether_it_publishes.md)

**Tests**: included, and they are the deliverable. The owner's request was for
two end-to-end walks; the product changes exist so that there is something true
for them to walk.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished work)

---

## Phase 1: The hinge

**Purpose**: an instance can state whether it publishes, and that statement
decides what it is asked for.

- [X] T001 `src/server/src/settings/registry.rs`: add `SetupVisibility::AskedWhenPublishing` and `SetupVisibility::Offered` beside `Asked`, documenting on each variant what the wizard does with it
- [X] T002 `registry.rs`: declare `instance.publishes_beyond_world` (`Kind::Bool`, `Backing::Row`, default `false`, group `Access`, env `THUNDERFORGE_INSTANCE_PUBLISHES_BEYOND_WORLD`), and move `instance.access_policy` to the front of the declaration list so Access is the first settings step
- [X] T003 `registry.rs`: change `instance.access_policy`'s declared default from `"closed"` to `"invite_only"`, agreeing with the migration that seeds the row on a fresh install. ADR-109 decision 5 reverses ADR-103 decision 3 here; nobody is locked out, because admission bypasses the policy while no administrator exists
- [X] T004 `registry.rs`: mark the three `notice.*` and the `legal.*` declarations `AskedWhenPublishing`
- [X] T005 `src/server/src/auth/setup_requirements.rs`: `setup_asks_about` takes the resolved settings and answers `false` for an `AskedWhenPublishing` declaration while the statement is false. Leave `missing_required_settings` alone — `RequiredAtSetup` stays exactly `operator.name`, `operator.contact_email`, `support_email`, so completion cannot regress
- [X] T006 `src/server/src/readiness.rs`: `CapabilityReport` gains `applicable: bool` and `assess` applies the same condition, so a declined capability is a third state rather than a gap (052 FR-011). Additive, so existing readers are unaffected
- [X] T007 `src/server/src/settings/graphql.rs`: carry `applicable` on `GraphQLCapability`

**Checkpoint**: a private instance is no longer asked for a notice contact, and
readiness says so rather than going quiet.

---

## Phase 2: Saying it out loud

**Purpose**: the operator can tell "it decided I do not owe this" from "it
forgot", and knows what checking the box will cost before checking it.

- [X] T010 `apps/web/src/services/instanceSetup.ts`: a step carries an optional `explainer`, keyed by registry group; `groupExplainer` holds the text
- [X] T011 `apps/web/src/pages/setup/steps/SettingsStep.tsx`: render the explainer **above** the fields (`setup-group-explainer`, `data-group`). FR-021 is a claim about position, not about wording
- [X] T012 `apps/web/src/pages/setup/steps/AccessConsequences.tsx` (new): each policy with its consequence, invite-only marked **Recommended** (FR-022)
- [X] T013 `AccessConsequences.tsx`: `setup-publishing-preview` — what checking "publishes beyond a world" adds, rendered while the step is still unsaved (FR-011)
- [X] T014 `apps/web/src/pages/setup/SetupPage.tsx`: `setup-not-asked` on the progress rail, naming what this instance is not being asked for

  **Note on why this is not `setup-skipped-steps`.** That existing rail note means, strictly, "the environment fixes everything on this step". The legal steps are *absent*, not skipped, and reusing the note would have said something false about the environment. A separate note also makes the private spec's assertion positive rather than only an absence, which is what FR-010 is actually asking for.

**Checkpoint**: nothing is silently missing.

---

## Phase 3: Providers, and the issuer URL

**Purpose**: generic OIDC becomes reachable through the product, and the wizard
is where it is reached.

- [X] T020 Migration `src/server/migrations/2026-10-02-090000-0000_oauth_provider_issuer_url/{up,down}.sql`: `oauth_providers.issuer_url`, nullable. Persisted rather than reverse-derived from `authorization_url`
- [X] T021 `src/server/src/graphql/admin_types.rs`: `issuerUrl` on the provider config input and `requiresIssuerUrl` on the type; `src/server/src/models.rs`, `schema.rs`, `adapters.rs` follow the column
- [X] T022 `crates/thunderforge-axum-oauth/src/provider_kind.rs`: `ProviderKind::from_provider_key`, which lowercases and strips the `__instance` suffix before matching — see T026 for why this task exists at all
- [X] T023 `src/server/src/admin.rs`: an issuer for an issuer-derived kind passes `check_outbound_url` (FR-033) and then `derive_endpoints`, writing the three derived URLs; a fixed-endpoint kind **refuses** it with a message saying so (FR-034). Env-sourced rows keep accepting only `enabled` — ADR-041, unchanged
- [X] T024 `apps/web/src/pages/admin/components/OAuthProviderForm.tsx`: an Issuer URL field, rendered only for issuer-derived kinds, with the hint that the three endpoints come from it
- [X] T025 `apps/web/src/pages/setup/steps/ProvidersStep.tsx` (new): a wizard step over the seeded providers, reusing `OAuthProviderForm`, always skippable (FR-030), saying that an invite link admits its holder by sign-up **or** by signing in with a provider enabled here (FR-031). `buildSetupSteps` grows `kind: "providers"`; `SetupPage.tsx`'s switch grows the branch
- [X] T026 Tests: `every_provider_resolves_from_its_stored_provider_key` walks `ProviderKind::ALL` through both `key` and `key__work`, and `a_provider_key_this_build_never_heard_of_resolves_to_nothing`

  **This task is the reason the feature works.** The first implementation pass resolved the kind with `from_env_segment(provider_key.split("__").next())` at two call sites. `provider_key` is lowercase (`oauth_env.rs`); `from_env_segment` matches uppercase environment-variable segments, and its own existing test asserts `from_env_segment("keycloak") == None`. So `kind` was `None` for every row ever: supplying Keycloak an issuer would have been **refused** with "Keycloak publishes its own endpoints, so it takes no issuer URL", and `requires_issuer_url` was always false, so the field would never have rendered. The feature was dead on arrival and the suite was silent, because nothing walked `ProviderKind::ALL` through the operator's route. It does now. `cargo test -p thunderforge_axum_oauth --lib provider_kind` → 9 passed.

**Checkpoint**: an operator can configure Keycloak from an issuer alone, in the
wizard, and a fifth provider cannot be added without being reachable the same
way.

---

## Phase 4: Where uploads are kept

**Purpose**: the wizard stops handing back an instance that cannot store a map.

- [X] T030 `src/server/src/settings/registry.rs`: `Capability::StoreAssets` — variant, `key()`, `label()`, `all()`. Readiness picks it up through `assess` with no further edit
- [X] T031 `registry.rs`: five declarations in group `Storage` — `storage.endpoint` (`Url`), `.region`, `.bucket`, `.access_key` (`Text`), `.secret_key` (`Text`, `secret: true`) — all `Backing::Row`, `RequiredFor(Capability::StoreAssets)`, `Offered`, keeping their `RUSTFS_*` environment forms so the environment still wins (FR-040)
- [X] T032 `src/server/src/storage/rustfs.rs`: `RustFsConfig::resolve(state)` — registry first, environment winning, with the bundled stack's development fallbacks left **in the client** rather than declared, so no shipped default is a credential (FR-041)
- [X] T033 Migrate the non-test `from_env()` call sites to `resolve`: `assets_serve/{canvas,feedback,lore,actor,scene}.rs`, `map_import/{image,mod}.rs`, `feedback/{deliver,mod,schedule}.rs`, `graphql/{mutations_assets,mutations_lore_images,mutations_actor_images}.rs`, `storage/backfill.rs`. Per-use, as before — **no process-global cache**, which is a shape that has caused test flakes here before
- [X] T034 `from_env` stays for `src/app/src/main.rs` (boot bucket creation runs before setup has answered anything) and for tests
- [X] T035 `src/server/src/graphql/mutations_admin.rs`: `testStorageConnection`, wrapping the existing `health_check` (FR-042); `admin_surface_tests.rs` and `play_pause_surface_tables.rs` list it, as they list every admin mutation
- [X] T036 `apps/web/src/api/admin.ts`, `types/admin.ts`, `SettingsStep.tsx`: the probe button and its result (`setup-storage-probe-run`, `setup-storage-probe-result`), keyed off the `storage.` declaration prefix so the test follows the settings if the registry moves them

**Checkpoint**: storage is answerable in the wizard and provable before the
operator leaves the step.

---

## Phase 5: One final step

**Purpose**: the wizard does not sprout four more mandatory-looking screens.

- [X] T040 `instanceSetup.ts`: `buildSetupSteps` collects every `Offered` declaration onto one step, `settings-anything-else`, in registry order, after the settings groups and the providers step (FR-023)
- [X] T041 That step's `complete` is unconditionally `true` — nothing on it can block completion by construction, so an operator who skips it has answered it, and a resumed pass lands on the second factor rather than being sent back through a page of optional extras it already walked past
- [X] T042 Its explainer says every answer on it can be changed later in the admin area
- [X] T043 `apps/web/src/services/__tests__/instanceSetup.test.ts`: the fork, the providers step and the merged final step

**Checkpoint**: declaration order is step order, and the order reads as the
owner asked — the legal duties, then the providers, then the extras.

---

## Phase 6: The two walks

**Purpose**: the thing that was actually asked for.

- [X] T050 `apps/web/e2e/fixtures/first-run.ts` (new): the mechanics all three first-run specs share — `waitForSetupCode` (polled, because `/api/readyz` answers before the bootstrap code is logged), `openWizard`, `createFirstAdministrator`, `enrolSecondFactor`, `advanceFrom`, `walkTo`, `openProviderEditor`, `graphqlPublic`. Mechanics, not claims: a spec's own file holds what it asserts
- [X] T051 `apps/web/e2e/instance-first-run-private.spec.ts` (FR-060): invite-only, publishing nothing; asserts the copyright-notice and legal-prose steps are never shown **and** that the rail says so; configures Keycloak from an issuer pointed at this shard's OAuth stub; proves storage reachable; completes; then issues an invitation and, in a fresh browser context, redeems it **through the provider**
- [X] T052 `apps/web/e2e/instance-first-run-public.spec.ts` (FR-061): open and publishing; asserts the preview names what that adds **before** the step is saved, that each legal step carries its reason above the fields, that a reserved-domain notice address is refused, and that the review names what was left unanswered; then reads `/legal/dmca` and `/legal/terms` as a stranger

  **Deviation from the plan, recorded.** The plan asserted that completion is refused with `409 incomplete` while a notice contact is blank. It is not, and could not be: `missing_required_settings` filters on `is_required_at_setup()`, and the `notice.*` and `legal.*` declarations are `RequiredFor(Capability::…)`. A blank notice contact leaves the *capability* unavailable, not setup incomplete — which is 052 FR-030 working as written. The proof is therefore readiness reporting `publish_terms` as applicable-but-unavailable with a gap naming the missing key, plus `/legal/terms` showing its placeholder rather than invented prose.

- [X] T053 `apps/web/e2e/instance-setup.spec.ts`: adapted to the fixture, and given one new step — it now says **yes** to publishing, which is what keeps its existing notices and legal assertions reachable. It becomes the third case: the full walk, with the fork taken the other way
- [X] T054 `apps/web/playwright.config.ts` and `scripts/e2e/specs.mjs`: one shared pattern, `/instance-(setup|first-run-[a-z]+)\.spec\.ts$/`, so `first-run`'s `testMatch`, `chromium`'s `testIgnore` and `isFirstRunSpec`'s routing cannot drift apart
- [X] T055 `scripts/e2e-parallel.mjs`: the first-run lane starts **a stack per spec** (FR-062), at indices `total`, `total + 2`, `total + 3` — `total + 1` belongs to the GitHub-applications lane, so the lane is skipped over rather than renumbered. Setup completes once; a second walk on the same stack finds `/setup` redirecting and fails with something that looks nothing like its subject
- [X] T056 `scripts/oauth-stub.mjs`: Keycloak's three issuer-derived paths, so the walk exercises the product's own `derive_endpoints` expansion rather than a hard-coded endpoint set

**Checkpoint**: the two walks exist. They are green only when the lane says so —
see Phase 7.

---

## Phase 7: Recording and verification

- [X] T060 `specs/064-the-instance-that-asks-only-what-it-needs/spec.md`, naming spec 052 a dependency and citing its FR-010, FR-011, FR-030, FR-032 and FR-060 rather than restating them
- [X] T061 [ADR-109](../../docs/adrs/20261002-109-an_instance_states_whether_it_publishes.md), and the ADR index row. It **amends** ADR-103 rather than superseding it: decisions 2, 4 and 5 stand, decision 1 is narrowed, decision 3 is reversed
- [X] T062 This plan and this ledger
- [X] T063 `cargo fmt --all`, and `cargo test -p thunderforge-server --lib` → 1771 passed, 0 failed
- [X] T064 `cargo test -p thunderforge_axum_oauth --lib provider_kind` → 9 passed
- [X] T065 `make lint` — **both** targets. The engine lints for wasm32, never the host
- [X] T066 `cargo test --workspace` → green, every crate, with `RUST_MIN_STACK=16777216`. Needs the `thunderforge-canvas-assets` bucket, or ~33 storage tests fail on `PutObject`
- [X] T067 `pnpm --filter web test` → 684 tests in 75 files, all passing
- [X] T072 `pnpm verify --fix` to regenerate the committed GraphQL schema snapshot and operations, which `testStorageConnection`, `issuerUrl` and `requiresIssuerUrl` all move
- [X] T073 Split the two files this change pushed over the repo's 1000-line gate, each along a seam that was already there rather than at a line count:

  - `src/server/src/settings/registry.rs` (1145) → `registry.rs` (375: the vocabulary a declaration is written in) plus `registry/declarations.rs` (797: the list, and nothing else). The module doc on the new file records why order matters twice over — it is the admin surface's render order *and* the wizard's step order, which is what FR-020 rests on.
  - `src/server/src/admin.rs` (1009) → `admin.rs` (672) plus `admin/providers.rs` (360), everything about the `oauth_providers` table and the two rules that govern it: ADR-041, and FR-032/FR-033.

- [X] T074 `pnpm verify` → all 16 checks pass
- [X] T068 Ask the slice tool what this change needs: `node scripts/e2e-slice.mjs which <changed paths>`

  **The answer is the full suite, and it was not what the plan assumed.** Eight slices are named — accounts, actors, canvas, feedback, instance, lore, play-pause, scenes — because `RustFsConfig::resolve` reached every uploader, and then five paths are cross-cutting and route past any slice regardless: `src/server/src/graphql.rs`, `models.rs`, `schema.rs`, the new migration directory, and `apps/web/e2e/fixtures/first-run.ts` ("shared e2e fixtures every spec builds on"). plan.md's Constitution Check §VI is corrected there to say so.

- [ ] T069 **The full suite**, `node ./scripts/e2e-parallel.mjs` — which is both what T068 routes to and the only thing that runs the first-run lane. This is the proof, and until it is green this spec's status line is honest only because it says "pending the first-run lane's green run"
- [ ] T070 `pnpm journeys`, for cross-cutting regressions
- [X] T071 `.env.example`: the `RUSTFS_*` block now says these are also declared `storage.*` settings, answerable in the wizard, with the environment winning as it does for every declared setting. No other document described them as environment-only — `grep` over `docs/` and `README.md` finds no mention, which is itself worth recording: the object store had no documentation outside this file

---

## Known conditions on `main`, not caused by this change

Recorded so that a red run is read correctly rather than blamed here.

- `thunderforge_engine`'s `systems::camera_focus::tests::*` — 6 failures on clean `main`, unrelated to anything in this spec.
- `play_pause_surface_tests::every_gated_pack_field_refuses_a_paused_world` and `interface_packs::integration_tests::the_mutation_is_reachable_on_the_root_and_a_dm_can_execute_it` overflow the stack under the default 2 MiB test-thread stack; both pass with `RUST_MIN_STACK=16777216`.
- `caught-in-the-middle` can fail in `seatATable` with the operator's session showing in the Game Master's browser during a full journey run. It does not reproduce per-file and is not root-caused.
