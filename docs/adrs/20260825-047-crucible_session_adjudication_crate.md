# ADR-047: Crucible — A Pluggable, Dual-Mode Session-Adjudication Crate

**Date:** 2026-08-25
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

`docs/research/session-hosting-architecture-spike.md` (a local, gitignored
research doc — not part of the shipped repo) explored whether a world's
live "session" could become an independently-scalable, even
spin-up/spin-down-on-demand unit, distinct from the always-on monolith
server. That research found the current architecture has no server-side
compute to isolate at all: the client is fully authoritative for
movement/manipulation, and the server only persists already-decided
events. That's the actual blocker to any future per-session isolation or
elastic (KEDA-style) scaling story — there's nothing there yet worth
isolating.

Building a **real** server-authoritative adjudication capability — the
server, not just the connecting client, decides whether a proposed move
or manipulation is valid — is both a legitimate product capability (harder
to cheat, enforceable rules) and the prerequisite for that future
elastic-session story, since it's the first thing in this system with
actual per-session compute cost.

This is a new subsystem (Constitution Principle IV) and touches Principle
I (ECS owns simulation) directly enough to need explicit reconciliation,
not a silent addition.

## Decision

**Add `crates/thunderforge-crucible`: one crate, two build outputs, selected
by runtime configuration — not a compile-time fork, not two separate
crates.**

1. A `SessionAdjudicator` trait is the single contract for "resolve a
   proposed action into an authoritative result." Two implementations
   satisfy it:
   - `LocalAdjudicator` — pure Rust, in-process, no network. Linked
     directly into the main `thunderforge` server as an ordinary library
     dependency. This is what every self-hosted deployment gets, with
     zero additional configuration or ops burden.
   - A network-client implementation, delegating over HTTP (reusing the
     `axum` stack the main server already depends on — no new RPC
     framework) to a standalone `crucible-server` binary process, which
     exposes the identical adjudication logic over the network.
2. The main server picks between them at **startup**, via a
   `CRUCIBLE_MODE` environment variable (`local`, the default when unset;
   `remote`, requiring a `CRUCIBLE_ENDPOINT`) — not a different compiled
   binary per mode. Misconfiguration (missing/malformed endpoint,
   unrecognized mode) fails fast at startup, never silently.
3. Everything downstream of the trait — the rest of the `thunderforge`
   server codebase — depends only on `SessionAdjudicator`, never on which
   concrete implementation is active.
4. **Explicitly out of scope for this decision**: the actual movement
   adjudication *ruleset* (what makes a proposed move valid), and any
   KEDA/orchestration layer that would provision/tear down `crucible-server`
   instances per session. This ADR is about the crate's shape and the
   selection mechanism, not either of those.

### Reconciling this with Principle I (ECS owns simulation)

Principle I states Bevy is the single source of truth for canvas
simulation state and that presentation/other layers "MUST NOT re-implement
simulation or adjudication logic." Crucible's *initial* ruleset (this
spec's scope) is a **deliberate placeholder pass-through** — it does not
supersede or duplicate Bevy's simulation authority; Bevy-computed,
client-submitted state remains what's persisted, exactly as today. Crucible
today only builds the *seam* (trait, in-process default, standalone-process
alternative, config-driven selection) — not a competing simulation.

The intended **future** resolution, discussed and endorsed alongside this
decision but explicitly deferred to its own later spec/ADR: rather than
hand-writing a separate server-side rules-checker (which *would* be the
kind of duplicated adjudication logic Principle I warns against), the real
`SessionAdjudicator` implementation should eventually run the **same**
plugin-modular engine code (Principle II: `src/engine/src/plugins/*` are
already meant to be self-contained and portable) as a second, native/headless
build target — no window, no renderer, same ECS/movement systems — with the
client doing prediction and reconciling against that server-run instance as
the authority. That is Bevy remaining the single source of truth, just
running in two places (client-predicting, server-authoritative) instead of
one — the same pattern real-time multiplayer games use for exactly this
problem, and a *better* fit for Principle I than a hand-rolled server-side
rules engine would be. Building that headless native target, the
prediction/reconciliation protocol, and late-join state sync is real,
substantial future work — not part of this decision, and not blocking it:
the `SessionAdjudicator` trait boundary decided here is exactly the seam
that future implementation would slot into without a rewrite.

## Consequences

- New crate `crates/thunderforge-crucible`, workspace member, licensed
  `AGPL-3.0-or-later` (`license.workspace = true`, per the 2026-08-25
  relicense).
- Main server gains one new startup-time configuration surface
  (`CRUCIBLE_MODE`/`CRUCIBLE_ENDPOINT`), defaulting to today's zero-config
  behavior.
- No change to what's persisted or how movement/manipulation is resolved
  today — the placeholder ruleset does not alter existing gameplay
  behavior, satisfying Constitution Principle V's "don't regress the
  default deployment path."
- Establishes the seam a future headless-Bevy authoritative-simulation
  implementation, and any future KEDA-backed per-session orchestration
  layer, would build against — neither is committed to by this decision.

## Alternatives Considered

- **Hand-written server-side rules-checker as the long-term design**,
  duplicating movement-validity logic independent of the Bevy plugin
  systems — rejected as the intended long-term shape specifically because
  it *would* violate Principle I's "MUST NOT re-implement simulation logic"
  in a way the headless-shared-plugin approach does not; kept only as the
  short-term placeholder within this spec's narrow scope, not the design
  direction.
- **Two separate crates (one OSS, one enterprise-licensed)** for local vs.
  remote modes — considered per the research doc's earlier open-core
  framing, but superseded by the decision (recorded in the research doc,
  §9.3) to license the whole crate `AGPL-3.0-or-later` and rely on that
  license's deterrent effect rather than a closed/open code split.
- **Building the KEDA orchestration layer alongside this crate** — rejected
  as premature: there is no proven standalone `crucible-server` to
  orchestrate yet, and the research doc's own build order (§8.4) places
  orchestration last, after the trait/local/remote pieces are proven.
