---

description: "Task list for Configurable Multi-Provider Authentication"
---

# Tasks: Configurable Multi-Provider Authentication

**Input**: Design documents from `/specs/007-configurable-auth-providers/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/oauth-provider-admin.md, quickstart.md

**Tests**: Included — this feature changes an authentication trust boundary (env-var vs. admin-panel config precedence), which Constitution Principle V's "Verify Before Claiming Done" and this repo's established per-spec precedent (specs 001-006) both treat as requiring live verification, not just code review.

**Organization**: Tasks are grouped by user story (US1-US4 from spec.md).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US4)

## Path Conventions

`src/server/` (Rust/Axum/Diesel backend), `apps/web/` (React frontend), `docs/adrs/` (already landed — ADR-041). No `src/engine` changes.

---

## Phase 1: Setup

**Purpose**: Remove the dead scaffolding this feature would otherwise be confused with, before adding the real mechanism.

- [X] T001 [P] Delete the unwired `THUNDERFORGE_AUTHENTICATION` env var, `SupportedAuthentication`, and `OAuth2Config` types from `src/server/src/config/mod.rs` (research.md §1) — confirmed dead via repo-wide grep (parsed into `Config.authentication`, never read anywhere after). Update `Config`/`Config::from_env` accordingly; `src/server/src/test_support.rs`'s `Config::from_env()` call keeps working unchanged since the struct still exists, just without the removed field.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared DB schema and env-var-parsing/materialization mechanism every user story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 Migration: add `config_source VARCHAR NOT NULL DEFAULT 'admin'` to `oauth_providers`, per data-model.md — new directory under `src/server/migrations/`, paired `up.sql`/`down.sql` (down: `ALTER TABLE oauth_providers DROP COLUMN config_source;`), following this repo's existing per-change migration-directory convention.
- [X] T003 [P] Migration: seed an unconfigured Keycloak/generic-OIDC template row into `oauth_providers` (data-model.md's "New built-in preset seed data"), following `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`'s exact `INSERT ... ON CONFLICT (provider_key) DO UPDATE` shape — new migration directory, independent of T002 (different file).
- [X] T004 Update `OAuthProvider`, `NewOAuthProvider` (and any `OAuthProviderUpdate`/changeset struct used by `update_oauth_provider`) in `src/server/src/models.rs` to include `config_source: String` (or a small enum with a `#[diesel(sql_type = ...)]` mapping, implementer's call) — depends on T002's column existing.
- [X] T005 New module `src/server/src/config/oauth_env.rs`: env-var provider-instance parser implementing the `OAUTH_<PROVIDER>_[<INSTANCE>_]<FIELD>` naming scheme from research.md §4 — longest-known-preset-prefix match, then known-field-suffix match (`CLIENT_ID`, `CLIENT_SECRET`, `LABEL`, `ISSUER_URL`, and the generic-provider fields `AUTHORIZATION_URL`/`TOKEN_URL`/`USERINFO_URL`/`SCOPES`), remainder-between-them is the instance key. Output: a `Vec` of parsed candidate provider-instance configs (provider/instance/field map), not yet resolved against presets.
- [X] T006 [P] Built-in preset resolution table in `src/server/src/config/oauth_env.rs` (or a sibling `oauth_presets.rs`): maps a recognized preset name (`discord`, `github`, `google`, `keycloak`, ...) to its known `authorization_url`/`token_url`/`userinfo_url`/`scopes` — for Discord/Google/GitHub, reuse the exact URLs already seeded in `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`'s `up.sql` (research.md §2, keep them in sync); for Keycloak, derive the three endpoint URLs from `OAUTH_KEYCLOAK[_<INSTANCE>]_ISSUER_URL` + the standard `/protocol/openid-connect/{auth,token,userinfo}` suffixes (data-model.md).
- [X] T007 Startup materialization function (`src/server/src/config/oauth_env.rs` or `src/server/src/auth/mod.rs`, implementer's call): for each parsed+resolved instance from T005/T006, `INSERT ... ON CONFLICT (provider_key) DO UPDATE` into `oauth_providers` with `config_source = 'env'`, updating `display_name`/`authorization_url`/`token_url`/`userinfo_url`/`scopes`/`oauth_client_id`/`oauth_client_secret`/`configured` every run but setting `enabled = true` **only on first insert** (research.md §3). For every existing `config_source = 'env'` row whose env vars are no longer present in the current scan, flip it back to `config_source = 'admin'` without touching its other columns (research.md §6). Depends on T004, T005, T006.
- [X] T008 Wire T007's materialization function into server startup in `src/server/src/main.rs`, running once after migrations complete and before the HTTP listener starts accepting connections.
- [X] T009 [P] Unit tests (`#[cfg(test)]` in `src/server/src/config/oauth_env.rs`, matching this repo's existing colocated-test convention) for the T005 parser: default instance (`OAUTH_KEYCLOAK_CLIENT_ID` → provider `keycloak`, no instance), named instance (`OAUTH_KEYCLOAK_WORK_CLIENT_ID` → instance `work`), generic provider with no preset match, and an unrecognized/partial env var set that must be safely ignored with a diagnostic (FR-010) rather than crashing the parse.

**Checkpoint**: Foundational mechanism complete — env vars can be parsed, resolved, and materialized into `oauth_providers` rows with correct precedence bookkeeping. No user-visible behavior yet (nothing calls T007/T008 into anything user-facing until US1's verification).

---

## Phase 3: User Story 1 - Operator enables a new login provider with zero UI setup (Priority: P1)

**Goal**: Setting `OAUTH_<PROVIDER>_*` env vars and restarting is sufficient, alone, to make a real login button appear and work — including for named multi-instance and fully-generic providers.

**Independent Test**: quickstart.md Scenarios 1-3 — set/unset Discord env vars across a restart and confirm the button appears/disappears; configure two named Keycloak instances and confirm two independent buttons; configure an unlisted OAuth2 provider via the fully-generic env-var set and confirm it works identically to a preset.

### Tests for User Story 1

- [X] T010 [P] [US1] Playwright e2e in a new `apps/web/e2e/auth-providers.spec.ts`: set `OAUTH_DISCORD_CLIENT_ID`/`OAUTH_DISCORD_CLIENT_SECRET` against a live dev stack, restart, confirm a "Log in with Discord" button renders and completes a real login; unset and restart, confirm the button disappears (quickstart Scenario 1; FR-001, FR-003; SC-001).
- [X] T011 [P] [US1] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: configure a default and a named-instance (`_WORK_`) Keycloak env-var set simultaneously, confirm two distinct, independently-working login buttons render (quickstart Scenario 2; FR-012).
- [X] T012 [P] [US1] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: configure an unlisted OAuth2 provider via the fully-generic env-var set (`_AUTHORIZATION_URL`/`_TOKEN_URL`/`_CLIENT_ID`/`_CLIENT_SECRET`), confirm its login button renders and completes a real login (quickstart Scenario 3; FR-002 acceptance scenario 6).

### Implementation for User Story 1

- [X] T013 [US1] Verify (and fix if needed) that `apps/web/src/api/auth.ts`'s `startOAuthLogin` redirect-URI construction and `apps/web/src/pages/auth/LoginView.tsx`'s button rendering handle a compound `provider_key` (e.g. `keycloak__work`) transparently, since both currently treat `provider_key` as an opaque string already — this task is expected to be verification-only per research.md §4's design intent, but any assumption found that breaks on an `__`-containing key must be fixed here.
- [X] T014 [US1] Diagnostic logging (FR-010) in T005/T007's parse/materialize path: when a detected `OAUTH_<PROVIDER>_*` env-var group is missing a required field (e.g. `CLIENT_ID` present, `CLIENT_SECRET` absent; or a Keycloak-shaped instance missing `ISSUER_URL`), log a clear line naming the provider/instance and the missing field, and skip materializing that incomplete instance rather than partially upserting it or crashing startup.

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001 confirmed by T010-T012.

---

## Phase 4: User Story 2 - Username/password sign-in stays a first-class option (Priority: P1)

**Goal**: Confirm this feature does not regress the existing, already-implemented username/password path — it must remain visible and functional regardless of how many OAuth providers are configured.

**Independent Test**: quickstart.md Scenario 4 — with several OAuth providers configured (from US1), confirm username/password sign-up/login is still presented and unobstructed, and resolves to the same unified identity as any other auth path.

### Tests for User Story 2

- [X] T015 [P] [US2] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: with multiple OAuth providers configured (reusing US1's env-var setup), confirm username/password sign-up and login remain visible, functional, and not visually subordinated on the sign-in screen; confirm a user who signs up with username/password and later links a configured OAuth provider resolves to one unified account, not two (quickstart Scenario 4; FR-004; SC-003).

### Implementation for User Story 2

- [X] T016 [US2] Regression-verification pass (no implementation expected): confirm none of T001-T014's changes touch `LoginView.tsx`'s username/password form rendering path, its visibility gating, or the existing unified-identity account-linking logic in `src/server/src/auth/mod.rs`. If T010-T012/T015 reveal any regression, fix it here — this task exists to make that check explicit and trackable, not to assume it away.

**Checkpoint**: User Story 2 confirmed with no regression — SC-003 confirmed by T015.

---

## Phase 5: User Story 3 - Owner configures and overrides providers from the admin panel (Priority: P2)

**Goal**: The existing admin panel keeps working for providers with no env-var configuration, clearly shows env-sourced providers as read-only (except `enabled`), and toggling `enabled` on an env-sourced provider survives a server restart.

**Independent Test**: quickstart.md Scenario 5 — admin adds credentials for an env-var-free provider and sees it go live without a restart; admin views an env-sourced row as read-only; admin disables an env-sourced provider and confirms it stays disabled across a restart.

### Tests for User Story 3

- [X] T017 [P] [US3] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: as admin, add credentials for a provider with no env vars set, save, confirm its login button appears on the sign-in screen with no server restart (quickstart Scenario 5 steps 1-2; FR-005; SC-002).
- [X] T018 [P] [US3] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: as admin, view an env-sourced provider row and confirm its credential/URL fields render read-only/masked while `enabled` stays interactive; toggle `enabled` off, save, confirm the button disappears immediately; restart the server (env vars unchanged) and confirm the button stays gone (quickstart Scenario 5 steps 3-4; FR-006, FR-008; SC-002).

### Implementation for User Story 3

- [X] T019 [P] [US3] Expose `config_source` as `configSource: OAuthConfigSource!` (`ADMIN`/`ENV`) on `GraphQLOAuthProvider` in `src/server/src/graphql/admin_types.rs`, per contracts/oauth-provider-admin.md — depends on T004.
- [X] T020 [US3] Write-guard in `update_oauth_provider` (`src/server/src/graphql.rs`'s `AdminMutation`, per contracts/oauth-provider-admin.md and research.md §5): when the target row's `config_source == 'env'`, apply only `enabled` from `GraphQLOAuthProviderConfigInput`; ignore (do not write) `displayName`/`oauthClientId`/`oauthClientSecret`/`userinfoUrl`/`scopes` on such rows, and return the row's real persisted values in the response regardless of what the input requested. Depends on T019.
- [X] T021 [US3] Frontend: `apps/web/src/pages/admin/components/OAuthProviderForm.tsx` renders credential/URL/label fields as read-only/masked when `provider.configSource === "ENV"`, keeping the `enabled` toggle interactive and adding a visible "configured via environment variable" indicator (satisfies contracts/oauth-provider-admin.md's admin-UX note and quickstart Scenario 5 step 3). Depends on T019.

- [X] T021a [US3] Migration upgrade-safety test: against a DB seeded with a working, admin-configured provider row from *before* T002's migration, apply T002 (and T003) and confirm the row is untouched except for `config_source` defaulting to `'admin'`, and that it still authenticates successfully with zero re-entry (quickstart Scenario 8; FR-014; SC-007) — a `#[test]` in the migrations' own test harness if one exists for this repo's Diesel migrations, otherwise a manual-run step documented in this task and exercised once via `diesel migration run` against a pre-feature DB snapshot.

**Checkpoint**: User Story 3 fully functional and independently verified — SC-002 confirmed by T017-T018, SC-007/FR-014 confirmed by T021a.

---

## Phase 6: User Story 4 - Custom branding for a provider's login button (Priority: P3)

**Goal**: `OAUTH_<PROVIDER>_[<INSTANCE>_]LABEL` and the existing admin-panel display-name field both override a provider's button text.

**Independent Test**: quickstart.md Scenario 6 — set a custom `_LABEL` env var alongside valid credentials and confirm the button reflects it; separately confirm the same via the admin panel's label field with no env var involved.

### Tests for User Story 4

- [X] T022 [P] [US4] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: set `OAUTH_KEYCLOAK_LABEL="Thicc Dungeon"` alongside valid credentials, confirm the button reads "Log in with Thicc Dungeon" instead of the default (quickstart Scenario 6 steps 1-2; FR-007).
- [X] T023 [P] [US4] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: as admin, set a custom display name on an admin-sourced (non-env) provider row with no `_LABEL` env var, confirm the sign-in screen reflects it (quickstart Scenario 6 step 3; FR-007).

### Implementation for User Story 4

- [X] T024 [US4] Confirm T005's parser already recognizes the `LABEL` field suffix per-instance (it's listed in T005's known-suffix set) and that T007's materialization writes it into `display_name`; this task is expected to be verification-only given T005/T007's design, but implement any gap T022 reveals.

**Checkpoint**: User Story 4 fully functional and independently verified — FR-007 confirmed by T022-T023.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T025 [P] Run `cargo check` and `cargo test` on `src/server` (Constitution Principle V), resolving any new warnings.
- [X] T026 [P] Run `tsc --noEmit` and `vite build` on `apps/web` (Constitution Principle V).
- [X] T027 [P] Playwright e2e in `apps/web/e2e/auth-providers.spec.ts`: with providers configured via both env vars and the admin panel, inspect every network response body (sign-in screen load, admin panel load) and confirm no `oauthClientSecret` value ever appears — only `hasClientSecret` (quickstart Scenario 7; SC-004).
- [X] T028 [P] Update `MVP.md`'s Phase 1 ("User Login") note — append that OAuth providers are now configurable via environment variables (with multi-instance support) in addition to the admin panel, referencing this feature's closure.
- [X] T029 Run the complete quickstart.md walkthrough (all 7 scenarios) as one connected pass against a live dev stack, confirming SC-001 through SC-007 all hold together.

---

## Dependencies & Execution Order

- **Phase 1 (Setup)**: No dependencies — can start immediately, independent of everything else.
- **Phase 2 (Foundational)**: Depends on nothing external, but is a hard blocker for Phases 3-6 — T002→T004→T007→T008 is a strict chain; T003 and T006 and T009 can run in parallel with each other and with T004 (different files/no shared state).
- **Phase 3 (US1)**: Depends on Phase 2 complete (needs materialization wired into startup). T010-T012 (tests) can run in parallel with each other; T013-T014 depend on Phase 2's parser/materialization code existing but not on T010-T012.
- **Phase 4 (US2)**: Depends on Phase 2 complete and benefits from US1's env-var setup existing for its multi-provider test scenario (T015), but is conceptually independent — could be verified against zero providers too. Sequenced after US1 in this plan for test-setup convenience only.
- **Phase 5 (US3)**: Depends on Phase 2 (specifically T004's `config_source` field). Independent of US1/US2's runtime behavior — could be implemented in parallel with Phase 3 by a different contributor once Phase 2 is done.
- **Phase 6 (US4)**: Depends on Phase 2 (T005's parser). Independent of US1/US3's other work — the `LABEL` field is orthogonal to everything else T005 parses.
- **Phase 7 (Polish)**: Depends on all prior phases being complete.

## Parallel Execution Examples

- Within Phase 2: T003, T006, T009 in parallel once T002/T004/T005 respectively are ready.
- Across Phases 3/5/6 once Phase 2 is done: US1, US3, and US4's implementation tasks touch almost entirely different files (`LoginView.tsx`/`auth.ts` vs. `admin_types.rs`/`graphql.rs`/`OAuthProviderForm.tsx` vs. `oauth_env.rs`'s `LABEL` handling) and could be picked up by different contributors simultaneously.
- All Playwright test tasks (T010-T012, T015, T017-T018, T022-T023, T027) target the same new spec file (`apps/web/e2e/auth-providers.spec.ts`) — parallel-safe to *write* (marked [P] since they're independent scenarios with no shared mutable state), but note they'll need to land as one file, so the actual commit/merge step is sequential even though drafting isn't.

## Implementation Strategy

**MVP scope**: User Story 1 alone (Phases 1-3) delivers the feature's core value — zero-touch env-var provider enablement, including multi-instance and generic-provider support. User Story 2 (Phase 4) is a near-zero-cost regression guard that should ship alongside it. User Stories 3 and 4 (Phases 5-6) are valuable but independently deferrable — an operator gets full value from US1 alone even before the admin-panel override UX or label branding land.

**Suggested delivery order**: Phase 1 → Phase 2 → Phase 3 (US1) + Phase 4 (US2) as the MVP checkpoint → Phase 5 (US3) and Phase 6 (US4) in either order or in parallel → Phase 7.
