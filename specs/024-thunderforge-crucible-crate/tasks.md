---

description: "Task list for feature implementation"
---

# Tasks: Thunderforge Crucible Crate

**Input**: Design documents from `/specs/024-thunderforge-crucible-crate/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/crucible-server-http.md, quickstart.md

**Tests**: Included — matches this project's established convention (backend `cargo test` coverage per feature; see quickstart.md's "Automated coverage expectations").

**Organization**: Tasks are grouped by user story (P1/P2 from spec.md) so each can be implemented, tested, and shipped independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2)

## Path Conventions

New crate at `crates/thunderforge-crucible/` (matches `crates/thunderforge-dice`,
`crates/thunderforge-canvas-core` convention); wiring changes in `src/server/`.
No frontend changes, no migrations — see plan.md.

---

## Phase 1: Setup

- [X] T001 Create `crates/thunderforge-crucible/Cargo.toml`: `[lib]` + `[[bin]] name = "crucible-server"`, `license.workspace = true`, dependencies on `serde`/`serde_json` (+ `uuid`, matching workspace versions used elsewhere e.g. `crates/thunderforge-dice/Cargo.toml`), `axum` and `tokio` (for the bin target and `server.rs`), `reqwest` (for `remote.rs`) — all versions matching what `src/server/Cargo.toml` already pins, per research.md §1
- [X] T002 Add `"crates/thunderforge-crucible"` to the root `Cargo.toml`'s `[workspace] members` list

**Checkpoint**: `cargo check -p thunderforge-crucible` succeeds against an empty crate skeleton.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `SessionAdjudicator` trait and shared types every user story's implementation depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T003 In `crates/thunderforge-crucible/src/lib.rs`: define `AdjudicationRequest`, `AdjudicationResult` (with its `Outcome` enum: `Accepted`/`Rejected`/`Adjusted`), and `SessionAdjudicatorError` (with `RemoteUnavailable`/`InvalidRequest` variants) per data-model.md, all `Serialize`/`Deserialize`; define the `SessionAdjudicator` trait itself (`async fn resolve(&self, req: AdjudicationRequest) -> Result<AdjudicationResult, SessionAdjudicatorError>`) — FR-001
- [X] T004 [P] `cargo test` coverage in `crates/thunderforge-crucible/src/lib.rs` (or a `tests` module) for `AdjudicationRequest`/`AdjudicationResult` JSON (de)serialization round-tripping correctly for each `Outcome` variant, per quickstart.md's automated-coverage expectations

**Checkpoint**: The trait and shared types exist and compile — both user stories' implementations can now proceed independently.

---

## Phase 3: User Story 1 - Self-hosted deployment gets adjudication with zero extra setup (Priority: P1) 🎯 MVP

**Goal**: The default (local) mode works with no new configuration, ports, or processes — every self-hosted deployment gets this automatically.

**Independent Test**: Per spec.md — run the existing `thunderforge` server binary with no new environment variables set; an adjudication-eligible action resolves in-process, with no network call to any other process.

- [X] T005 [US1] In `crates/thunderforge-crucible/src/local.rs`: implement `LocalAdjudicator` satisfying `SessionAdjudicator` — the placeholder ruleset always returns `Outcome::Accepted` for a well-formed request and `SessionAdjudicatorError::InvalidRequest` for a malformed one (unrecognized `kind`), per data-model.md and ADR-047's "deliberate placeholder pass-through" framing
- [X] T006 [P] [US1] `cargo test` coverage in `crates/thunderforge-crucible/src/local.rs` for `LocalAdjudicator`'s placeholder behavior (accepts well-formed requests, rejects malformed ones) — quickstart.md's automated-coverage expectations
- [X] T007 [US1] In `src/server/src/state.rs`: add an `adjudicator: Arc<dyn SessionAdjudicator + Send + Sync>` field to `AppState` (matches the existing `Arc<RwLock<SystemHookRegistry>>` pattern already on `AppState` for a shared, boot-time-constructed service)
- [X] T008 [US1] In `src/server/src/main.rs`: read `CRUCIBLE_MODE` at startup (default `"local"` when unset), construct a `LocalAdjudicator` and wrap it into `AppState.adjudicator` for this mode — mirrors the existing `DATABASE_URL`/`THUNDERFORGE_SECRET` startup-read pattern already in this file, per research.md §4
- [X] T009 [US1] Add `thunderforge-crucible` as a path dependency in `src/server/Cargo.toml`

**Checkpoint**: User Story 1 is independently functional — `cargo check`/`cargo test` for `src/server` passes with the default (local) mode wired in, no behavior change to existing gameplay (placeholder ruleset only).

---

## Phase 4: User Story 2 - An operator can run adjudication as its own standalone process (Priority: P2)

**Goal**: `crucible-server` runs standalone; the main server can be pointed at it via `CRUCIBLE_MODE=remote` and produces identical results to local mode; misconfiguration and unreachability both fail clearly, never silently or by hanging.

**Independent Test**: Per spec.md — start `crucible-server` as a standalone process, configure a `thunderforge` server instance to use it, and confirm adjudication results match local mode exactly; confirm a clear, bounded-time error (not a hang) when the remote adjudicator is unreachable.

- [X] T010 [US2] In `crates/thunderforge-crucible/src/server.rs`: an `axum::Router` builder function exposing `POST /adjudicate` (delegates to an injected `LocalAdjudicator`, returns `200` + `AdjudicationResult` JSON on success, `400` on `SessionAdjudicatorError::InvalidRequest`) and `GET /health` (`200 OK`, empty body) — per contracts/crucible-server-http.md
- [X] T011 [US2] In `crates/thunderforge-crucible/src/bin/crucible-server.rs`: thin binary entrypoint — reads a listen address/port, builds the router from T010, serves it via `axum::serve` — per plan.md's "thin binary wrapper" convention
- [X] T012 [US2] In `crates/thunderforge-crucible/src/remote.rs`: implement `RemoteAdjudicator` satisfying `SessionAdjudicator` — POSTs to `{CRUCIBLE_ENDPOINT}/adjudicate` via `reqwest` with a bounded timeout (research.md §3's fixed-constant timeout), mapping connection failure/timeout/non-2xx-non-400 responses to `SessionAdjudicatorError::RemoteUnavailable` and `400` responses to `SessionAdjudicatorError::InvalidRequest` — per contracts/crucible-server-http.md
- [X] T013 [P] [US2] `cargo test` integration coverage in `crates/thunderforge-crucible` spinning up the T010 router in-process (`tokio::spawn` + an ephemeral port, no separately-run process needed in CI) and exercising `RemoteAdjudicator` against it, asserting identical results to `LocalAdjudicator` for the same input — proves User Story 2's core "identical result" claim (SC-002), per quickstart.md/plan.md's Testing note
- [X] T014 [P] [US2] `cargo test` coverage for `RemoteAdjudicator` against an unreachable endpoint (e.g. a closed/unbound port), asserting `SessionAdjudicatorError::RemoteUnavailable` is returned within the bounded timeout, not an indefinite hang — SC-004
- [X] T015 [US2] In `src/server/src/main.rs`: extend the T008 startup config-read to handle `CRUCIBLE_MODE=remote` — read `CRUCIBLE_ENDPOINT`, construct a `RemoteAdjudicator` pointed at it; an unrecognized `CRUCIBLE_MODE` value, or `remote` mode with a missing/malformed `CRUCIBLE_ENDPOINT`, MUST exit the process immediately with a clear error naming the accepted values/missing variable — FR-005, SC-003
- [X] T016 [P] [US2] `cargo test` coverage in `src/server` for the T015 config-parsing logic in isolation (valid `local`, valid `remote` + valid endpoint, invalid mode value, `remote` with missing endpoint) — quickstart.md's automated-coverage expectations, matching quickstart.md §3 as unit-testable logic rather than only a manual walkthrough

**Checkpoint**: Both user stories complete and independently verified — the crate's local and remote modes are interchangeable, and misconfiguration/unreachability both fail clearly per SC-003/SC-004.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T017 Run `cargo check`/`cargo test` for the full workspace (native target — this crate has no WASM surface) and resolve any new warnings introduced by this feature (Constitution Principle V); pre-existing warnings/failures unrelated to this feature are not blocking
- [X] T018 Full quickstart.md walkthrough (all 4 sections) against a running dev instance before calling the feature done

---

## Dependencies & Execution Order

- **Setup (Phase 1)** blocks Foundational — the crate must exist before the trait can be defined in it.
- **Foundational (Phase 2)** blocks both user stories — the `SessionAdjudicator` trait and shared types are what both `LocalAdjudicator` and `RemoteAdjudicator` implement.
- **User Story 1 (Phase 3)**: depends only on Foundational. This is the MVP — a self-hosted deployment gets adjudication with zero new configuration, demoable alone.
- **User Story 2 (Phase 4)**: depends on Foundational directly (not on US1's `src/server` wiring, since T010-T014 are all within the `thunderforge-crucible` crate itself), but T015 (the `main.rs` `remote` branch) extends the same startup-config block US1's T008 establishes, so T015 comes after T008 in practice even though the crate-side remote work (T010-T014) could proceed in parallel with US1.
- **Polish (Phase 5)**: depends on both stories being complete.

## Parallel Execution Examples

- Within Foundational: T004 is `[P]` alongside nothing else in its phase (only one other task, T003, which T004 depends on).
- Within US1: T006 is `[P]` with T007/T008/T009 (different files).
- Within US2: T013/T014 are `[P]` with each other and with T015 (test files vs. the `main.rs` wiring); T016 is `[P]` alongside them.
- Across stories: US2's crate-side work (T010-T014) has no file overlap with US1's `src/server` wiring (T007-T009) and could proceed in parallel by a second contributor once Foundational is done — only T015 (which extends T008's same code block) is sequenced after US1.

## Implementation Strategy

**MVP = User Story 1 alone** (Phase 1 → 2 → 3): every self-hosted deployment gets the adjudication seam with zero new configuration, and the trait boundary is proven with a real (if placeholder) implementation. Recommended incremental delivery: Setup → Foundational → US1 (MVP checkpoint) → US2 (standalone-mode proof, the prerequisite for any future orchestration work per ADR-047) → Polish.
