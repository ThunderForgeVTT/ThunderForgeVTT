# Phase 0 Research: Thunderforge Crucible Crate

This spec's design already has most of its groundwork done in
`docs/research/session-hosting-architecture-spike.md` §8 (a local, gitignored
document — not part of the shipped repo, referenced here for provenance only)
and [ADR-047](../../docs/adrs/20260825-047-crucible_session_adjudication_crate.md).
This file resolves the remaining concrete implementation-shape questions.

## 1. Transport between the main server and `crucible-server`

**Decision**: Plain HTTP + JSON via `axum` (server side) and `reqwest`
(client side) — both already `src/server` dependencies (confirmed:
`src/server/Cargo.toml` already depends on `axum 0.8.9` and
`reqwest 0.13.3`). No new RPC framework (e.g. `tonic`/gRPC) introduced.

**Rationale**: The workspace already has zero gRPC/protobuf tooling
anywhere. Introducing one for a single internal endpoint would be a new
category of dependency and build-tooling (a `.proto` compile step) for a
request/response shape simple enough that JSON-over-HTTP is not a
meaningful cost. Reusing `axum` also means `crucible-server`'s router code
looks and feels like every other route in this codebase — familiar to
future contributors — rather than introducing a second web-framework
idiom.

**Alternatives considered**: `tonic`/gRPC — rejected per above (new
tooling category for no proven need yet); a raw TCP/custom binary protocol
— rejected as premature optimization for a feature whose own spec
(Assumptions) says the real ruleset isn't being built yet.

## 2. Crate layout — one crate, `[lib]` + `[[bin]]`

**Decision**: A single Cargo package with both a library target and a
binary target, per the user's original framing and `plan.md`'s Project
Structure. The binary (`crucible-server`) depends on the library's own
`server.rs` module (an `axum::Router` builder function) rather than
duplicating routing logic — the binary crate is a thin `main()` that calls
into the library.

**Rationale**: Matches Rust's standard pattern for "library usable both
in-process and as its own server" (the router-builder-function-in-lib,
thin-binary-wrapper idiom is common in the `axum` ecosystem specifically).
Keeps exactly one source of truth for the HTTP contract (Phase 1
`contracts/`) regardless of which mode is deployed.

**Alternatives considered**: Two separate crates (a `-core` lib crate and a
separate `-server` bin crate) — rejected as unnecessary indirection for a
crate this size; revisit if the lib crate grows large enough that binary-only
dependencies (the `axum`/`reqwest` surface) become worth isolating from
consumers who only want `LocalAdjudicator` and don't want to pull in HTTP
server dependencies transitively. Noted as a future refactor if that
tension materializes, not a problem today.

## 3. Error handling for an unreachable remote adjudicator (FR-008)

**Decision**: `RemoteAdjudicator` wraps its `reqwest` call with an explicit,
bounded timeout (a fixed constant for this spec — not user-configurable
yet) and surfaces a distinct error variant (e.g.
`SessionAdjudicatorError::RemoteUnavailable`) rather than reusing a generic
error type — this lets the main server's call sites (out of scope for this
spec beyond the startup wiring) eventually decide how to present that
distinctly from "the action itself was rejected as invalid."

**Rationale**: Directly satisfies SC-004 ("clear error within a bounded
time, not an indefinite hang"). A distinct error variant (vs. a generic
string/anyhow error) keeps the door open for different handling later
(e.g., a future retry policy, or surfacing "adjudication service
unavailable" distinctly from "your move was illegal" in a UI) without
this spec having to design that UI/UX now.

**Alternatives considered**: No timeout (rely on TCP-level timeouts) —
rejected, TCP timeouts are typically much longer than what SC-004's "bounded
time" implies and are not under this crate's control; a retry-with-backoff
policy — rejected as scope creep for this spec (the spec's Assumptions
section explicitly limits scope to the seam, not resilience policy design).

## 4. Startup configuration validation (FR-005)

**Decision**: `CRUCIBLE_MODE` and `CRUCIBLE_ENDPOINT` are read once, at
`main.rs` startup, before the server begins accepting connections — mirroring
how `THUNDERFORGE_SECRET`/`DATABASE_URL` are already validated at startup
today (`main.rs` already `.expect()`s on `DATABASE_URL`, establishing the
existing "fail fast at boot, not at first request" convention this spec
follows). An unrecognized `CRUCIBLE_MODE` value, or `remote` mode with a
missing/malformed `CRUCIBLE_ENDPOINT`, causes the process to exit with a
clear error message naming the accepted values — not a panic with an opaque
message, not a silent fallback to `local`.

**Rationale**: Matches SC-003 exactly, and reuses an established pattern in
this codebase rather than inventing a new configuration-validation
convention.

**Alternatives considered**: Validate lazily on first adjudication request —
rejected, directly contradicts SC-003's "before any adjudication request is
attempted, in 100% of cases."
