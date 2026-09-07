# Implementation Plan: Instance Setup and Configuration

**Branch**: `040-instance-setup` | **Date**: 2026-09-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/040-instance-setup/spec.md`

## Summary

A deployment becomes an instance in one guided pass: the first administrator's
account, who operates it, where a copyright notice is served, the support
address, and whether it can send mail — all of it editable afterwards from the
administration surface, all of it resolved by one precedence rule, and none of
it requiring a file to be edited.

Three things carry the feature, and each of them exists in the codebase in
exactly one place already:

- **One declaration list, one resolver.** Spec 007/ADR-041 settled "the
  environment wins" for OAuth and every subsystem since has been free to
  decide again. A declared setting resolves `Environment > Instance > Default`
  and reports which of the three it came from. A setting that is not declared
  does not resolve, which is what makes FR-012 ("a new setting inherits this
  rule") enforceable rather than hoped for.
- **Mail is a new subsystem, and the plan treats it as the largest single
  piece.** There is none — confirmed by search across every `Cargo.toml`,
  `Cargo.lock` and source file in the workspace. It arrives as a transport
  behind a trait (the shape `AppState.adjudicator` already uses), a durable
  outbox, and a background sender modelled line-for-line on
  `lore_sync/schedule.rs`, which is the only retry loop this codebase has and
  is a good one. Delivery is proved at three levels, one of which is a real
  SMTP server in the dev stack — because a mailer that has only ever talked to
  a mock is not a mailer.
- **Diagnostics that name a variable and never a value.** `repo_host.rs`'s
  `RegistrationProblem` enum already is FR-023, for one subsystem: every
  variant names something an operator can act on, `guidance()` produces prose
  naming variables, and `registration_from_env` returns *every* problem rather
  than the first. This feature generalises it to credentials, mail and
  readiness rather than inventing a second error vocabulary.

Two dependencies are named rather than absorbed: **spec 041 owns second-factor
enrolment** and this feature owns only setup's definition of finished (FR-002a);
**spec 039 owns the copyright rule** and this feature owns the gate that
enforces it (FR-026). Both are recorded in research.md so they cannot drift.

## Technical Context

**Language/Version**: Rust (2024 edition, server), TypeScript 5 / React 18 (web)

**Primary Dependencies**: Axum 0.8, async-graphql 7.2, Diesel 2.3/PostgreSQL 18, tower-cookies, argon2, aes-gcm; **new: `lettre` (SMTP, `tokio1-rustls-tls`)**; Playwright for e2e; **new dev/e2e service: Mailpit**

**Storage**: PostgreSQL. Three new tables (`instance_settings`, `instance_setting_changes`, `mail_outbox`). The realm manifest stays a JSON file on disk and is presented through the same surface rather than migrated (research.md § R1, § R6)

**Testing**: `cargo test --workspace -j 4` (native), `make lint` (lint-host + lint-wasm + file length), `pnpm --filter @thunderforge/web test`, Playwright via `scripts/e2e-parallel.mjs`

**Target Platform**: Chromium browsers only (constitution constraint); Linux server, container deployment

**Project Type**: Web application — Rust backend, React shell; no engine change at all

**Performance Goals**: setting resolution is a per-request read and must not add a query per setting (one row-set load per request, memoised for the request); the outbox sender ticks on the existing 30-second cadence and never blocks a mutation; SC-001's "under 10 minutes" is a human budget, not a machine one

**Constraints**: No diagnostic, log, screen or audit record may print a credential, a fragment of one, or its length (FR-023, FR-027, SC-007). An existing deployment must upgrade with no reconfiguration and must not fail to start (FR-024, FR-028). The instance must be fully usable with no mail configured (FR-017)

**Scale/Scope**: one deployment, one instance, one operator (explicitly not multi-tenant). ~20 declared settings at first landing; 6 of them are existing realm-manifest keys presented, not moved; 14 `[OPERATOR — …]` markers across two legal documents, of which 4 are fillable values and 10 are prose (research.md § D2)

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | PASS | Nothing here touches the canvas. The setup wizard and the admin panels are React chrome over server data, and they compute nothing: validation, precedence, redaction and readiness are all decided server-side and rendered. The one place the boundary is under pressure is the legal pages, where operator-supplied values enter compiled-in prose — handled by rendering substituted values as text, never through `LegalProse`'s inline parser (research.md § R4). |
| **II. Plugin-modular engine architecture** | PASS (not engaged) | No engine change, no new Bevy plugin, no wasm target touched. `make lint`'s `lint-wasm` leg must still be run per Principle V, but this feature contributes nothing to it. |
| **III. Ownership & authorization at the data boundary** | PASS | Every settings read and write is `admin_user(ctx)?` at the GraphQL boundary, as the existing admin surface already is. `instance_settings` and `mail_outbox` carry `updated_by`/`created_by` per ADR-009/ADR-010. FR-026's publish gate is enforced **in the share mutations themselves**, not in the client — it is an authorization decision and it is made where every other one is. The two unauthenticated surfaces are deliberate and narrow: the notice contact must be reachable without an account (spec 039 FR-056), and `/authentication/setup/status` already answers before anyone is signed in. |
| **IV. Real ADRs and specs before divergent implementation** | **ACTION REQUIRED** | Five architecturally significant decisions, all landing in this change set. **ADR-088** one precedence rule for every instance setting (extends ADR-041 from OAuth to the product). **ADR-089** mail as a transport seam with a durable outbox (a new subsystem and a new dependency). **ADR-090** credential resolution — scope outside, source inside (generalises `SYNC_GITHUB_APP_*`; decides what the spec left open, research.md § R10). **ADR-091** instance configuration is rows, and why not the manifest file and emphatically not `instance_identity` (research.md § R2). **ADR-092** operator values are substituted into compiled-in legal prose at render time, as text and from a closed token set — this moves a trust boundary `legalDocuments.ts` documents at length, so it is an ADR and not a helper. See "Numbering" below. |
| **V. Verify before claiming done** | PASS, with one gap named | Per-target checks are in the task plan: native `cargo test`/`clippy` for the server, `--target wasm32-unknown-unknown` for the engine (unchanged but still linted), `tsc`/`vitest` for the web, Playwright per story. The gap: **there is no e2e for first-run setup today and there cannot be one against the current harness**, because every shard's database is cloned from a template that is already migrated *and seeded* with `e2eadmin`. US1's independent test needs a migrated-but-unseeded database. That is a task, not a footnote (tasks.md T020). |

**DMCA / content-moderation guardrail**: **not engaged, and it builds a
precondition of the guardrail.** Nothing in this feature makes one world's
compendium content visible outside that world. What it adds is FR-026's gate —
an instance with no notice contact refuses the operations that publish beyond a
world — which is a control spec 039 and the guardrail both depend on. No
"centralized public repository" determination is required for this feature;
ADR-069, ADR-070 and ADR-071 already hold the determinations for the surfaces
being gated.

**Numbering**: 088–092 is a block deliberately chosen above everything any
sibling spec has claimed. The highest ADR on disk is 081; spec 036 reserves
073–075, spec 037 reserves 078 and 084–087, spec 039 reserves 080–083, and
spec 041 reserves 081–084 — several of which already collide with each other,
because 036, 037, 039 and 041 are being planned in parallel. This feature
stays clear of all of it. If the block ever has to move, every reference lives
in this directory, so a renumber is a find-and-replace and not an archaeology
exercise.

**Violations to justify**: three, all in Complexity Tracking below. This is a
feature that adds a subsystem, so an empty table there would be dishonest.

## Project Structure

### Documentation (this feature)

```text
specs/040-instance-setup/
├── plan.md                        # This file
├── research.md                    # Phase 0 — 15 decisions, plus § D, the disagreements
├── data-model.md                  # Phase 1 — three tables, one declaration list, one derived report
├── quickstart.md                  # Phase 1 — how to prove it, scenario by scenario
├── contracts/
│   ├── settings.md                # The declaration, the resolver, the admin surface
│   ├── setup.md                   # First run: the REST surface it extends, and "finished"
│   ├── mail.md                    # The transport seam, the outbox, the test message
│   ├── github-applications.md     # Two scales, one resolution order, one diagnostic vocabulary
│   ├── readiness.md               # The derived report and the publish gate
│   ├── legal-rendering.md         # Operator values into compiled-in prose, safely
│   └── e2e-fixtures.md            # The unseeded-database shard and the Mailpit sink
├── checklists/
│   └── requirements.md            # Written by /speckit-specify
└── tasks.md                       # Phase 2
```

### Source Code (repository root)

```text
src/server/src/
├── settings/
│   ├── mod.rs                     # NEW — Resolved { value, source }, the read path
│   ├── registry.rs                # NEW — the declaration list: key, kind, required, secret, env name, backing, capability
│   ├── resolver.rs                # NEW — Environment > Instance > Default, per request
│   ├── validate.rs                # NEW — FR-004's refusal rules, incl. the shipped defaults
│   └── changes.rs                 # NEW — the append-only change record, redacted per declaration
├── mail/
│   ├── mod.rs                     # NEW — MailTransport trait; Unconfigured refuses with a reason
│   ├── smtp.rs                    # NEW — the lettre transport, built from resolved settings
│   ├── capture.rs                 # NEW — test-support transport; never compiled into a release
│   ├── outbox.rs                  # NEW — enqueue, state transitions, operator-visible projection
│   └── schedule.rs                # NEW — spawn_mail_task; modelled on lore_sync/schedule.rs
├── github_apps.rs                 # NEW — registration_for(subsystem); scope outside, source inside
├── readiness.rs                   # NEW — derived from registry.rs; never a stored flag
├── repo_host.rs                   # registration_from_env stays; becomes step 1 of github_apps
├── crypto.rs                      # unchanged; reused for SMTP password, PEM, outbox body
├── admin.rs                       # manifest keys gain a source; update path unchanged underneath
├── auth/
│   ├── admin_setup.rs             # basic setup gains the rest of the pass; one transaction
│   └── admin_bootstrap.rs         # an unconsumed code is reused, not regenerated
├── graphql/
│   ├── queries/instance_settings.rs   # NEW — instanceSettings, instanceReadiness
│   ├── mutations_instance_settings.rs # NEW — updateInstanceSetting
│   ├── mutations_mail.rs              # NEW — sendTestMail; mailOutbox projection
│   ├── mutations_github_apps.rs       # NEW — setGithubApplication, checkGithubApplication
│   ├── admin_types.rs                 # GraphQLSystemManifest entries gain source/fixedBy
│   └── mutations_collection_shares.rs # the publish gate; and the three singleton-share siblings
└── lib.rs                         # pub mod settings; pub mod mail; pub mod github_apps; pub mod readiness

src/app/src/
└── main.rs                        # mail::schedule::spawn_mail_task beside the other five spawn_*_task calls

src/server/migrations/
├── 2026-09-07-000000-0000_instance_settings/          # NEW — settings + change record
└── 2026-09-07-000100-0000_mail_outbox/                # NEW — the outbox

legal/
├── terms-of-service.md            # [OPERATOR] markers become named tokens (values) or stay markers (prose)
├── privacy-policy.md              # same
└── README.md                      # the split between fillable values and operator prose blocks

apps/web/src/
├── pages/setup/
│   ├── SetupPage.tsx              # becomes a stepped pass over the same REST endpoints
│   └── steps/                     # NEW — account, operator, notices, support, mail, second factor (041), review
├── pages/admin/components/
│   ├── InstanceSettingsPanel.tsx  # NEW — every setting, its source, and what is fixed
│   ├── MailPanel.tsx              # NEW — settings, test message, the outbox
│   ├── GitHubAppsPanel.tsx        # NEW — global and per-subsystem, and what each acts for
│   ├── ReadinessPanel.tsx         # NEW — what this instance cannot do, and what to set
│   ├── adminSections.ts           # three entries; adding a section is one entry, by design
│   └── ManifestEditor.tsx         # renders "fixed by <VAR>" instead of a disabled field
├── legal/legalDocuments.ts        # gains the closed-token substitution, values as text
├── pages/legal/DmcaCompliancePage.tsx  # the agent designation reads settings instead of literals
└── api/instanceSettings.ts        # NEW — the GraphQL operations

apps/web/e2e/
├── instance-setup.spec.ts         # NEW — US1, against an unseeded database
├── instance-settings.spec.ts      # NEW — US2, US3
├── mail-delivery.spec.ts          # NEW — US4, against Mailpit
├── github-apps.spec.ts            # NEW — US5
├── instance-readiness.spec.ts     # NEW — US6, incl. the publish gate
└── fixtures/mailpit.ts            # NEW — read the delivered message back

compose.yml                        # mailpit, beside postgres and rustfs
scripts/e2e-parallel.mjs           # an unseeded template, and a mail sink port per shard
.env.example                       # the new families, documented as SYNC_GITHUB_APP_* already is
```

**Structure Decision**: no new crate and no new package. The server gains four
modules and four GraphQL surfaces; the web gains four admin panels and a
stepped setup; the harness gains one container and one database template. Mail
lives in `src/server/src/mail/` rather than a workspace crate because — unlike
dice, authentication rules or the cache — none of it is shared with the engine
or the client, and the one part that could be extracted (message rendering) has
no second consumer yet. `settings/` is a directory rather than one file for the
reason `check-file-length.sh` exists.

## Phase Summary

- **Phase 0 — research** (`research.md`): fifteen decisions, each with what it
  rejected, plus **§ D**, six places where the spec and the codebase disagree.
  Complete; no NEEDS CLARIFICATION remains, but § D1 and § D2 want the owner's
  eye before scheduling.
- **Phase 1 — design** (`data-model.md`, `contracts/`, `quickstart.md`):
  complete.
- **Phase 2 — tasks** (`tasks.md`): written.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| **A new subsystem with a background task and a new dependency** (`mail/`, `lettre`) | FR-013 to FR-017. Three shipped or specified features (035's invitations, 037's outcome messages, 039's strike and appeal notices) already promise delivery with nothing behind them. The spec is explicit that configuring mail without providing mail repeats the mistake at one remove. | *Settings only, delivery later*: leaves three features promising something no code does, and makes the first feature to need mail also the feature that has to invent it under deadline. *An HTTP mail provider*: an account a self-hoster does not have; the spec's assumption is SMTP first. |
| **A fourth container in `compose.yml` and a fifth port per e2e shard** (Mailpit) | FR-013's "a test message that proves it works **before** anything depends on it" is only proved by real SMTP. A capturing transport proves the code around the transport and nothing about the transport, which is where mail fails. | *Only the in-process capturing transport*: would let a broken `lettre` wiring, a wrong From address or a failed STARTTLS negotiation ship green. *A hand-written fake SMTP server in the test process*: a second SMTP implementation with none of the review of the first. |
| **A key/value settings table rather than typed columns** | FR-012 and FR-028 together: a new required setting must not need a migration and must not stop an existing instance starting. Typing lives in `registry.rs`, where it is one entry per setting and the declaration also carries redaction, the env name and the capability it enables — so a new setting cannot forget any of them. | *A wide singleton with a column per setting*: every new setting is a migration, and FR-028 becomes a schema problem. *A JSON blob column*: no per-key change record, no per-key source, and ADR-041 already rejected a single JSON env var for the same reason. |
