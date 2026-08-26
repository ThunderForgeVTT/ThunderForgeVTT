# ADR-052: The Client May Hold, Continue, and Distribute — With the Server as Record and the GM as Arbiter

**Date:** 2026-08-26
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Accountable owner:** Michael Bruno, project owner

**Amends:** ADR-046 (Server-Authoritative Active Scene). Builds on ADR-039
(RustFS scoped asset storage), ADR-044 (dice trust boundary), ADR-048
(`graphql-ws` live-sync transport), ADR-050 (permission declaration).
**Implements:** spec `028-client-world-cache`.

---

## Problem Statement

Three problems share one cause: **the client is not allowed to remember
anything.**

1. **Every load is a cold load.** Opening a world, switching scenes, or
   reloading the page refetches all scene state and every byte of every map
   background and token image. A group that plays in the same three maps
   every week downloads those maps every week, and every player pays it
   again on every mid-session reload.

2. **A momentary blip ends a session.** Because the client holds nothing and
   may decide nothing, losing the server means losing the game — even when
   every person at the table can still reach each other.

3. **The server carries every byte.** Eight players opening the same 4K map
   is eight transfers of identical content that the players could have
   handed each other.

ADR-046 established that the server is authoritative for active-scene state,
and correctly so. But "the server decides what is true" was implemented as
"the client may not retain, continue, or distribute" — three separate
capabilities collapsed into one prohibition. They are separable, and
separating them is what this ADR does.

The prior architecture doc (`docs/Advanced Virtual Tabletop Specification.md`)
proposed solving all of this with local-first CRDTs and peer-to-peer state
replication, deleting the server's authority entirely. That is the road not
taken here, and the distinction is the substance of this decision.

## Decision

**A client may hold content, may continue playing without the server, and
may exchange bytes with peers. The server remains the record of what is
true, and the Game Master remains the arbiter of what happens at the
table.**

Four separable changes:

### 1. Content-addressed local persistence

Clients keep scene state and asset bytes in a durable per-user, per-world
store, encrypted at rest under a session-bound key. Every item carries a
SHA-256 fingerprint of its stored bytes. On opening a world the client
declares what it holds; the server replies with only what differs.

The server is the sole authority on which fingerprints are current. A
client's manifest is a claim of possession, never of entitlement — the plan
is computed *from* what the caller is authorized to see, so an item they may
not have appears in neither the fetch nor the evict list, disclosing nothing.

### 2. Offline authoring, narrowly scoped

A disconnected client may continue editing **token position, rotation and
scale**, queued in a durable outbox and replayed through the existing
mutations on reconnection.

Creation and deletion are refused offline. Precedence cannot resolve a
create/delete conflict without destroying work, and a rule that silently
discards someone's new content is worse than refusing the edit.

Conflicts resolve **GM over player**; between two users of the same role,
whoever reconnects first. Client timestamps are never consulted — they are
forgeable and routinely wrong, and a skewed clock would silently overwrite
other people's work.

### 3. Peer-to-peer distribution, on by default

Clients may fetch bytes from other clients in the same session. Peers are
asked for a **hash**, never for a thing: the requester learns which
fingerprints it is entitled to from its own server-issued plan, requests one
by hash, and verifies before storing. A malicious peer can waste bandwidth
and nothing else.

Peer transfer is always optional at runtime. Every failure path falls back
to the server, and disabling it changes timing only.

### 4. Peer-adjudicated play while server-isolated

A client that cannot reach the server but *can* reach **every** peer,
including the GM, may continue playing. The GM's client adjudicates. All of
it is provisional: on reconnection every change is resubmitted,
re-authorized, and may be rejected.

**Full peer connectivity, not a quorum.** A quorum admits split-brain — two
subsets each satisfying a majority, both making progress, two irreconcilable
histories. Requiring everyone means at most one group is ever playing, so
there is never a second history to merge. It stops play more often; it never
produces state we cannot reconcile.

**The GM specifically, with no election.** If the GM is unreachable, play
stops rather than promoting a replacement. Promotion would mean two
adjudicators across one session and no single chain of authority.

## The trust model, stated plainly

An earlier draft of this design built per-user cryptographic signatures so
the server could verify that a change attributed to player A, arriving over
the GM's connection, was genuinely A's.

**That defended against the wrong party.**

The Game Master is the trusted one. The software's relationship is *with*
them. A GM who acts on a player's behalf, overrides a result, or simply
decides an outcome is exercising authority the role already carries at every
table that has ever been played. Building machinery to prevent that would be
spending real complexity to stop something that is not a wrong — and would
misrepresent what this product is. We are not refereeing the game. We are
running the simulation for the person who is.

So the server's check is: **does the submitter hold the GM role in this
world?** That reuses authorization we already have. It needs no session
keypairs, no signature format, and no new trust root. A non-GM still cannot
submit on anyone else's behalf.

The genuine concern is different: a *player* disconnecting to fabricate an
outcome — a dice roll above all. Dice are already server-authoritative under
ADR-044, so the server can determine what the result should have been.

Our answer is **disclosure, not enforcement.** Where a client reports a value
the server determined differently, the GM's view renders that result
distinctly and lets them inspect both numbers. We do not reject the change,
interrupt play, alter the outcome, or tell the other players. There is no
dispute workflow and no escalation path, deliberately.

A mismatch has many innocent explanations — a stale client, a reconnect
artefact, a bug of ours — and one guilty one. Telling them apart requires
knowing the people involved, which the software never will and the GM
already does. Once the GM can see that two numbers differ, what it means is
a social question at their table, not a technical one for us to answer.
Building a dispute mechanism would be inventing a technical answer to a human
question and getting it wrong precisely in the cases that matter.

**The obligation this creates is accuracy.** A missed discrepancy costs
nothing — the GM runs their table either way. A false one puts an innocent
player under suspicion in front of the only person who can act on it. So
detection reports a discrepancy only on a genuine determined-value mismatch:
never on a timeout, a parse failure, a version mismatch, or any other
ambiguity. When in doubt, report nothing.

## What this is not

This is **not** local-first, and not the CRDT/peer-replication architecture
of the specification document:

- **No CRDTs.** Conflicts resolve by a stated precedence rule adjudicated
  server-side, not by convergent data types.
- **No peer state replication.** Peers exchange content-addressed bytes,
  plus — in the server-isolated state only — signed-off movement proposals
  in a narrow entity scope. They never gossip events, never replicate scene
  state, and never become the record of what is current.
- **No offline-first authoring.** Offline editing is a narrow, temporary
  continuation that reconciles to the server, not a mode of operation.
- **No client authority.** Every offline and peer-adjudicated change is
  provisional until the server confirms it.

Worth recording: striking the AI-GM layer (ADR-051) removed most of the
argument *for* local-first, since the specification's case rested largely on
keeping campaign data on-device for private local inference. What remained
was a transfer-efficiency problem and a session-resilience problem, and both
are solvable without surrendering authority.

## Content moderation determination (Constitution guardrail)

The constitution requires an explicit determination before any feature makes
one world's content accessible outside that world. Peer transfer moves
content between users, so the checkpoint applies.

**Determination: peer transfer does not constitute a centralized public
repository and does not widen content access.** A peer only ever receives
content it is independently permitted to obtain from the server — entitlement
comes from the requester's own server-issued plan, never from asking a peer —
and connections are confined to participants of the same world session and do
not persist beyond it. Nothing becomes reachable that was not already
reachable; only the byte path changes.

This mirrors ADR-049's reasoning for share links. Accepted on record by the
accountable owner.

## Consequences

- Two new workspace crates: `thunderforge-cache-core` (shared policy, no
  I/O, native-testable, compiled into both server and engine) and
  `thunderforge-cache-browser` (wasm32 I/O adapters). The split follows
  ADR-038's reasoning — rules that both sides must agree on are written once
  and tested without a browser.
- `canvas_image_assets` gains a nullable indexed `content_hash`; a new
  `scene_state_fingerprints` table is added. NULL means "must fetch", so the
  feature ships before the backfill completes.
- New GraphQL surface: `worldSyncPlan`, `reconcileQueuedChanges`, and peer
  signaling relayed over the existing subscription.
- Peer transfer is enabled by default. IP exposure between participants is
  disclosed, with an off switch — which also forfeits server-isolated play,
  and the user is told so.
- No telemetry. Cache effectiveness is observable through a local
  diagnostics view and nothing leaves the device.
- The `Applied → Superseded` case is now reachable: a player's offline
  change may be applied on their reconnection and later overridden when the
  GM reconnects. The player must be told. This is specified behaviour of GM
  precedence, not an error path, and needs real UX.
- ADR-046 is amended, not superseded: the server remains authoritative for
  active-scene state and for every reconciled outcome.

## Alternatives Considered

- **Keep the strict read-through cache** (the original spec 028). Satisfies
  ADR-046 untouched and is materially simpler. Rejected by the owner: it
  leaves a session dying on a momentary blip while everyone can still see
  each other, and forgoes peer distribution entirely.
- **Full local-first with CRDTs and P2P replication** — the specification
  document's design. Rejected: it deletes the server's authority, and with
  it the permission model (ADR-050), moderation posture (ADR-043/049), and
  hosted multi-tenant shape the product actually has.
- **Per-user signed proposals** — the earlier draft of this ADR. Rejected on
  the trust model above; it defended against the GM, who is not an adversary
  here, at the cost of a key-distribution scheme and a signature format.
- **Automatically rejecting or sanctioning discrepant outcomes** — rejected.
  False positives in a social game damage real relationships, and the GM
  already holds the authority to decide.
- **Quorum rather than full peer connectivity** — rejected on split-brain.
- **Electing a new adjudicator when the GM drops** — rejected: it breaks the
  single chain of authority for a case that should simply stop.
- **Service Worker or WASM SQLite for the client store** — rejected in
  planning on encryption-at-rest and payload-size grounds respectively; see
  `specs/028-client-world-cache/research.md` R1 and R2.

## Related Decisions

- ADR-046 — amended here.
- ADR-039, ADR-044, ADR-048, ADR-050 — reused unchanged.
- ADR-043, ADR-049 — the moderation posture the determination above follows.
- ADR-051 — removing the AI-GM layer removed most of the case for
  local-first, which is part of why this narrower amendment suffices.
- Constitution Principles III and IV.
