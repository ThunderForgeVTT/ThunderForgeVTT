# Implementation Plan: The Instance That Asks Only What It Needs

**Branch**: `main` | **Date**: 2026-10-02 | **Spec**: [spec.md](./spec.md)

**Depends on**: [spec 052, What an Access Mode Obliges](../052-access-mode-and-legal-duty/spec.md)

**Input**: Feature specification from
`/specs/064-the-instance-that-asks-only-what-it-needs/spec.md`

## Summary

The first-run wizard already exists and already gates the whole application:
until setup completes, `/` and `/login` redirect to `/setup` and nothing else
renders (`apps/web/src/routes/AppRoutes.tsx`). Spec 040 built it. So this spec
is not "build a wizard" — it is **make the wizard ask the right questions, in
the right order, for the kind of instance being stood up, and finish the job it
left half-done.**

Three things made that possible without new page machinery.

1. **The wizard's shape is data, not code.** A step is a group of registry
   declarations, in declaration order (`instanceSetup.ts: buildSetupSteps`).
   Forking the wizard is a change to *which* declarations
   `setup_asks_about` returns and in what order. The React side needed one new
   step kind, one new panel, and an explanation slot.
2. **The hinge had to move off the access policy.** ADR-103 put the legal duty
   on the access mode. That could not be explained to the operator incurring it
   (see [ADR-109](../../docs/adrs/20261002-109-an_instance_states_whether_it_publishes.md)),
   so the duty now hangs on an explicit `instance.publishes_beyond_world`.
   This is the only genuinely architectural decision in the change.
3. **The OIDC gap was one field.** Discord, GitHub and Google are configurable
   today with a client id and secret. Keycloak's seeded row carries empty
   endpoint strings because they are issuer-derived, and no field in
   `UpdateOAuthProviderInput` could set them. One input closes generic OIDC;
   `ProviderKind::derive_endpoints` already existed and had no caller from the
   operator's side.

## Technical Context

**Language/Version**: Rust 2021 (server, `thunderforge_axum_oauth`), TypeScript 5 (`apps/web`), SQL (one migration)

**Primary Dependencies**: Axum, Diesel + PostgreSQL, async-graphql, React 18, Playwright

**Storage**: PostgreSQL. One new column, `oauth_providers.issuer_url`. Everything else this spec collects lands in the existing `instance_settings` table through the settings registry — including the object-store connection, which was environment-only before.

**Testing**: `cargo test` (registry, `setup_requirements`, `readiness`, `provider_kind`, the admin surface tables); `pnpm --filter web test` for `instanceSetup.test.ts`; Playwright's **first-run lane** for the two walks, which is the actual proof.

**Target Platform**: Linux server, browser (Chromium under Playwright)

**Project Type**: Web application — Rust/Axum backend, React frontend.

**Performance Goals**: None specific. `RustFsConfig::resolve` reads the settings registry per use, exactly as `from_env` read the environment per use, so no client lifetime and no process-global cache is introduced — that shape has caused test flakes here before.

**Constraints**: The environment keeps winning over every declared setting (FR-040). No shipped default may be a credential (FR-041). ADR-041 stands: an `env`-sourced provider row accepts only `enabled`. Completion cannot regress — `Requirement::RequiredAtSetup` is still exactly `operator.name`, `operator.contact_email`, `support_email`.

**Scale/Scope**: 18 functional requirements in 6 groups. One migration, one new `Capability` variant, one new `SetupVisibility` variant, six new registry declarations, one new GraphQL mutation, two new React step components, one new e2e fixture module and two new e2e specs.

## Constitution Check

- **I. A change is described before it is made** — spec 052 stated the rule;
  spec.md and this plan state the shape. PASS.
- **II. The product is one product** — nothing here is instance-specific or
  pack-specific; the storage settings replace an environment-only path with the
  same registry every other setting uses. PASS.
- **III. Data has one owner** — the object store's connection now has one owner,
  the settings registry, with the environment overriding as it does everywhere.
  Before this change it had two statements of itself (`RUSTFS_*` and the client's
  own fallback defaults) and no owner. PASS.
- **IV. A significant decision leaves an ADR** — **ADR-109**, which amends
  ADR-103's decision 1 rather than superseding it. PASS.
- **V. Licensed content carries its licence** — no new content. PASS.
- **VI. Every feature is proven by its own slice** — **slice**: `instance`, run
  as `pnpm e2e:instance`, which owns the `instance-` prefix
  (`scripts/e2e/slices.json`). Both new specs are named `instance-first-run-*`
  at the `e2e/` root, so they join it by that prefix and no slices.json edit is
  needed. The two of them **also** route through the first-run lane
  (`isFirstRunSpec`), which is where they actually run: the `instance` slice's
  seeded stack has setup already complete, so a first-run spec cannot run there.
  PASS, with that caveat recorded rather than papered over.

  **And the slice is not sufficient on its own.** `node scripts/e2e-slice.mjs
  which` over the real changed paths names eight slices, not one — migrating
  `from_env` to `resolve` reached every uploader — and then routes to the **full
  suite** anyway, because five of the paths are cross-cutting: `graphql.rs`,
  `models.rs`, `schema.rs`, the new migration directory, and
  `apps/web/e2e/fixtures/first-run.ts`. That is also the only run that includes
  the first-run lane, so the full suite is the proof either way.

## The work, in six parts

### 1. The fork itself

`src/server/src/settings/registry.rs` — `instance.access_policy` moves to the
front of the declaration list, so Access is the first settings step, and its
declared default changes from `"closed"` to `"invite_only"` to agree with the
migration that seeds the row (ADR-109 decision 5). A sibling declaration
`instance.publishes_beyond_world` (`Kind::Bool`, `Backing::Row`, default false,
group `Access`) is the statement everything hangs on. `SetupVisibility` gains
`AskedWhenPublishing`, carried by the three `notice.*` and the `legal.*`
declarations, and `Offered`, carried by everything an instance may want and does
not need.

`src/server/src/auth/setup_requirements.rs` — `setup_asks_about` consults the
resolved settings. `AskedWhenPublishing` declarations appear only while the
statement is true. `missing_required_settings` is untouched.

`src/server/src/readiness.rs` — `CapabilityReport` gains `applicable: bool`, and
`assess` applies the same condition. A declined capability is a third state, not
a gap (052 FR-011).

### 2. The explanation, above the fields

`apps/web/src/services/instanceSetup.ts` — a step carries an optional
`explainer`, keyed by registry group, and `SettingsStep.tsx` renders it above
the fields (`setup-group-explainer`, `data-group`). FR-021 is about position:
a hint under a field is read after the question has already landed.

`steps/AccessConsequences.tsx` (new) — each policy with its consequence,
invite-only marked as the recommendation (FR-022), and a preview of what
checking "publishes beyond a world" will add, shown **before** it can be saved
(FR-011, `setup-publishing-preview`).

`SetupPage.tsx` — the progress rail says what this instance is *not* being asked
(`setup-not-asked`). `setup-skipped-steps` could not carry it: its copy strictly
means "the environment fixes everything on this step", and the legal steps are
absent, not skipped.

### 3. Providers, and an issuer URL

Migration `2026-10-02-090000-0000_oauth_provider_issuer_url` adds
`oauth_providers.issuer_url`, persisted rather than reverse-derived from
`authorization_url`.

`src/server/src/admin.rs` — when the row's kind has a `required_issuer_field`,
the issuer passes `check_outbound_url` (FR-033) and then
`ProviderKind::derive_endpoints`, which writes the three derived URLs. A kind
that publishes its own endpoints **refuses** an issuer (FR-034).

`crates/thunderforge-axum-oauth/src/provider_kind.rs` — new
`ProviderKind::from_provider_key`. The stored `provider_key` is lowercase and
may carry an instance suffix (`keycloak__work`); `from_env_segment` matches
uppercase environment-variable segments. Resolving one with the other returns
`None` for every provider there is, which is how the issuer feature came to be
dead on arrival at two call sites — see the Verification note.

`apps/web/src/pages/setup/steps/ProvidersStep.tsx` (new) — lists the seeded
providers and reuses `OAuthProviderForm` directly. Never required (FR-030), and
it says that an invite link admits its holder either by sign-up or by signing in
with a provider enabled there (FR-031). Not registry-derived, so
`buildSetupSteps` grows a `kind: "providers"`.

### 4. Where uploads are kept

Five registry declarations in group `Storage`, all `Backing::Row`,
`Requirement::RequiredFor(Capability::StoreAssets)`, `SetupVisibility::Offered`,
with their `RUSTFS_*` environment forms preserved. `Capability::StoreAssets` is
a new variant, which readiness picks up through `assess` automatically. The
secret key is `secret: true`, which already means never pre-filled and never
echoed.

`src/server/src/storage/rustfs.rs` — `RustFsConfig::resolve(state)`, registry
first and environment winning, with the development fallbacks kept **in the
client** rather than declared, so no shipped default is a credential (FR-041).
The non-test `from_env()` call sites migrate to it; `from_env` stays for
`src/app/src/main.rs` (boot bucket creation runs before setup has answered
anything) and for tests.

New admin mutation `testStorageConnection` wrapping the existing `health_check`,
surfaced as a button on the step (FR-042).

### 5. The final step

`buildSetupSteps` collects every `Offered` declaration onto **one** step,
`settings-anything-else`, rather than one step per group (FR-023), with an
explainer saying all of it can be changed later in the admin area. Its
`complete` is unconditionally true: nothing on it can block completion by
construction, so an operator who skips it has answered it, and a resumed pass
lands on the second factor rather than being sent back.

### 6. The two walks

`apps/web/e2e/fixtures/first-run.ts` (new) holds the mechanics all three
first-run specs share — scrape the setup link from the backend log, open the
wizard, create the first administrator, enrol a second factor, walk to a step,
open a provider's editor. Mechanics, not claims: a spec's own file holds what it
asserts.

`instance-first-run-private.spec.ts` (FR-060) and
`instance-first-run-public.spec.ts` (FR-061). The existing
`instance-setup.spec.ts` becomes the third case and gains one step: it now says
*yes* to publishing, so its notices and legal assertions stay reachable.

The lane grows from one spec to three, so it starts **a stack per spec**
(FR-062): setup completes once, and a second walk on the same stack finds
`/setup` redirecting and fails with something that looks nothing like its
subject. `playwright.config.ts` and `scripts/e2e/specs.mjs` share one pattern,
`/instance-(setup|first-run-[a-z]+)\.spec\.ts$/`, so the partition between the
`chromium` and `first-run` projects cannot drift.

## Verification

Local only; there is no CI.

1. `cargo fmt --check`, then `make lint` — **both** targets: the engine lints for
   wasm32, never the host.
2. `cargo test --workspace`. Needs the `thunderforge-canvas-assets` bucket to
   exist, or ~33 storage tests fail on `PutObject`.
3. `pnpm --filter web test` for `instanceSetup.test.ts`.
4. `node scripts/e2e-slice.mjs which <changed paths>`, then the slices it names
   — before the full suite, because a slice failure is cheaper to read.
5. **`node ./scripts/e2e-parallel.mjs`**, the full suite, which is what the slice
   tool routes this change to and the only run that includes the first-run lane.
   That lane is the actual proof.
6. `pnpm journeys` last, for cross-cutting regressions.

**One finding worth recording here, because it is why step 2 is not optional.**
The issuer feature was dead on arrival in the first implementation pass. Both
`src/server/src/admin.rs` and `src/server/src/graphql/admin_types.rs` resolved
the provider kind with `from_env_segment(provider_key.split("__").next())`, and
`provider_key` is lowercase while `from_env_segment` matches uppercase. So
`kind` was always `None`: supplying Keycloak an issuer would have been
**refused** with "Keycloak publishes its own endpoints, so it takes no issuer
URL", and `requires_issuer_url` was always false, so the field would never have
rendered. Nothing in the test suite caught it, because nothing walked
`ProviderKind::ALL` through the operator's route. `from_provider_key` now exists
with exactly that test.

## Not in scope

As spec.md: no OIDC `.well-known` discovery, no create/delete mutation for
providers, and spec 052's US3 and US5 beyond what the two walks assert in
passing.
