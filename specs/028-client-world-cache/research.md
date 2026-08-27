# Phase 0 Research: Client-Side World Cache

**Feature**: 028-client-world-cache | **Date**: 2026-08-26

Resolves every NEEDS CLARIFICATION carried out of `/speckit-clarify`, plus
the design questions the spec deliberately deferred to planning.

---

## R1: Which side of the WASM boundary owns the store

**Decision**: Neither exclusively. Policy lives in a new shared Rust crate
(`thunderforge-cache-core`) compiled into both the server and the engine.
Browser I/O lives in `thunderforge-cache-browser` (wasm32-only). The engine
owns the asset read path; TypeScript owns only the diagnostics UI.

**Rationale**: The decisive fact is in `src/server/src/canvas_assets_serve.rs`
— asset bytes are fetched by **Bevy's `AssetServer`** via
`GET /canvas-assets/{id}.webp`, issued from inside WASM, not by TypeScript.
Any cache that intercepts at the TS layer would miss the single largest
category of bytes this feature exists to avoid re-transferring.

Beyond that, the rules are needed on both sides and both sides are Rust. The
server computes fingerprints and the fetch/evict plan; the client verifies
fingerprints and decides evictions. Same rules. Written twice they drift,
and drift here means a client that believes it is current when it is not.

**Resolved 2026-08-26 — sync is engine-driven.** The manifest lives in the
Rust browser crate alongside the index that produces it, so the request is
built and the plan applied on that side of the WASM boundary. TypeScript
triggers a sync and observes the result; it never decides anything.

The alternative — orchestrating from TS with `manifest()`/`apply_plan()`
exposed through `wasm_bindgen`, matching how `apply_world_command` already
works — was rejected because TS would then hold, however briefly, a second
account of what is cached. Constitution Principle I exists to prevent exactly
that, and cache policy having one owner is the whole reason `cache-core` is
shared rather than reimplemented.

**Alternatives considered**:

- **Service Worker intercepting `/canvas-assets/*`** — attractive because it
  catches every fetch regardless of origin, TS or WASM. Rejected: the Cache
  API stores plaintext, which fights FR-016's encryption-at-rest; the key
  would have to be shuttled to the worker by `postMessage` and re-obtained
  on every worker restart; and SW lifecycle bugs are notoriously hard to
  reproduce in Playwright. Worth revisiting if OPFS proves slow.
- **Custom Bevy `AssetReader`** — idiomatic for Bevy and keeps everything in
  the engine. Kept as the *mechanism* inside `cached_assets.rs`, but not as
  the owner of the store: the offline queue and the delta protocol are not
  asset concerns and should not live behind an asset abstraction.
- **All TypeScript** — rejected on the AssetServer finding above and on
  Principle I: cache policy in React is the second-source-of-truth mistake.

---

## R2: Local storage primitives

**Decision**: OPFS for encrypted byte blobs; IndexedDB for the index and the
offline outbox. **No WASM SQLite in v1.**

**Rationale**: The index is a flat map from item identity to
`(fingerprint, size, last_read)`. That is a key-value workload, not a
relational one — there are no joins, no ad-hoc queries, no schema evolution
pressure. IndexedDB serves it natively at zero payload cost.

WASM SQLite would add roughly 1MB of additional WASM to a bundle whose size
is already the subject of User Story 6. Spending download budget to solve a
download problem needs a stronger justification than "it might be tidier."

OPFS is the right home for bytes: synchronous access handles in a worker,
no per-entry size ceiling of consequence, and quota governed by the same
`navigator.storage.estimate()` the budget is derived from.

**Alternatives considered**:

- **WASM SQLite for the index** — revisit only if the index grows relational
  needs (e.g. querying "all assets shared across worlds" for dedup). Noted
  as a follow-up, not a v1 need.
- **IndexedDB for bytes too** — simpler (one store) but historically poor
  for large binary values, and gives up OPFS's streaming writes, which
  matter for multi-hundred-MB maps.
- **Cache API for bytes** — plaintext only; fails FR-016.

---

## R3: Fingerprint definition and when it is computed

**Decision**: SHA-256 over the **stored** bytes — post-transcode, not the
uploaded original — computed on write and persisted in a new column on
`canvas_image_assets`. Scene state is fingerprinted over a canonical
serialization produced by `thunderforge-cache-core`.

**Rationale**: `transcode_to_webp` means what the client receives is never
what was uploaded, so hashing the original would produce a fingerprint that
never matches what a client can verify. Hashing on write costs one pass over
bytes already in memory during upload; hashing on demand would re-read every
object from RustFS on every sync, which is precisely the server load this
feature is meant to reduce.

For scene state, "canonical" must mean *stable*: identical logical state must
hash identically regardless of row order, map iteration order, or float
formatting. That is a correctness requirement of the delta protocol and the
reason it belongs in the shared crate rather than being hand-rolled per
call site.

**Migration**: a new nullable `content_hash` column, backfilled lazily —
a NULL hash means "client must fetch," which is correct-by-default and lets
the feature ship before the backfill completes.

**Alternatives considered**:

- **ETag / Last-Modified** — already understood by HTTP caches, but not
  content-addressed: identical bytes under two identifiers get two ETags,
  defeating peer transfer and cross-scene dedup.
- **Hash on read** — no migration needed, but re-reads every object per
  sync.
- **Client-computed hashes** — violates FR-045; a client must never be the
  authority on what is current.

---

## R4: Encryption key derivation and lifetime

**Decision**: A per-session AES-GCM key generated with WebCrypto as
**non-extractable**, stored as a `CryptoKey` object in IndexedDB, deleted on
sign-out. Does not survive sign-out; does survive page reload within a
session.

**Rationale**: Non-extractable is the key property — the raw bytes are never
readable by JavaScript, so XSS cannot exfiltrate the key even though it sits
in IndexedDB. Browsers support structured-cloning `CryptoKey` into IndexedDB
specifically for this pattern.

Surviving reload is required, or every refresh would cold-start the cache
and defeat SC-002. Not surviving sign-out is required by FR-016a.

FR-016c makes the failure mode benign: a missing key is indistinguishable
from a cold cache. That means key loss never needs error handling beyond
"fetch it again," which removes a whole class of edge case.

**Open for implementation**: whether to derive the key from a server-issued
per-session secret instead of generating it client-side. Server-issued would
let the server revoke a cache remotely; client-generated is simpler and has
no round trip. Recommend client-generated for v1, noted as a hardening
option.

**Alternatives considered**:

- **Key in `sessionStorage`** — extractable, readable by any script.
- **Derive from a password** — no password exists for OAuth sessions.
- **No encryption, delete on sign-out** — rejected during clarification.

---

## R5: Delta protocol shape

**Decision**: One GraphQL query, `worldSyncPlan(worldId, held: [HeldItem!])`,
returning items to fetch and items to evict. Asset bytes continue to be
fetched over the existing authenticated `/canvas-assets/{id}` route, which
already enforces `require_world_member`.

**Rationale**: Reusing the existing byte route means the authorization
already written and tested (ADR-039, FR-014) applies unchanged; adding a
second byte path would mean a second place to get authorization wrong. The
plan query is small and cacheable-free; the bytes stay on the route built
for bytes.

The manifest a client sends is `(identifier, fingerprint)` pairs. It is
**not** a claim of entitlement — the server re-authorizes from scratch and
simply omits from the plan anything the caller may not see, which is what
makes FR-014 and FR-047 fall out naturally rather than needing separate
enforcement.

**Alternatives considered**:

- **HTTP conditional requests (`If-None-Match`)** — per-item round trips;
  a world with 200 assets means 200 requests to discover that nothing
  changed.
- **Server-push over the existing subscription** — good for in-session
  changes, which it already handles; wrong for the open-a-world case, which
  needs a synchronous answer before rendering.

---

## R6: Peer transfer and signaling

**Decision**: WebRTC data channels, signaled over the **existing
`graphql-ws` connection** (ADR-048) rather than a new signaling service.
Peers exchange only content-addressed blobs, requested by fingerprint.

**Rationale**: The live-sync socket already exists, is already
authenticated by cookie, and already knows who is in a world session — which
is exactly the membership list FR-050 needs to confine peer connections to.
Standing up separate signaling would duplicate all of that.

Requesting **by fingerprint rather than by identifier** is what makes the
protocol safe: a peer answering "give me blob `sha256:abc…`" cannot
substitute different content, because the requester verifies the hash it
asked for before storing (FR-046). A malicious peer can waste bandwidth; it
cannot poison the cache.

FR-047 (a peer must not receive what it could not obtain itself) is enforced
by the server, not by the serving peer: the requester learns which
fingerprints it is entitled to from its own `worldSyncPlan`, and a peer only
answers requests for fingerprints present in the requester's plan. A peer is
never asked to make an authorization decision.

**Default**: peer transfer is **on**. The owner chose peer-to-peer with
server adjudication as the intended model rather than an opt-in extra.

**Alternatives considered**:

- **New dedicated signaling server** — more moving parts, another auth
  surface, no benefit.
- **Opt-in by default** — recommended initially on privacy grounds, rejected
  by the owner in favour of the benefit reaching everyone. IP exposure is
  handled by disclosure and an off switch instead.

---

## R6a: Peer adjudication while server-isolated

**Decision**: A third connectivity state. When the server is unreachable but
**every** peer is reachable and the GM is among them, play continues for
token movement, with the GM's client adjudicating and both origin and
adjudicator signing each change. All of it is provisional until the server
confirms on reconnection.

**Rationale**: A momentary server blip should not end a session in which
every participant can still see each other. This is the owner's call and it
is a real departure — peers move from distributing bytes to adjudicating
state, which the original FR-034 forbade.

**Why full connectivity rather than a quorum**: a quorum admits split-brain.
Two subsets could each satisfy a majority and both make progress, producing
two irreconcilable histories. Requiring *all* peers means at most one group
can ever be playing, so there is never a second history to merge. It is a
strictly stronger condition than a quorum and it is what keeps reconciliation
tractable — the cost is that adjudicated play stops more often, which is the
correct trade for a session that must stay consistent.

**Why the GM specifically**: it matches table authority, matches the
GM-over-player conflict rule already chosen, and avoids leader election
entirely. If the GM is gone, play stops rather than promoting someone —
promotion would mean two different adjudicators across a session, and the
signatures would no longer form a single chain of authority.

**The trust model, corrected**: an earlier draft treated "a change
attributed to user A arriving over user B's connection" as an impersonation
primitive requiring per-user signatures. That framing was wrong for this
product. The Game Master is the trusted party — the software's relationship
is *with* them — and a GM who acts on a player's behalf or simply decides an
outcome is exercising authority the role already has at any table. Defending
against that would be building machinery to prevent something that is not a
wrong.

So the server's check is simply: **does the submitter hold the GM role in
this world?** That reuses the authorization the codebase already has
(`require_world_member` plus the role model of ADR-050) and needs **no
session keypairs, no signature scheme, and no new trust root**. A non-GM
still cannot submit on anyone else's behalf (FR-061a).

**Where the real concern lies**: a *player* disconnecting in order to
fabricate an outcome — a dice result above all. The answer is disclosure,
not prevention (FR-064 to FR-068). Dice are already server-authoritative
under ADR-044, so the server can independently determine what the result
should have been. If a client reports 20 where the server determined 12, the
system records it and tells the GM both numbers.

It deliberately does **not** reject the change, interrupt play, or accuse
anyone. A mismatch has many innocent explanations — a stale client, a sync
artefact, a bug of ours — and one guilty one, and the person best placed to
distinguish them is the GM, who knows the table. Automated enforcement in a
social game produces false accusations against real people, which is a worse
outcome than the cheating it would prevent.

**This is a significant simplification** over the signing scheme: it removes
a key-distribution mechanism, a signature format, and their failure modes,
and replaces them with a role check plus a comparison the dice system can
already make.

**Alternatives considered**:

- **Treat any server loss as fully offline** — the prior design; simpler,
  and rejected by the owner as too fragile for a brief blip.
- **Quorum instead of full connectivity** — split-brain, above.
- **Elect a new adjudicator when the GM drops** — breaks the single chain of
  authority and adds leader election for a case that should simply stop.
- **Per-user signed proposals** — the earlier draft, described above.
  Rejected once the trust model was corrected: it defended against the GM,
  who is not an adversary here.
- **Automatically rejecting or sanctioning discrepant outcomes** — rejected.
  False positives in a social game damage real relationships; the GM is the
  right decider and already holds that authority.

---

## R7: Offline queue and conflict adjudication

**Decision**: A durable outbox in IndexedDB holding emitted world-store
commands verbatim, replayed on reconnect through the **existing mutations**.
Conflict adjudication is server-side, in `thunderforge-cache-core::conflict`,
consumed by the server.

**Rationale**: Storing emitted commands rather than a diff of local state
keeps the queue an outbox rather than a second simulator — the Principle I
mitigation named in the plan's Constitution Check. Replaying through
existing mutations means offline changes traverse exactly the same
authorization path as online ones, which is what makes FR-042
(re-authorization at reconnect time) automatic rather than a separate
mechanism to get right.

Putting the precedence rule in the shared crate matters because the client
must be able to *predict* the outcome to show the user what will happen,
while the server must *decide* it. Same rule, two consumers — the exact case
the shared crate exists for.

**GM-over-player asymmetry worth noting**: FR-040 means a player's change
can be applied on reconnect and then superseded when a GM reconnects later
with a conflicting offline edit. The player must be told their change was
overridden after the fact (FR-041). This is the sharpest edge in the
feature and needs explicit UX, not just a log line.

**Decided**: offline editing is limited to token position, rotation and
scale. Creation and deletion are refused offline, because precedence cannot
resolve a create/delete conflict without destroying work. Fuller offline
authoring is explicitly deferred well past MVP.

---

## R8: Space budget

**Decision**: 50% of `navigator.storage.estimate().quota`, capped at 20GB,
recomputed on each world open. LRU eviction at world granularity first, then
per-item within a world.

**Rationale**: Proportional adapts across a two-order-of-magnitude range of
machines without shipping a number wrong everywhere. 50% leaves headroom so
this feature does not starve the application's other storage (FR-022b). The
20GB ceiling exists because beyond it the marginal benefit is small and the
eviction bookkeeping is not free.

Evicting whole worlds before individual items keeps a world either usable or
absent, rather than leaving many worlds each missing a scattering of assets
— a half-cached world is slow *and* takes space, the worst of both.

**Alternatives considered**:

- **Fixed 5GB** — wrong on a 128GB Chromebook and on a 4TB workstation.
- **Unlimited until browser eviction** — the undefined-behaviour-under-
  pressure case the spec was written to avoid.

---

## R9: Engine load progress (User Story 6)

**Decision**: Fetch the WASM bundle with an explicit streaming read so
`Content-Length` and received-bytes are both available, feeding a real
progress bar; then a distinct "starting" phase for instantiation, which
reports indeterminate progress.

**Rationale**: FR-030 forbids a fabricated percentage and FR-031 requires
download and startup to be distinguishable. A plain
`WebAssembly.instantiateStreaming` gives no progress events, which is why
the fetch has to be driven explicitly to observe it.

Compression matters to honesty here: with `Content-Encoding: gzip` the
`Content-Length` describes compressed bytes while progress is measured in
compressed bytes too, so the ratio stays correct — but if the header is
absent (chunked transfer), no total is knowable and FR-030's indeterminate
path must engage rather than a guess.

**Note**: the ~190MB figure motivating this story is an unoptimised dev
build. A release build with `wasm-opt` is expected to be dramatically
smaller. FR-035 keeps that work out of scope; the loader must be correct at
any size.

---

## Resolved unknowns summary

| Question | Resolution |
|---|---|
| Cache ownership across the WASM boundary | Shared Rust policy crate; engine owns asset reads (R1) |
| Local storage primitive | OPFS + IndexedDB; no WASM SQLite in v1 (R2) |
| Fingerprint definition and timing | SHA-256 of stored bytes, on write, new column (R3) |
| Encryption key derivation and lifetime | Non-extractable AES-GCM in IndexedDB, dropped on sign-out (R4) |
| Delta protocol | `worldSyncPlan` query; bytes over existing authenticated route (R5) |
| Peer signaling | Existing `graphql-ws` connection; request by fingerprint (R6) |
| Offline queue and conflicts | Durable outbox of commands; server-side adjudication (R7) |
| Budget proportion and ceiling | 50% of quota, 20GB cap (R8) |
| Honest load progress | Explicit streaming fetch, separate startup phase (R9) |

**Still open, deliberately deferred to `/speckit-tasks` or implementation**:
which entity types may be edited offline (R7 recommends a narrow start);
whether the encryption key should be server-issued (R4).
