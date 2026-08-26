# Implementation Plan: Client-Side World Cache with Content-Addressed Delta Sync

**Branch**: `028-client-world-cache` | **Date**: 2026-08-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/028-client-world-cache/spec.md`

## Summary

Give each client a durable, encrypted, per-world local store of scene state
and asset bytes, keyed by content fingerprint, so that reopening a world
transfers only what changed. Extend that store to let a disconnected client
keep working and reconcile on return, and let clients fetch bytes from each
other rather than always from the server.

The technical approach turns on one observation: **the same logic is needed
on both sides of the wire, and both sides are Rust.** The server computes
fingerprints, decides what a client is missing, and adjudicates conflicting
offline edits. The client computes fingerprints to verify what it received,
decides what to evict, and decides what to ask for. Those are the same
rules. Implementing them twice — once in Rust on the server, once in
TypeScript in the browser — would guarantee they drift, and a cache whose
two halves disagree about what "current" means is a correctness bug that
presents as missing art and lost edits.

So the policy goes in a shared, native-testable crate compiled into both the
server binary and the engine's WASM bundle, following the precedent set by
`thunderforge-canvas-core` (ADR-038) and `thunderforge-dice` (ADR-044). Only
the I/O differs per side: RustFS and Postgres on the server, OPFS and
WebCrypto in the browser.

## Technical Context

**Language/Version**: Rust (edition 2024) for shared policy, server, and
engine; TypeScript for UI chrome only

**Primary Dependencies**: Bevy 0.18 (WASM engine), Axum + async-graphql +
Diesel (server), `graphql-ws` (existing live-sync transport), WebCrypto and
OPFS via `web-sys` (browser I/O), `sha2` (fingerprints), `webrtc`-family
browser APIs via `web-sys` (peer transfer)

**Storage**: Server — PostgreSQL (state, fingerprints) + RustFS (asset
bytes, ADR-039). Client — OPFS for encrypted byte blobs, IndexedDB for the
index and the offline queue, both scoped per authenticated user

**Testing**: `cargo test` for the shared crate (native, no browser needed —
this is the point of the split), `cargo test --target wasm32` for browser
I/O adapters, Playwright for end-to-end cache/offline/peer behaviour

**Target Platform**: Modern browsers with OPFS, WebCrypto, and WebRTC; the
feature degrades to today's behaviour where any of these are unavailable

**Project Type**: Web application — Rust server, Rust/WASM engine, React
shell

**Performance Goals**: Unchanged-world reopen transfers ≤5% of first-visit
bytes (SC-001) and reaches interactive ≥3× faster (SC-002); fingerprint
verification must not become the new bottleneck on large assets

**Constraints**: Server stays authoritative (FR-039, FR-045); no telemetry
leaves the client (FR-052); local store unreadable after sign-out (FR-016a);
storage bounded by a proportion of browser-reported quota (FR-022)

**Scale/Scope**: Worlds up to tens of GB of asset bytes; a session of ~8
concurrent participants for peer transfer; offline queues of hundreds of
pending changes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| **I. ECS Owns Simulation, React Owns Chrome** | **PASS, with a named risk** | The cache is transport, not simulation, so it does not create a second source of truth for canvas state. The offline queue is the risk: it holds pending *changes*. Mitigated by storing emitted world-store commands verbatim rather than a parallel model — the queue is a durable outbox, never a simulator. Diagnostics UI (FR-051) is chrome and belongs in React. |
| **II. Plugin-Modular Engine Architecture** | **PASS** | Engine-side work ships as a self-contained plugin owning the asset-read path; it does not reach into other plugins' internals. |
| **III. Ownership & Authorization at the Data Boundary** | **PASS, load-bearing** | This is the principle the whole design leans on. FR-014 (re-authorize on every open), FR-042 (re-authorize queued changes at reconnect), FR-047 (never serve a peer what it cannot independently obtain). All three are enforced server-side; the client is never trusted to decide. |
| **IV. Real ADRs and Specs Before Divergent Implementation** | **FAIL — blocking** | Offline authoring and peer transfer both amend the server-authoritative posture recorded in ADR-046. **ADR-052 must be written and accepted before implementation begins.** See Complexity Tracking. This is not a formality: ADR-048 exists precisely because this obligation was skipped once and had to be recorded post-hoc. |
| **V. Verify Before Claiming Done** | **PASS** | Every success criterion is stated so it can be tested; the quickstart defines the runnable checks. |

### Additional guardrail check — DMCA / content moderation

The constitution requires an explicit determination before any feature makes
one world's content "visible, copyable, or otherwise accessible outside that
world." Peer transfer moves content between *users*, so the checkpoint
applies.

**Determination**: peer transfer does **not** constitute a centralized
public repository, and does not widen content access. FR-047 requires that a
peer only ever receives content it is independently permitted to obtain from
the server, and FR-050 confines connections to participants of the same
world session. Nothing becomes reachable that was not already reachable;
only the byte path changes. This mirrors ADR-049's reasoning for share
links and should be recorded in ADR-052 rather than left implicit.

**Gate result**: **CONDITIONAL PASS** — conditional on ADR-052 being
authored and accepted before implementation, covering: the amended authority
model (offline authoring), the DMCA determination above, the peer-transfer
default, and — most significantly — **peer-adjudicated play in the
server-isolated state** (FR-055 to FR-063), which moves peers from
distributing bytes to adjudicating state and is the largest single departure
from ADR-046.

## Project Structure

### Documentation (this feature)

```text
specs/028-client-world-cache/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── graphql-delta-sync.md
│   ├── cache-core-api.md
│   └── peer-protocol.md
├── checklists/
│   └── requirements.md  # From /speckit-specify
└── tasks.md             # /speckit-tasks output — NOT created here
```

### Source Code (repository root)

```text
crates/
├── thunderforge-cache-core/        # NEW — shared policy, no I/O, native-testable
│   ├── src/
│   │   ├── lib.rs
│   │   ├── fingerprint.rs          # canonical hashing of state and bytes
│   │   ├── manifest.rs             # what a client holds; diffing against server truth
│   │   ├── delta.rs                # fetch/evict plan computation
│   │   ├── budget.rs               # quota-proportional budget, LRU eviction choice
│   │   ├── queue.rs                # offline change queue ordering and replay rules
│   │   └── conflict.rs             # GM-over-player precedence, same-role tiebreak
│   └── tests/                      # runs under plain `cargo test` — no browser
│
└── thunderforge-cache-browser/     # NEW — wasm32-only I/O adapters
    ├── src/
    │   ├── lib.rs
    │   ├── opfs.rs                 # encrypted blob read/write
    │   ├── crypto.rs               # non-extractable session key, AES-GCM
    │   ├── index.rs                # IndexedDB index + durable outbox
    │   └── peer.rs                 # WebRTC data channel transfer
    └── tests/

src/
├── server/
│   ├── migrations/
│   │   └── NNNN_add_content_fingerprints/   # up.sql / down.sql
│   └── src/
│       ├── graphql/
│       │   ├── queries/world_sync_plan.rs   # NEW — the delta endpoint
│       │   └── mutations_reconcile.rs       # NEW — queued-change replay
│       ├── storage/rustfs.rs                # fingerprint on write
│       └── peer_signaling.rs                # NEW — WebRTC brokering only
│
└── engine/
    └── src/plugins/
        └── cached_assets.rs                 # NEW plugin — asset reads via cache

apps/web/src/
├── engine/world/sync/                       # existing sync modules, now cache-aware
└── components/diagnostics/                  # NEW — FR-051 diagnostics panel (chrome)
```

**Structure Decision**: Two new workspace crates plus targeted additions to
the existing server and engine crates. `thunderforge-cache-core` holds every
rule both sides must agree on and depends on nothing platform-specific, so
it runs under ordinary `cargo test` — the same reasoning ADR-038 used to
split `thunderforge-canvas-core` out for native testability, and the reason
that crate's logic is trustworthy today. `thunderforge-cache-browser`
isolates the parts that can only run in a browser, keeping the untestable
surface as small as possible. Server and engine consume both; TypeScript
gets only the diagnostics UI, because putting cache policy in the React
layer would be the second-source-of-truth mistake Principle I exists to
prevent.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| **Amends ADR-046's server-authoritative model** (Principle IV gate failure) | Offline authoring (US7) and peer transfer (FR-044) were both explicitly chosen by the accountable owner after the conflict was raised. Neither is possible under a strict reading of ADR-046. | A pure read-through cache would have satisfied ADR-046 untouched and was the original spec. It was rejected by the owner in favour of the wider scope. The obligation is therefore to *record* the amendment in ADR-052, not to avoid it. |
| **Two new crates rather than one** | Browser I/O cannot be exercised by `cargo test`; keeping it separate means the policy crate stays 100% native-testable and the untestable surface is small and obvious. | One combined crate would drag `wasm-bindgen`/`web-sys` into the policy layer, making the conflict and delta rules only testable in a browser — exactly the problem ADR-038 was written to solve. |
| **New client-to-client network path** | FR-044 through FR-050, owner-directed. Now **on by default**. | Server-only transfer is simpler and remains the mandatory fallback (FR-048). Peer transfer is strictly additive and can be disabled (FR-049). |
| **Peer-adjudicated play while server-isolated** (FR-055 to FR-063) | Owner-directed. A player who loses the server but still sees the whole party keeps playing, with the GM adjudicating, rather than being ejected mid-session. | Treating any server loss as full offline is simpler and was the prior design. Rejected by the owner: it ends a session for a momentary blip even when every participant can still see each other. **This is the single largest trust-model change in the feature** — peers move from distributing bytes to adjudicating state, and it must be the centrepiece of ADR-052. Split-brain is prevented by requiring *full* peer connectivity (FR-058), not a quorum. |
| **Encryption at rest on the client** | FR-016; a large store cannot be wiped instantly, so deletion alone leaves a readable window. | Best-effort deletion was considered and rejected during clarification for exactly that reason. |
