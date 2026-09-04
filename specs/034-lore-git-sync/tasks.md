# Tasks: Optional Lore Synchronisation to an External Repository

**Feature**: `034-lore-git-sync` · **Generated**: 2026-09-04
**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

## Format: `[ID] [P?] [Story] Description`

- **[P]** — parallelisable: different files, no dependency on an incomplete task
- **[US1] / [US2] / [US3]** — the user story a task serves. Setup, Foundational
  and Polish tasks carry no story label

## Why tests are included

The template treats tests as optional. They are not optional here, and that is
this project's standing position rather than a choice made for this feature:
Constitution Principle V requires verification before claiming done, and the
repository's practice is that an e2e moves a claim from theory to proven. Every
story below therefore carries its own tests, and the failure-mode story carries
most of them, because Story 2 *is* the tests.

---

## ⛔ Phase 0: The gate

**Nothing below may begin until T001 is done.** This is not a soft ordering.

**Cleared 2026-09-04** by [ADR-067](../../docs/adrs/20260904-067-a_user_initiated_mirror_is_not_a_public_repository.md), accepted by the project's owner. The determination and the cost it accepts are recorded there.

- [X] T001 Obtain and record the FR-042 determination — whether a user-initiated mirror to a repository the user owns constitutes a "centralized public repository" under spec 015's policy — as an ADR under `docs/adrs/` extending ADR-049, accepted by an accountable owner. The reasoning to weigh is already written in `specs/034-lore-git-sync/spec.md` under Assumptions ("The moderation posture is that mirroring is user-initiated distribution"), including the honest cost: a takedown cannot reach content already mirrored. Constitution v1.1.0's DMCA guardrail requires this **before implementation begins**, and spec 015 FR-012 says the same.

---

## Phase 1: Setup

- [X] T002 Write the ADR for the transport and grant decisions in `docs/adrs/` — git-over-HTTPS chosen over a host REST API on FR-004c grounds, and the application-installation grant model. Constitution Principle IV requires it to land in the same change set as the feature, not retroactively. See `research.md` R1 and R5.
- [X] T003 [P] Scaffold `crates/thunderforge-repo-host/` (package `thunderforge_repo_host`, `crate-type = ["rlib"]`, edition 2024, `[lints] workspace = true`), add it to the root `Cargo.toml` workspace members, and depend on it by path from `src/server/Cargo.toml`. **No `axum`, `diesel` or `reqwest` dependency** — state that constraint in the manifest as a comment the way `crates/thunderforge-axum-oauth/Cargo.toml` does, with the reason (`research.md` R5a).
- [X] T004 [P] Add the RS256 JWT signing dependency to `crates/thunderforge-repo-host/Cargo.toml`, plus `proptest` as a dev-dependency. Nothing in the workspace signs asymmetrically today — verified 2026-09-04, the crypto surface is `aes-gcm`, `sha2`, `rand`, `totp-rs` — so this is a deliberate addition (`research.md` R5).
- [X] T005 [P] Add a startup check for the `git` binary in `src/app/src/main.rs`, reporting its absence the way a partially-configured OAuth provider is reported rather than failing when a Game Master first connects (FR-036c). This is the server's first external binary dependency and there is no `Dockerfile` recording it (`research.md` R1).
- [X] T006 [P] Document the `git` requirement and the repository-application configuration in `README.md` and `docs/`, including that an instance without a registered application must answer `configured: false` (FR-036b).

---

## Phase 2: Foundational (blocking prerequisites)

**Everything here blocks every user story.** No story phase may start until this phase completes.

### Shared crypto

- [X] T007 Extract `encrypt_secret`, `decrypt_secret` and `encryption_key_from_config_secret` from `src/server/src/auth/mod.rs` into a new `src/server/src/crypto.rs`, re-exporting for existing callers so no behaviour changes. They are private today; copying an encryption routine is how two implementations drift until one is wrong (`research.md` R4).
- [X] T008 [P] Move the existing encryption unit tests alongside the extracted module in `src/server/src/crypto.rs`, and confirm `cargo test -p thunderforge-server` is green before and after — an extraction that changes a test is not an extraction.

### The grant crate

- [X] T009 [P] Define the crate's public surface in `crates/thunderforge-repo-host/src/lib.rs`: a `RepoHost` grant trait, an opaque `RepositoryCredential { token, expires_at }`, and the error type. **The return type is where FR-004c's boundary physically lives** — nothing that names a host may cross it.
- [X] T010 [P] Implement JWT claim construction and RS256 signing in `crates/thunderforge-repo-host/src/jwt.rs`. Pure: takes a key and a clock, returns a signed token.
- [X] T011 [P] Implement token-response parsing and the refresh decision in `crates/thunderforge-repo-host/src/token.rs` — given a cached credential and a clock, does it need refreshing. Pure, and property-testable.
- [X] T012 [P] Implement grant hand-off construction and single-repository scope validation in `crates/thunderforge-repo-host/src/github.rs` (FR-036a: a grant covering more than the one repository is refused). The grant requests **contents write and issue write** (FR-036e) — the second is what makes FR-040b's public disassociation possible, and a disassociation the product cannot perform is a commitment it should not make. Both must be shown to the user before they grant, with the reason for the second.
- [X] T013 Property-test the crate in `crates/thunderforge-repo-host/tests/` — refresh-window arithmetic across clock boundaries, scope validation rejecting a broader grant, claim construction. **`cargo test -p thunderforge_repo_host` must pass with no network and no application configured**; that is the whole point of the pure/effects split.
- [X] T014 Implement the effects half in `src/server/src/repo_host.rs` — the token-exchange HTTP call via the existing `reqwest` (rustls) client, and the credential cache. This is the only place in `src/server` that may name a host.

### Storage

- [X] T015 Write the Diesel migration in `src/server/migrations/` with paired `up.sql`/`down.sql` creating `lore_repository_connections`, `lore_sync_runs`, `lore_exported_entries` and `lore_fidelity_notes` per [data-model.md](./data-model.md). Include `UNIQUE (world_id)` (FR-001) and `UNIQUE (repository_ref, directory)` (FR-033) — enforcing those in application code is enforcing them nowhere. **Do not create `lore_pending_incoming_changes`**; it belongs to Story 3 and a table with no writer has a guessed shape.
- [X] T015a Extend the T015 migration in `src/server/migrations/` with `repository_is_public` and `visibility_checked_at` on `lore_repository_connections` (FR-040a), and the `lore_disassociation_notices` table per [data-model.md](./data-model.md) (FR-040b, FR-040d).
- [X] T016 Regenerate `src/server/src/schema.rs` and add the four models to `src/server/src/models.rs`, each carrying `created_by`/`updated_by` per Constitution Principle III.
- [X] T017 [P] Add a migration test in `src/server/src/lore_sync/mod.rs` (`#[cfg(test)]`) asserting the two unique constraints actually reject a second connection for a world and a second world claiming one repository directory. A constraint nobody tested is a comment.

### The sync primitives

- [X] T018 [P] Implement path mapping in `src/server/src/lore_sync/paths.rs`: tree position + title → path, with deterministic disambiguation for a title that normalises to nothing, siblings differing only by case or accent, and excessive depth or length. Pure, and the densest unit tests in this feature. **The path is a label, never a key** (`research.md` R7).
- [X] T019 [P] Implement the document format in `src/server/src/lore_sync/document.rs` — front matter (`id`, `title`, `tags`, `updated`, `unresolvable_links`), body preserved byte-for-byte, lore cross-links rewritten to relative paths, non-lore cross-links left readable and recorded. Per [contracts/repository-file-format.md](./contracts/repository-file-format.md).
- [X] T020 Unit-test `document.rs` for the round trip SC-008 requires: export an entry, parse it back, and get byte-identical markdown. Any reformatting, normalisation or prettifying fails this, which is the point.
- [X] T021 Implement the git wrapper in `src/server/src/lore_sync/git.rs` — clone, fetch, commit with distinct author and committer, `push --force-with-lease`, and `rev-parse` for verification. **Credentials via `GIT_ASKPASS` and the child process environment, never `argv`**: a token in a remote URL lands in the process table, which is worse than a log (FR-035, `research.md` R1).
- [X] T022 [P] Unit-test that `git.rs` never places a credential in a command's arguments, by inspecting the constructed invocation. This is the FR-035 failure that no other test would catch.

---

## Phase 3: User Story 1 — mirroring a world's lore outward (P1) 🎯 MVP

**Goal**: A Game Master connects a repository and sees their world's lore in it, with every edit arriving as an attributed commit.

**Independent test**: Connect an empty repository to a world containing lore, run the first synchronisation, clone it, and confirm the file tree and contents match the app. Then edit an entry and confirm a new commit appears carrying that change and naming that author.

### Implementation

- [X] T023 [US1] Implement the working clone lifecycle in `src/server/src/lore_sync/mod.rs` — one persistent clone per connection under a server-managed directory, treated as a **rebuildable cache**: losing it costs a re-clone and nothing else (`research.md` R2).
- [ ] T024 [US1] Implement one synchronisation pass in `src/server/src/lore_sync/run.rs` — fetch, diff the world against the clone, write files, commit, push, verify. Read and write on the same pass (FR-034b).
- [ ] T025 [US1] Implement commit identity in `run.rs`: **committer is `ThunderForge VTT <noreply@<instance domain>>`, author is the world member who wrote the revision** under a generated no-reply address, never a personal one (FR-017, `research.md` R6). One commit per revision in order (FR-016), with FR-020's bounded batching window for rapid successive edits.
- [ ] T026 [US1] Implement rename and move as a file move preserving history in `src/server/src/lore_sync/run.rs` (FR-010), using the `lore_exported_entries` row to know which file to move rather than inferring it.
- [ ] T027 [US1] Implement image mirroring in `src/server/src/lore_sync/run.rs`, writing to `<directory>/_images/` — uploaded originals only, referenced relatively, no derived renditions (FR-014).
- [X] T028 [US1] Exclude moderation-disabled entries in `src/server/src/lore_sync/run.rs`, and make that exclusion not block the rest of the world (FR-015).
- [X] T029 [US1] Record fidelity notes from `src/server/src/lore_sync/run.rs` into the `lore_fidelity_notes` table — unresolvable cross-links, permission flattening, path disambiguation (FR-013, FR-037). Rows rather than log lines: SC-008 requires losses to be *enumerated*, and something enumerable must be queryable.
- [ ] T029a [US1] Detect the connected repository's visibility in `src/server/src/repo_host.rs` and record it on the connection each run (FR-040a). **Observed, not guaranteed** — visibility changes at the host without telling us, and anywhere it is shown must say when it was last seen.
- [X] T029b [US1] Extend the pre-synchronisation notice in `apps/web/src/pages/world/settings/LoreRepositoryCard.tsx` to state distinctly whether the repository is public (FR-037a), and where it is, that a takedown will result in a public issue on it (FR-037b). **A private repository must not be assumed**: "everyone you invited" and "everyone on the internet" are different sentences, and a notice covering only the first is silently wrong for the users most exposed.
- [ ] T030 [US1] Implement `spawn_lore_sync_task` in `src/server/src/lore_sync/mod.rs` and call it from `src/app/src/main.rs` alongside the existing background tasks. **A connection with a null `notice_acknowledged_at` is never picked up** (FR-038).

### GraphQL

- [X] T031 [P] [US1] Implement the queries in `src/server/src/graphql/queries/lore_sync.rs` — `loreRepositoryConnection`, `loreSyncRuns`, `instanceRepositoryIntegration`. **No credential field at any depth, and no `installationRef` or `hostKind` in the API** (FR-035, FR-004c). Per [contracts/graphql-lore-sync.md](./contracts/graphql-lore-sync.md).
- [X] T032 [US1] Implement `beginLoreRepositoryConnection`, `completeLoreRepositoryConnection`, `acknowledgeLoreSyncNotice` and `removeLoreRepositoryConnection` in `src/server/src/graphql/mutations_lore_sync.rs`. Owner-level authority (FR-002), **re-checked per call and per run rather than captured at connection time** (FR-003).
- [X] T033 [US1] Add server tests in `src/server/src/graphql/mutations_lore_sync.rs` (`#[cfg(test)]`) for the authority rules: a non-owner refused, a second connection for a world refused (FR-001), a second world claiming one repository directory refused (FR-033), and a grant covering more than one repository refused (FR-036a).

### Web

- [X] T034 [P] [US1] Add the client in `apps/web/src/api/loreSync.ts`.
- [X] T035 [US1] Add the connection surface in `apps/web/src/pages/world/settings/LoreRepositoryCard.tsx` — connect, the FR-037 notice with its acknowledgement gate, and the current state. **Query `instanceRepositoryIntegration` before offering the flow**: a Game Master must never be shown something that cannot complete (FR-036b).

### Tests

- [ ] T036 [US1] Add `apps/web/e2e/lore-repository-sync.spec.ts` covering quickstart Scenarios 1 and 2 against a **local bare repository as the remote**, so no test touches a real host: first synchronisation produces the tree, an edit produces an attributed commit, a rename preserves file history, and no personal email address appears in `git log`.
- [ ] T037 [US1] Add an e2e assertion in `apps/web/e2e/lore-repository-sync.spec.ts` for the unconfigured instance (FR-036b) — with no application registered, the world's settings offer nothing connectable and the operator guidance is present. **Check this first when running by hand**: it is the state every self-hosted instance starts in and the easiest to leave broken, because nobody developing the feature ever sees it.
- [ ] T038 [US1] Verify SC-011 in `apps/web/e2e/lore-repository-sync.spec.ts`: a clone renders every entry, its images and its inter-entry links **with no network access to the platform**. This is what catches absolute URLs pointing back at the app, the likeliest way FR-012 and FR-014 get half-implemented.

**Checkpoint**: Story 1 is independently shippable. Nothing in it writes to a world's lore, so it cannot damage one by construction.

---

## Phase 4: User Story 2 — the connection fails and the world is unharmed (P2)

**Goal**: Every failure of the remote is cosmetic to the world.

**Independent test**: With a world synchronising normally, revoke the granted access. Confirm lore editing continues unchanged, the settings report the connection as broken with a cause and a remedy, and re-granting followed by a resynchronisation restores a faithful mirror.

### Implementation

- [ ] T039 [US2] Implement the connection state machine in `src/server/src/lore_sync/mod.rs` per [data-model.md](./data-model.md) — `working`, `needs_attention`, `never_configured`, `deactivated`. `needs_attention` **always** carries a `state_reason` naming the remedy in plain language, never a raw host error (FR-029).
- [ ] T040 [US2] Implement retry with progressively longer intervals in `src/server/src/lore_sync/mod.rs`, converging on correct contents once the cause is resolved with no user reconstruction (FR-030). Backoff reads `lore_sync_runs.attempt`, which is why runs are retained rather than overwritten.
- [ ] T041 [US2] Notify the Game Master once rather than repeatedly on continuing failure, in `src/server/src/lore_sync/run.rs` (Story 2 scenario 6).
- [ ] T042 [US2] Implement divergence detection in `src/server/src/lore_sync/run.rs`: where the remote history no longer contains the last state written, **stop and require an explicit choice** (FR-031). `push --force-with-lease` refuses this at the server; the run records `stopped_for_divergence`.
- [ ] T043 [US2] Implement `resolveLoreSyncDivergence` in `src/server/src/graphql/mutations_lore_sync.rs` — overwrite the divergent remote, or abandon the connection. **There is no third option that reconciles silently**, because reconciling would mean merging prose (FR-024).
- [ ] T044 [US2] Implement first-synchronisation collision safety in `src/server/src/lore_sync/run.rs`: never delete or modify a file the system did not write, and stop with an explanation on a collision inside the world's own directory (FR-032). Files outside that directory are untouched forever.
- [ ] T045 [US2] Implement write verification in `src/server/src/lore_sync/run.rs` against remote state already fetched (FR-034, FR-034b) rather than trusting a reported success.
- [ ] T046 [US2] Detect a revoked grant — an uninstalled application — in `src/server/src/repo_host.rs`, and surface it as a connection needing attention with that cause named, not as an uninterpretable synchronisation error (FR-036d).
- [ ] T047 [P] [US2] Implement enforcement deactivation in `src/server/src/graphql/mutations_lore_sync.rs` and `src/server/src/lore_sync/mod.rs` (FR-041a): a `deactivated` connection does not resume without an administrative action, and is distinguishable from one the owner removed and from one that is merely failing (FR-041c). Excluding a disabled entry remains the default response to a takedown; full deactivation is for when exclusion cannot stop republication, or for the repeat-infringer policy (FR-041b, spec 015 FR-016).
- [ ] T048 [US2] Notify the world owner from `src/server/src/moderation/mod.rs` when moderated content may already exist outside the platform's control, stating that removing it there is theirs to do (FR-040).

### Tests

- [ ] T048a [US2] Implement the disassociation issue in `src/server/src/lore_sync/disassociate.rs` (FR-040b): on a takedown affecting content mirrored to a **publicly visible** repository, lodge an issue using the body in [contracts/repository-file-format.md](./contracts/repository-file-format.md). It names no complainant, asserts no infringement, and reproduces no content — the platform records its own withdrawal, it does not adjudicate a claim it has no standing in. Never on a private repository (FR-040c), and it deletes and alters nothing in either case.
- [ ] T048b [US2] Record every attempt in `lore_disassociation_notices` from `src/server/src/lore_sync/disassociate.rs` (FR-040d) — `lodged`, `failed` or `skipped_private`. A failure MUST NOT block or reverse the takedown, and MUST reach an administrator. "We deliberately did not" and "we forgot" must not look the same in the record.
- [ ] T048c [P] [US2] Add server tests in `src/server/src/lore_sync/disassociate.rs` (`#[cfg(test)]`): a public repository gets an issue, a private one gets `skipped_private`, the body contains no complainant name and no content, and a failure to lodge leaves the takedown applied.
- [ ] T049 [US2] Extend `apps/web/e2e/lore-repository-sync.spec.ts` with quickstart Scenario 3's table — unreachable host, revoked grant, force-pushed branch, deleted repository. For each: **lore reading and editing indistinguishable from an unconnected world**, and zero instances of in-app lore being altered, hidden or lost (SC-005, SC-006).
- [ ] T050 [US2] Add an e2e in `apps/web/e2e/lore-repository-sync.spec.ts` for quickstart Scenario 4 — a repository with pre-existing files modifies zero files the system did not write (SC-007), and a second world is refused the same directory.
- [ ] T051 [P] [US2] Add server tests in `src/server/src/lore_sync/mod.rs` (`#[cfg(test)]`) for backoff intervals and for convergence after an outage: every edit made while broken appears, in order, with none duplicated or lost.
- [ ] T052 [US2] Verify SC-010 by inspection and in a test in `src/server/src/lore_sync/git.rs`: no credential in any log, response, or **process listing while a run is in flight** (quickstart Scenario 6).

**Checkpoint**: Stories 1 and 2 together are the **first delivery**. Story 3 is not part of it.

---

## Phase 5: User Story 3 — writing in the repository and bringing it back (P3) — ⏸ DEFERRED

**Not part of the first delivery.** Clarified 2026-09-04: export-first was confirmed, and Story 3 is separately scheduled and may never be built. These tasks are recorded so the shape is known, not so they are worked.

Do not start this phase without scheduling it deliberately. It is the only story that can put text into a world its members did not write in the app, and it is worthless until export is trusted.

- [ ] T053 [US3] Create `lore_pending_incoming_changes` via a migration in `src/server/migrations/` per the spec's Key Entities — deliberately absent from T015 because Stories 1 and 2 have no writer for it.
- [ ] T054 [US3] Detect incoming changes on the polling pass in `src/server/src/lore_sync/run.rs` (FR-034a — no inbound endpoint, ever).
- [ ] T055 [US3] Present pending changes via `src/server/src/graphql/queries/lore_sync.rs` without altering lore until accepted by a user with authority (FR-023).
- [ ] T056 [US3] Present both versions for a per-entry choice in `apps/web/src/pages/world/settings/LoreRepositoryCard.tsx` where an entry changed on both sides, **never merging prose** (FR-024).
- [ ] T057 [US3] Record an accepted change in `src/server/src/graphql/mutations_lore_sync.rs` as an ordinary revision attributed to the accepting user, marked as originating from the repository (FR-025).
- [ ] T058 [US3] Treat a file with no recognised durable identifier in `src/server/src/lore_sync/document.rs` as a proposed new entry, never matched by path or title (FR-027).
- [ ] T059 [US3] Require explicit confirmation for a deletion in `src/server/src/graphql/mutations_lore_sync.rs`, and restore the file on the next synchronisation if declined (FR-026).
- [ ] T060 [US3] Add e2e in `apps/web/e2e/lore-repository-sync.spec.ts` for quickstart's Story 3 flow, including that a world with incoming acceptance never enabled is never modified by anything in the repository (FR-022, Story 3 scenario 6).

---

## Phase 6: Polish & cross-cutting

- [ ] T061 Perform the FR-004a seam review: name every location that knows which host is in use. The answer must be `crates/thunderforge-repo-host` and `src/server/src/repo_host.rs` and nothing else. Mechanical check: **`grep -ri github src/server/src/lore_sync/` returns nothing.**
- [ ] T062 [P] Confirm SC-003 in `apps/web/e2e/lore-repository-sync.spec.ts` — an edit reaches the repository within 60 seconds under normal operation — by measurement rather than assertion.
- [ ] T063 [P] Confirm SC-001 as a timed walkthrough recorded in `specs/034-lore-git-sync/quickstart.md` — a Game Master connects and sees lore in under 5 minutes for a 200-entry world, without reading documentation. Not a unit test.
- [ ] T064 [P] Update `packs`-adjacent and top-level documentation to describe the feature, and add the user-facing terms FR-041 requires to `legal/` if the wording belongs there rather than in the connection notice.
- [ ] T065 Update `specs/034-lore-git-sync/checklists/requirements.md` — FR-042 becomes checked once T001 lands, taking the checklist to 29/29.
- [ ] T066 [~] Manual pass: quickstart Scenario 5 (moderation and fidelity), deferred to the playtest alongside the other manual passes in spec 032's `tasks.md`.

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 0 (T001)** blocks *everything*. It is a signature, not code.
- **Phase 1 (Setup)**: needs T001
- **Phase 2 (Foundational)**: needs Phase 1; **blocks every story**
- **Phase 3 (US1)**: needs Phase 2
- **Phase 4 (US2)**: needs Phase 3 — Story 2 is about Story 1 failing, so it cannot precede it
- **Phase 5 (US3)**: deferred; would need Phase 3 and Phase 4
- **Phase 6 (Polish)**: needs Phase 3 and Phase 4

### Within Phase 2

T007 blocks T008. T009 blocks T010, T011, T012, and T014. T013 needs T010–T012.
T015 blocks T016, which blocks T017. T018, T019 and T021 are independent of each
other; T020 needs T019; T022 needs T021.

### Within User Story 1

T023 blocks T024, which blocks T025, T026, T027, T028 and T029. T030 needs T024.
T031 and T034 are independent. T032 needs T031's types; T033 needs T032. T035
needs T034 and T031. T036–T038 need T030 and T035.

### Within User Story 2

T039 blocks T040, T041 and T046. T042 blocks T043. T044, T045 and T047 are
independent of each other. T049 needs T039–T046; T050 needs T044; T052 needs
T021.

### Parallel opportunities

- **Phase 1**: T003, T004, T005 and T006 are four different files
- **Phase 2**: the crate (T009–T013) and the storage work (T015–T017) are two
  independent tracks; the sync primitives (T018, T019, T021) are a third
- **Story 1**: the server track (T023–T030) and the web track (T034–T035) meet
  only at the contract, so they can run side by side
- **Story 2**: T047 and T051 are independent of the failure-handling chain

---

## Implementation Strategy

### The gate is real

T001 is not a formality to be worked around while code is written "in parallel".
The constitution's wording is "before implementation begins", and spec 015
FR-012 says the same. Writing the feature and then seeking the determination
inverts the checkpoint into a rubber stamp, which is the failure it exists to
prevent.

### MVP

**Phase 0 + Phase 1 + Phase 2 + Phase 3 (User Story 1).** That delivers a
working mirror: a Game Master connects a repository and their world's lore
appears in it with real history. It is independently valuable — a readable,
cloneable backup — and it cannot damage a world, because nothing in it writes to
one.

### Incremental delivery

1. Land T001. Nothing else until then.
2. Setup and Foundational. At the end of Phase 2 nothing works yet, which is
   normal for a phase whose job is to stop three stories each building their own
   half of the same thing.
3. Story 1 → the MVP. Ship it.
4. Story 2 → the mirror becomes trustworthy rather than merely working. This is
   where the "our path stays first-class" claim becomes true rather than stated,
   and it is the point at which the feature stops being a liability.
5. Stop. Story 3 is deferred and should be re-argued on its own merits before
   anyone starts it.

### A note on Story 2

It reads like polish and is not. Story 1 without Story 2 is a feature that
breaks a Game Master's session when GitHub has an outage. The first delivery is
both, and a plan that ships Story 1 alone has misread which half carries the
risk.
