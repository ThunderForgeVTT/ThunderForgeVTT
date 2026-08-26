# Contract: Peer-Assisted Content Distribution

**Feature**: 028-client-world-cache

Clients may fetch content-addressed bytes from each other over WebRTC data
channels instead of from the server. Signaling rides the **existing**
`graphql-ws` connection (ADR-048).

**The one idea this protocol rests on**: peers are asked for a *hash*, never
for a *thing*. A peer cannot substitute different content, because the
requester verifies the hash it asked for before storing. A malicious peer can
waste bandwidth and nothing else.

---

## Signaling

Extends the existing subscription; no new service, no new auth surface.

```graphql
input PeerSignalInput {
  worldId: UUID!
  toSessionId: String!
  """Opaque SDP offer/answer or ICE candidate. Server never interprets it."""
  payload: String!
}

extend type Mutation {
  sendPeerSignal(input: PeerSignalInput!): Boolean!
}

type PeerSignal {
  fromSessionId: String!
  payload: String!
}

extend type Subscription {
  peerSignals(worldId: UUID!): PeerSignal!
}
```

**Server obligations**

- Relay only between sessions that are **both** current members of the named
  world (FR-050). Membership is checked per signal, not once at connect.
- Never interpret or store `payload`. The server is a post box.
- Drop signals to sessions that have ended. No queuing.

**What the server does not do**: it does not vouch for peers, does not
promise reachability, and does not participate in transfer.

---

## Transfer

Once a data channel is open:

```
REQUEST  { fingerprint: "<64 hex>" }
OFFER    { fingerprint, byte_size }
CHUNK    { fingerprint, seq, bytes }
DONE     { fingerprint }
DECLINE  { fingerprint, reason: NOT_HELD | NOT_PERMITTED | BUSY }
```

**Requester obligations**

1. Only request a fingerprint present in its own current `SyncPlan.fetch`.
   This is what enforces FR-047 — entitlement comes from the server's plan,
   never from asking a peer.
2. Verify received bytes with `fingerprint::verify` **before** storing or
   rendering (FR-046). Failure ⇒ discard, do not retry that peer, fall back
   to the server.
3. Abandon a slow or stalled peer and fall back. Peer transfer must never be
   slower than not having used it (FR-048).
4. Never treat a peer's `DECLINE` as information about whether content
   exists.

**Serving obligations**

1. Serve only fingerprints actually held and verified locally.
2. `DECLINE` rather than fabricate. Never send bytes that do not hash to the
   requested fingerprint — pointless (the requester will reject them) and
   indistinguishable from an attack.
3. Stop serving immediately on losing world membership.
4. Rate-limit. A peer is a participant in a game, not a CDN.

**Neither side makes an authorization decision.** The requester's plan came
from the server; the server already decided. This is the property that keeps
FR-047 enforceable without trusting either endpoint.

---

## Privacy

WebRTC reveals IP addresses between connected peers. Per FR-049:

- Users MUST be told peer transfer is active — a visible indicator, not a
  buried setting.
- Users MUST be able to disable it, falling back to server-only, with
  **identical outcomes** and only different timing (SC-013).
- The setting persists per user.

**Decided: peer transfer is ON by default** (FR-049). Peer-to-peer with
server adjudication is the intended model, not an opt-in extra. The IP
exposure is disclosed rather than avoided — a visible indicator, and a
setting that turns it off.

Disabling it also forfeits server-isolated play (FR-057), since that depends
on the same peer connections. The user must be told that when they turn it
off, or they will lose a capability without knowing they traded it away.

No telemetry accompanies any of this (FR-052, FR-054).

---

## Failure modes and required responses

| Failure | Required response |
|---|---|
| Peer sends mismatched bytes | Discard, do not retry that peer, fall back to server, count in diagnostics |
| Peer sends unrequested content | Ignore entirely |
| Peer disconnects mid-transfer | Fall back to server for the remainder; no partial store |
| No peers available | Server fetch, no user-visible difference |
| Signaling unavailable | Peer transfer disabled for the session; everything else works |
| Peer loses permission mid-transfer | Serving peer stops; requester falls back |
| Peer floods requests | Rate-limit and, on persistence, drop the channel |

Every row ends at "fall back to the server." There is no failure mode in
which peer transfer produces a worse outcome than not having it — only a
slower one.

---

## Peer adjudication (server-isolated state only)

Distinct from distribution above, and active **only** while a client is
server-isolated: server unreachable, every peer reachable, GM among them.

```
PROPOSE   { origin_user, entity_id, transform, nonce }
ADJUDICATE{ nonce, verdict: ACCEPT | REJECT }
APPLY     { nonce }
```

Submission to the server on reconnection is made by the **GM's client**, over
the GM's own authenticated session. No per-message signatures: the server
verifies the submitter's role, not a cryptographic attestation.

**Rules**

- Only token position, rotation and scale (FR-060). Never creation,
  deletion, or permission changes.
- Only the GM's client issues `ADJUDICATE` (FR-059). No peer is promoted if
  the GM leaves — adjudicated play stops instead.
- Every participant must be reachable. Losing any peer ends adjudicated play
  at once (FR-058). Two disjoint halves must both stop; neither wins.
- On reconnection the server verifies **only** that the submitter holds the
  GM role in that world (FR-061). A non-GM may never submit a change
  attributed to someone else (FR-061a). The GM is a trusted party by design,
  and the system does not attempt to detect or prevent a GM acting on a
  player's behalf (FR-061b).
- Where the server can independently determine an outcome the client
  reported — dice above all, already server-authoritative under ADR-044 —
  it compares them and, on mismatch, flags it to the GM with both values.
  It does not reject, interrupt, or sanction (FR-064 to FR-068).
- Ordering among peers is by `nonce` sequence agreed at session start, never
  by wall-clock — the same reasoning that keeps timestamps out of conflict
  resolution.
- Adjudication is **provisional**. On reconnection every adjudicated change
  is re-authorized and may be rejected; the server's decision is final
  (FR-062).

**Failure responses**

| Failure | Required response |
|---|---|
| GM unreachable | Adjudicated play does not start / stops immediately |
| Any peer unreachable | Same |
| Submitter is not the GM | Reject an attributed submission (FR-061a) |
| Reported outcome differs from the server's | Apply, record, and flag to the GM — never auto-reject (FR-066) |
| Server returns mid-flight | Reconcile once; never apply twice, never drop |
| Server rejects on reconnect | Revert locally, inform the originating user |

## Explicitly not in this protocol

- **State replication.** Peers exchange bytes addressed by hash, plus — in
  the server-isolated state only — signed movement proposals within the
  narrow scope above. They do not replicate scene state, do not gossip
  events, and never become the record of what is current. The server
  confirms or rejects everything on reconnection (FR-062).
- **Discovery beyond the session.** No DHT, no trackers, no cross-world or
  cross-deployment peer finding.
- **Persistence beyond the session** (FR-050).
