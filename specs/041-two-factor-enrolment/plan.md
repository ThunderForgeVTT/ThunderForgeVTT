# Implementation Plan: Two-Factor Authentication People Can Turn On, Keep, and Recover

**Branch**: `041-two-factor-enrolment` | **Date**: 2026-09-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/041-two-factor-enrolment/spec.md`

## Summary

The verifier is finished. `crates/thunderforge-axum-auth-core/src/totp.rs`
builds SHA1 / 6 digits / 30-second step / skew 1 / issuer `ThunderForge`, with
every rule taking an explicit clock so the window is testable, and the secret
is already encrypted at rest. **Nothing in this plan changes an algorithm, a
digit count, a period or the skew.** What it builds is the human path around
them: an enrolment somebody can actually complete, recovery codes they can
reach at their worst moment, a removal that costs what adding it cost, and an
administrator rule that cannot be switched off.

Four decisions carry the feature, and each is a separation the current code
does not make:

- **A pending secret is not the live secret.** `two_factor_setup_start`
  (`src/server/src/auth/two_factor.rs:91`) writes the new secret *and* clears
  `two_factor_enabled` in one statement, on the password alone. Splitting the
  pending enrolment into its own row turns FR-004, FR-013 and FR-001c from
  three rules somebody has to remember into one shape of the schema — and it
  is what makes `apps/web/e2e/two-factor.spec.ts`'s `test.fail()` case pass,
  by fixing the product rather than the test.
- **One flow, three authorisations.** `login_two_factor_challenges` is
  already a single-use, expiring, account-bound ticket minted after a correct
  password. Giving it a `purpose` of `enrol` lets account settings, first-run
  setup and a login that requires enrolment open *the same* endpoints — so
  FR-001a's "identical in steps and wording" is a property of having one
  implementation, not a promise to keep three in sync.
- **A recovery code is a credential, not a link.** The instance already made
  this call correctly once: the admin bootstrap code is Argon2-hashed with the
  password helper (`auth/admin_bootstrap.rs:8`). Recovery codes follow it, not
  the raw-stored share codes, which is FR-009 by construction — there is
  nothing left to display.
- **The matched step is kept.** `totp.rs`'s own comment says totp-rs returns
  the step "so a caller can refuse to accept the same step twice", and then
  drops it. A high-water mark per account, advanced by a conditional `UPDATE`
  whose zero-row result is the refusal, closes FR-016 under concurrency
  without a lock.

The administrator rule (FR-027 to FR-033) is computed from `users.is_admin`
and stored nowhere, so there is no column an `UPDATE` could switch off, and
first-run setup does not complete until that administrator's factor is
confirmed and their recovery codes are issued — which must work on an
instance with no mail, because that is the only kind of instance a first-run
setup ever runs on.

## Technical Context

**Language/Version**: Rust (2024 edition, server + crates), TypeScript 5 / React 19 (web)

**Primary Dependencies**: Axum 0.8 + `tower-cookies` (auth is REST, not GraphQL), Diesel/PostgreSQL, `argon2` 0.6, `totp-rs` 6.0 (`otpauth`), `aes-gcm` 0.11 via `src/server/src/crypto.rs`; React + Radix/shadcn on the web; Playwright for e2e. **One new server crate**: a pure-Rust QR encoder (research.md § R10). **No new web dependency.**

**Storage**: PostgreSQL. Two new tables (`two_factor_enrolments`, `user_recovery_codes`), one new event table (`two_factor_events`), two new columns on `users`, two on `login_two_factor_challenges`

**Testing**: `cargo test --workspace -j 4` (native), `pnpm --filter @thunderforge/web test` (vitest), Playwright via `scripts/e2e-parallel.mjs`; `make lint` = lint-host + lint-wasm + file length

**Target Platform**: Chromium browsers only (constitution constraint); Linux server

**Project Type**: Web application — Rust backend, React shell. **The engine is not touched by this feature at all.**

**Performance Goals**: Enrolment completes in under three minutes unaided (SC-001); a recovery-code verification costs ten sequential Argon2 verifications (~19 MiB peak, on the blocking pool) on a path reached only after a correct password

**Constraints**: Enrolment MUST NOT depend on mail (FR-001b) — there is no mail subsystem and spec 040 owns building one. The verifier's parameters may not change. `scripts/check-file-length.sh` caps a Rust file at 1000 lines and `auth/two_factor.rs` is already 491

**Scale/Scope**: 5 new REST routes, 1 admin GraphQL query, 3 new web surfaces (account security settings, the enrolment flow component, the setup step), 1 rewritten e2e spec

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | PASS | Not engaged. Nothing here touches the canvas, the engine crate or any simulation state; enrolment is React chrome over REST auth handlers. The engine is not rebuilt by this feature. |
| **II. Plugin-modular engine architecture** | PASS | Not engaged. No engine plugin, no `src/engine/` change. |
| **III. Ownership & authorization at the data boundary** | PASS | Strengthened, not merely preserved. `two_factor_setup_start` is currently **unauthenticated** and identifies the account by a `username` in the body; it becomes a surface that requires a session or a server-minted ticket, so enrolment can no longer be started *for* somebody. Every recovery code, event row and enrolment row is account-scoped and adjudicated server-side. New tables carry `created_by`/`updated_by` where the row has an actor distinct from its subject (`two_factor_events`); the two per-account tables are self-owned by `user_id`, matching `user_sessions`' existing shape. |
| **IV. Real ADRs before divergent implementation** | **ACTION REQUIRED** | Four architecturally significant decisions, each landing in the same change set: **ADR-081** a confirmed second factor is replaced, never disarmed (supersedes the `setup/start` behaviour at `auth/two_factor.rs:91`); **ADR-082** one enrolment flow, three authorisations (extends `login_two_factor_challenges` with a purpose); **ADR-083** recovery codes are credentials, hashed like the bootstrap code and never readable again; **ADR-097** the administrator's second factor is a property of the role, and gates first-run completion (amends the setup path ADR-072 established, and pairs with spec 040's FR-002a). **Numbering is a reservation, not a fact yet**: specs 036–040 were planned in the same pass and, at the time of writing, 036's plan claims 073–075 while 037, 039 and 040 each independently claim 076–080. 041 therefore takes **081–084**, the first block nothing else has claimed, and whoever merges these five plans should reconcile the whole range in one pass rather than per feature. The matched-step guard (FR-016) deliberately gets **no** ADR — see research.md § R4 for why. |
| **V. Verify before claiming done** | PASS | Per-target checks are in the task plan: native `cargo test`/`clippy` for the server and the auth crate, `tsc`/`vitest` for the web, Playwright for every user story, and `make lint` including the file-length gate that `auth/two_factor.rs` is being split ahead of (research.md § R11). No wasm target is involved. |

**DMCA / content-moderation guardrail**: not engaged. Nothing here exposes any
world's compendium content anywhere; the feature never reads world content.

**No violations to justify.** The Complexity Tracking table is empty by
design. The one new dependency (a QR encoder crate, server-side) is argued in
research.md § R10 against the three alternatives, one of which — an external
QR image service — would have put the TOTP secret in a third party's URL.

## Project Structure

### Documentation (this feature)

```text
specs/041-two-factor-enrolment/
├── plan.md                  # This file
├── research.md              # Phase 0 — eleven decisions and what each rejected
├── data-model.md            # Phase 1 — tables, columns, lifetimes, transitions
├── quickstart.md            # Phase 1 — how to prove it, scenario by scenario
├── contracts/
│   ├── enrolment.md         # setup/start, setup/confirm, and the three authorisations
│   ├── recovery-codes.md    # issuing, spending, regenerating, running low
│   ├── removal-and-reset.md # deliberate removal, and the operator reset path
│   ├── requirement-policy.md# who must enrol, the admin rule, the operator's view
│   └── verification.md      # the step guard, the challenge counter, refusal shapes
├── checklists/
│   └── requirements.md      # Written by /speckit-specify
└── tasks.md                 # Phase 2
```

### Source Code (repository root)

```text
crates/thunderforge-axum-auth-core/src/
├── totp.rs                      # gains a step-returning form; parameters unchanged
├── recovery_codes.rs            # NEW — generation + the constant-work match rule
└── random.rs                    # `random_setup_code`'s alphabet reused for recovery codes

src/server/src/auth/
├── two_factor.rs                # DELETED — becomes the directory below (R11)
├── two_factor/
│   ├── mod.rs                   # route wiring and the shared request types
│   ├── enrolment.rs             # setup/start, setup/confirm — writes users.* only at confirm
│   ├── recovery.rs              # issue, spend, regenerate, count-remaining
│   ├── verification.rs          # the challenge, the step guard, the attempt counter
│   ├── policy.rs                # is-required-for, the admin rule, the instance switch
│   └── events.rs                # two_factor_events, and the best-effort notify seam
├── sessions.rs                  # `hash_password` promoted to a shared helper
├── admin_setup.rs               # setup/basic returns an enrol ticket, not a session
├── admin_bootstrap.rs           # completion is marked at 2FA confirmation, not before
└── mod.rs                       # the five new routes

src/server/src/
├── crypto.rs                    # unchanged; the pending secret uses the same envelope
├── qr.rs                        # NEW — otpauth URI -> module matrix, nothing else
├── migrations/                  # three migration directories, up.sql + down.sql each
└── graphql/queries/admin.rs     # the enrolled/not-enrolled counts (FR-021)

apps/web/src/
├── api/twoFactor.ts             # NEW — the five routes, typed
├── components/two-factor/
│   ├── EnrolmentFlow.tsx        # NEW — the one flow: QR, typeable secret, confirm, codes
│   ├── QrMatrix.tsx             # NEW — inline SVG from the matrix; no dependency, no HTML injection
│   └── RecoveryCodeSheet.tsx    # NEW — shown once, copyable, downloadable, acknowledged
├── pages/user/
│   └── SecuritySettingsPage.tsx # NEW — /settings/security: state, enrol, remove, codes
├── pages/auth/LoginView.tsx     # the enrolment branch of the challenge step
├── pages/setup/SetupPage.tsx    # the enrolment step, before setup completes
├── pages/admin/components/
│   ├── SecurityPanel.tsx        # gains the enrolled/not-enrolled figures
│   └── UserTwoFactorControl.tsx # NEW — per-account requirement and operator reset
└── routes/AppRoutes.tsx         # /settings/security, beside /settings/storage

apps/web/e2e/
└── two-factor.spec.ts           # rewritten: the UI it says does not exist now does
```

**Structure Decision**: no new project or package. Auth in this codebase is
REST (`src/server/src/auth/`, merged under `/api`), not GraphQL — confirmed
before planning, because putting enrolment on the GraphQL root would have
split one feature across two authentication models. The one GraphQL addition
is the administrator's *count* of enrolled accounts (FR-021), which belongs
beside `authSecuritySettings` on `AdminQuery` where the policy switch it
explains already lives.

`auth/two_factor.rs` becomes a module directory in the same change set rather
than afterwards: it is 491 lines against a 1000-line gate
(`scripts/check-file-length.sh`), and this feature roughly doubles it. Doing
the split first means every task below names a small file; doing it afterwards
means one unreviewable commit of pure movement.

The recovery-code generator and the constant-work match rule go in
`thunderforge-axum-auth-core` for the reason that crate exists — "the
authentication rules, extracted so they can be tested (and proptested)
without a database, a request or a provider account behind them"
(`src/server/Cargo.toml`). "Ten codes, each used at most once, and the same
work whether one matched or none did" is exactly a proptest.

## Phase Summary

- **Phase 0 — research** (`research.md`): eleven decisions, each with what it
  rejected and what in the codebase grounds it. Complete; no
  NEEDS CLARIFICATION remains.
- **Phase 1 — design** (`data-model.md`, `contracts/`, `quickstart.md`):
  complete.
- **Phase 2 — tasks** (`tasks.md`): written, organised by user story, with an
  MVP slice of US1 + US2 + US4.

## Dependencies on other features

- **Spec 040 (instance setup)** owns the first-run wizard and the mail
  capability. This feature owns the enrolment step inside that wizard and the
  condition under which setup is complete; spec 040's FR-002a states the same
  gate deliberately. **Neither blocks the other**: FR-001b requires enrolment
  to work with no mail at all, so 041 can ship first and 040's mail work
  fills the notification seam (`auth/two_factor/events.rs`) when it lands.
  Until then the record exists and the person is told in the product, which
  is what FR-026 asks for anyway.
- **Spec 036 (concurrent sessions)** touches `auth/sessions.rs` and
  `auth/two_factor.rs` in the same region. Its T015 already names the
  two-factor session path. If both are in flight, 036's plan reserves
  ADR-073–075 and this one takes 076–079; the file-level conflict is
  `auth/sessions.rs`'s session creation, which this feature only reads.

## Complexity Tracking

No constitution violations. Table intentionally empty.
