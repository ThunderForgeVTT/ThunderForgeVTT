# Feature Specification: Thunderforge Crucible Crate

**Feature Branch**: `024-thunderforge-crucible-crate`

**Created**: 2026-08-25

**Status**: Draft

**Input**: User description: "Add a new thunderforge-crucible crate to the Cargo workspace, at crates/thunderforge-crucible. This is the server-authoritative session-adjudication engine (movement and manipulation resolution) discussed and captured in docs/research/session-hosting-architecture-spike.md Section 8-9. It ships as one crate producing two build outputs: a library (`LocalAdjudicator`, pure Rust, no network, called in-process by the main thunderforge server — this is what every self-hosted deployment gets by default) and a standalone binary (`crucible-server`, a thin axum wrapper exposing the same adjudication logic over HTTP for out-of-process use — this is what a future enterprise KEDA-backed per-session orchestration layer would deploy one instance of per active session). The main thunderforge server should pick between the two via a runtime config flag (CRUCIBLE_MODE=local default, or CRUCIBLE_MODE=remote + CRUCIBLE_ENDPOINT=... ), both satisfying the same SessionAdjudicator trait so the rest of the server codebase doesn't need to branch on which mode is active. The crate is AGPL-3.0-or-later (workspace default, via license.workspace = true), consistent with the rest of the repo's just-completed relicense. Scope for this spec: the crate itself (trait + LocalAdjudicator + crucible-server binary + RemoteAdjudicator client) and wiring it into the main server behind CRUCIBLE_MODE — NOT the KEDA orchestration/session-controller layer itself, which is future/enterprise work tracked separately in the research doc."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A self-hosted deployment gets authoritative adjudication with zero extra setup (Priority: P1)

A GM/operator running ThunderForgeVTT self-hosted (the default, free deployment path) benefits from server-authoritative movement/manipulation resolution — the server, not just the connecting client, is the source of truth for whether a proposed move or manipulation is valid — without installing, configuring, or running any additional service. It works the same way the rest of the server already does: one binary, no new ops burden.

**Why this priority**: This is the floor requirement — the crate must not make the default (and only currently supported) deployment path worse or more complex. Every self-hoster gets this automatically; it's the reason the feature is worth building at all today, ahead of anything ephemeral/enterprise.

**Independent Test**: Run the existing `thunderforge` server binary with no new environment variables set. It resolves adjudication requests correctly and with no additional process, port, or configuration required beyond what already exists today.

**Acceptance Scenarios**:

1. **Given** a freshly started `thunderforge` server with no `CRUCIBLE_MODE` environment variable set, **When** an adjudication-eligible action (a move or manipulation) is submitted, **Then** the server resolves it in-process, with no network call to any other process.
2. **Given** the same default setup, **When** the server is inspected for new required ports, environment variables, or external processes, **Then** none are found beyond what already exists prior to this feature.

---

### User Story 2 - An operator can run adjudication as its own standalone process (Priority: P2)

An operator preparing for a future out-of-process deployment (e.g., testing the shape a per-session enterprise orchestration layer would eventually rely on) can run `crucible-server` as its own process, point the main `thunderforge` server at it via configuration, and get the identical adjudication behavior as the in-process mode — proving the standalone mode works correctly before any orchestration exists around it.

**Why this priority**: This is the prerequisite proof-of-concept for the eventual enterprise/KEDA orchestration layer (explicitly out of scope for this spec — see Assumptions), but has no value without User Story 1 first (the trait and its logic must exist before there's anything to expose over a network).

**Independent Test**: Start `crucible-server` as a standalone process, configure a `thunderforge` server instance to point at it, and submit adjudication-eligible actions through the normal flow — confirm identical results to User Story 1's in-process behavior.

**Acceptance Scenarios**:

1. **Given** a running `crucible-server` process and a `thunderforge` server configured to use it, **When** an adjudication-eligible action is submitted, **Then** the result matches what the same action would produce under in-process (local) mode.
2. **Given** a `thunderforge` server configured to use a remote adjudicator that is unreachable, **When** an adjudication-eligible action is submitted, **Then** the system produces a clear, actionable error rather than hanging indefinitely or silently falling back to a different resolution.

---

### Edge Cases

- What happens when `CRUCIBLE_MODE=remote` is set but `CRUCIBLE_ENDPOINT` is missing or malformed? The server MUST fail fast at startup with a clear error, not fail silently or fall back to local mode.
- What happens when the remote adjudicator becomes unreachable mid-session (network partition, process crash)? Covered by User Story 2's second acceptance scenario — a clear, actionable error, not a hang or a silent behavior change.
- What happens when `CRUCIBLE_MODE` is set to an unrecognized value? The server MUST fail fast at startup with a clear error naming the accepted values, rather than silently defaulting to one mode.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a `SessionAdjudicator` capability (resolving proposed movement and manipulation actions into an authoritative result) as a Rust trait, with at least two implementations: an in-process implementation requiring no network calls, and a network-client implementation that delegates to a separately-running process.
- **FR-002**: The in-process implementation MUST be usable as an ordinary library dependency by the main `thunderforge` server, with no additional runtime process required.
- **FR-003**: The system MUST provide a standalone, independently-runnable server process that exposes the same adjudication logic as the in-process implementation over a network interface, such that the network-client implementation (FR-001) can delegate to it and produce identical results to the in-process implementation for the same input.
- **FR-004**: The main `thunderforge` server MUST select which `SessionAdjudicator` implementation to use at startup via runtime configuration (not requiring a different compiled binary for each mode), defaulting to the in-process implementation when no configuration is provided.
- **FR-005**: When configured to use the network-client implementation, the main `thunderforge` server MUST validate its required configuration (at minimum, the target endpoint) at startup and fail fast with a clear error if that configuration is missing or malformed, rather than failing later or silently falling back.
- **FR-006**: The rest of the `thunderforge` server codebase MUST NOT need to know or branch on which `SessionAdjudicator` implementation is active — all call sites interact with the trait, not a specific implementation.
- **FR-007**: The new crate MUST be licensed AGPL-3.0-or-later, consistent with the rest of the workspace's license as of the 2026-08-25 relicense (`Cargo.toml`'s `[workspace.package]` default).
- **FR-008**: When the network-client implementation cannot reach its configured remote adjudicator at request time, the system MUST surface a clear, actionable error to the caller rather than hanging indefinitely or silently changing which implementation is used.

### Key Entities

- **SessionAdjudicator**: The capability/contract for resolving a proposed movement or manipulation action into an authoritative result. Two things satisfy it: an in-process resolver and a network-delegating client.
- **Adjudication request/result**: The proposed action (e.g., "move this token from A to B") and the authoritative outcome (accepted, rejected, or adjusted) it resolves to. Exact shape is a planning-phase concern, not fixed here.
- **Standalone adjudication process**: An independently-runnable process exposing the same adjudication logic as the in-process resolver, over a network interface, for use by the network-delegating client.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A self-hosted deployment with no new configuration set behaves identically, from an operator's perspective, to today's deployment — no new required ports, processes, or environment variables.
- **SC-002**: An adjudication-eligible action produces the same authoritative result whether resolved via the in-process implementation or the standalone-process implementation, for identical input, 100% of the time.
- **SC-003**: Misconfiguration of the network-delegating mode (missing/malformed endpoint, unrecognized mode value) is caught at server startup, before any adjudication request is attempted, in 100% of cases.
- **SC-004**: An unreachable remote adjudicator produces a clear error within a bounded time (not an indefinite hang) for every affected request.

## Assumptions

- The actual movement/manipulation adjudication *logic* (what makes a proposed move valid or invalid) is a planning-phase design concern, not fixed by this spec — this spec's scope is the crate's shape (trait, in-process implementation, standalone-process implementation, configuration-driven selection), not the game-rules content of the adjudication itself. A reasonable, minimal placeholder ruleset is acceptable for this feature; expanding adjudication sophistication is expected future work.
- The KEDA-backed, per-session orchestration/scaling layer described in `docs/research/session-hosting-architecture-spike.md` Section 8 is explicitly **out of scope** for this spec. This spec only builds the crate and the main server's ability to talk to either mode of it — not anything that provisions or tears down `crucible-server` instances automatically.
- Licensing/business-model decisions for any *future* orchestration layer (Section 9 of the research doc) do not apply to this spec's scope, since that layer isn't being built here; this crate itself is AGPL-3.0-or-later per FR-007, matching the rest of the workspace.
- The network transport between the main server and a standalone `crucible-server` reuses this workspace's existing HTTP stack (already a `thunderforge` server dependency) rather than introducing a new RPC framework — a planning-phase decision, noted here as the expected direction from the originating research, not a hard requirement of this spec.
