# The Client World Cache Crates

**Status**: Implemented (spec `028-client-world-cache`, Phase 11)
**Last Updated**: 2026-08-28
**Related ADRs**: [ADR-052](adrs/20260826-052-client-cache-offline-and-peer-adjudication.md)
(the decisions these crates implement), ADR-046 (amended by it), ADR-038
(the native-testability split this reuses), ADR-044 (dice trust boundary),
ADR-048 (`graphql-ws` transport the peer signaling rides), ADR-050
(permission declaration).

---

## What this document is for

`cargo doc` describes the API, and does it better and without going stale.
This is what you want *before* reading the code: what the two crates are for,
why there are two of them, the invariants that hold across both, and the
handful of decisions that look arbitrary until someone explains them.

The decisions themselves are argued in
[ADR-052](adrs/20260826-052-client-cache-offline-and-peer-adjudication.md);
the wire shapes are specified in
[`contracts/cache-core-api.md`](../specs/028-client-world-cache/contracts/cache-core-api.md),
[`contracts/peer-protocol.md`](../specs/028-client-world-cache/contracts/peer-protocol.md)
and
[`contracts/graphql-delta-sync.md`](../specs/028-client-world-cache/contracts/graphql-delta-sync.md).
Neither is restated here.

## The two crates

| Crate | What it holds | Where it runs |
|---|---|---|
| [`crates/thunderforge-cache-core`](../crates/thunderforge-cache-core) | Every rule the server and the client must agree on. No I/O. | Compiled into **both** the server binary (`src/server`) and the engine WASM bundle (`src/engine`). |
| [`crates/thunderforge-cache-browser`](../crates/thunderforge-cache-browser) | The I/O only a browser can perform: OPFS blobs, a WebCrypto key, IndexedDB records, WebRTC data channels. | The engine's WASM bundle only. |

### Why two, and not one

The same rules are needed on both sides of the wire, and both sides are Rust.
The server computes fingerprints, decides what a client is missing, and
adjudicates conflicting offline edits. The client computes fingerprints to
verify what it received, decides what to evict, and predicts what the server
will say. Those are the same rules — and implemented twice they drift. Drift
here is not cosmetic: a client whose notion of "current" disagrees with the
server's believes it is up to date when it is not, which presents as missing
map art and silently lost edits.

So `cache-core` depends on no platform: no `web-sys`, no Diesel, no network,
no clock. That is what lets every rule in it be exercised by plain
`cargo test` rather than only inside a browser — the same reasoning ADR-038
used to split `thunderforge-canvas-core` out. **If something in `cache-core`
needs I/O, it belongs in `cache-browser` or the server crate instead.**

The split is build-enforced rather than a convention someone remembers.
`cache-core`'s manifest carries no wasm dependency at all, and
`cache-browser` confines `wasm-bindgen`, `js-sys` and `web-sys` to
`[target.'cfg(target_arch = "wasm32")'.dependencies]` — on a native build they
are not in the dependency graph, so anything in `cache-browser` that compiles
natively provably does no browser I/O. Every module there is organised the
same way: pure logic first, unit-tested natively, with a
`#[cfg(target_arch = "wasm32")]` block underneath holding the platform calls.

### A third crate the ADR does not name

ADR-052's Consequences records exactly two new crates. There are now three:
the OPFS paths and filesystem calls were subsequently extracted into
[`crates/thunderforge-opfs`](../crates/thunderforge-opfs), because every line
of them was `#[cfg(target_arch = "wasm32")]` and therefore had no native test
at all — `cache-browser`'s many passing tests were about path strings, and the
question FR-021 actually asks ("what does one tab see while another is
writing?") could not be asked. `cache_browser::opfs` re-exports the path
vocabulary and keeps what only it can do: sealing the bytes, opening them
again, and deciding that a *complete* file which will not open, or which opens
to bytes that do not hash to its own filename, is garbage and is deleted.

## Invariants that hold across both crates

**One place to trust bytes.** `cache_core::fingerprint::verify` is the only
sanctioned way to decide that bytes are what they claim to be, and every path
that accepts bytes goes through it: server responses, peer transfers, and
blobs read back off local disk. A local file is not more trustworthy than a
remote one; it is merely closer. Nothing in either crate compares a digest by
hand, which is what makes "we never trusted unverified content" checkable
rather than hopeful.

**A fingerprint is SHA-256 over the bytes as stored** — post-transcode, not as
uploaded. The server transcodes to WebP, so hashing the original would produce
a value no client could ever verify against what it holds.

**No clocks, anywhere a decision depends on order.** Conflict resolution uses a
server-assigned reconnect sequence; LRU eviction uses a monotonic local
counter; peer adjudication uses a Lamport nonce. Client clocks are forgeable
and routinely wrong, and a skewed one would silently overwrite other people's
work — which is exactly the failure these rules exist to prevent.

**No degraded state.** In `cache-browser` there is no state between "have it"
and "don't". A corrupt blob, a decryption failure, and a lost key all produce
the same observable outcome as an empty cache: `Ok(None)`, refetch, no error
surfaced. `CacheError` is reserved for conditions a caller genuinely cannot
proceed through — a missing browser API, a rejected platform call, a record we
wrote that will not parse back.

**The index is a belief, not the truth.** OPFS holds the bytes; the IndexedDB
`index` store holds an account of them, and a crash between the two writes
makes them disagree. That is expected, and is why the fingerprint is the
blob's filename: the disk can be re-read and the disagreement settled without
downloading anything. Where they differ, **OPFS wins**.

## The decisions that look arbitrary

Each of these is verified against the code as it stands, not against the
spec's intent.

### Peers are asked for a hash, never for a thing

A requester learns which fingerprints it is entitled to from its own
server-issued `SyncPlan`, asks a peer for one by hash, and verifies before
storing. A peer cannot substitute different content, so a hostile peer can
waste bandwidth and nothing else. That single property is what lets both
endpoints be untrusted, and it is why nothing in `peer.rs` contains an
authorization decision — the server already made it.

`PeerDownload` is the receive machine, and the only way bytes leave it is
`DownloadStep::Verified`, produced after `fingerprint::verify` returns `Ok`
and nowhere else. A peer vanishing mid-transfer drops the partial buffer
because there is no expressible way to get half of it out. Every failure —
a decline, silence, a stall, a size that does not match the server's figure,
frames out of order — ends at *fetch it from the server*, and there is
deliberately no error type: a caller that has to handle a peer failure is a
caller that can be made worse by one.

### Entitlement is made unexpressible, not checked

`PlanScope::from_plan` is the only constructor, it reads only `plan.fetch`,
and there is no `insert` or `extend`. `PlanScope::request` is the only place
in the program that can mint a `PeerRequest`, and it returns `Option` — `None`
for a fingerprint the server did not list. Asking a peer for anything else is
not a rejected operation; it is not an expressible one. `PeerRequest` is not
`Clone` (one token, one transfer) and is consumed by value when a download
begins, so a plan entry cannot be spent twice. The scope is replaced wholesale
on every sync rather than merged, because a scope outliving its plan is the
same revocation hole `evict` closes on the storage side.

The same shape appears in `TokenTransform`, which carries position, rotation
and scale and has no field for a creation, a deletion, a permission, a name or
a hit point — so a peer-adjudicated proposal outside that scope is
unrepresentable rather than refused. A check can be forgotten at one call
site; a type that cannot hold the wrong thing cannot be.

The server side has the same ordering. `worldSyncPlan` builds a map of what
*this caller may see* and only then hands it to
`cache_core::delta::compute_plan`; nothing is filtered afterwards. Because
`compute_plan` is pure over the map it is given, it structurally cannot offer
content that never entered the map. Revocation then needs no separate channel:
a held item the caller has lost access to is simply missing from the map and
lands in `evict`, byte-identical to a deleted item, so the client cannot tell
"you may not see this" from "this no longer exists".

### Speculative content never evicts

Prefetching warms scenes nobody has opened yet. `budget::admit_speculative`
takes `(in_use, limit, incoming)` and **no index and no open world** — while
`budget::plan_eviction` needs both, because releasing things is what it does.
The narrowest signature that can express the speculation rule is also the one
that cannot break it: there is no eviction list to return, no world to
accidentally not protect, and no way for a later edit to turn "we are short by
3MB" into "so free 3MB". A prefetch that will not fit stops.

The asymmetry around an unknown quota follows from the same rule. When the
browser declines to estimate, `sync::enforce_budget` leaves `limit_bytes` at
zero and evicts nothing — acting on an unknown limit would destroy a working
cache — while `admit_speculative` reads a zero limit as `Stop`, because
speculation is only ever permitted on demonstrated room. Demand loads are
unaffected either way.

`PrefetchQueue` is built only from a plan, is stamped with the world it was
built for and re-checks it every step, yields whenever any demand fetch is in
flight, and stops (rather than skipping to a smaller item) when the visit
allowance of 64MB or the store's headroom is reached.

### Eviction protects the open world even when that fails

`plan_eviction` never selects an entry belonging to the open world, even when
that means returning a plan that does not free enough — it reports
`insufficient: true` instead, and the caller degrades to fetching without
storing. Whole worlds go before individual items, least-recently-used first: a
half-cached world is the worst outcome available, being slow *and* occupying
space. The budget itself is proportional (half the reported quota, capped at
20 GiB) rather than a shipped constant, which would be wrong on a low-storage
laptop and absurd on a workstation.

### The GM is the trusted party, so the server checks a role

This is the most surprising decision in the feature, and ADR-052's "The trust
model, stated plainly" argues it at length. An earlier draft built per-user
signatures so the server could verify that a change attributed to player A,
arriving over the GM's connection, was genuinely A's. **That defended against
the wrong party.** A GM who acts on a player's behalf is exercising authority
the role already carries at every table that has ever been played.

So on reconnection `reconcileQueuedChanges` checks whether the submitter holds
the GM role in this world — the authorization we already have — and there is
no keypair, no signature format and no new trust root anywhere in
`AdjudicationMessage`. A non-GM may never attribute a change to anyone else.
A GM acting on a player's behalf produces no flag and no notification.

The genuine concern is a *player* fabricating an outcome, a dice roll above
all, and the answer there is **disclosure, not enforcement**: dice are
server-authoritative under ADR-044, so where a client reports a value the
server determined differently, the change is applied anyway and the difference
is shown to the GM with both numbers. Nothing is rejected, interrupted,
altered, or told to the other players, and there is no dispute workflow to
reach. The obligation that creates is accuracy — a false discrepancy puts an
innocent player under suspicion in front of the only person who can act on it
— so reporting is arranged as the exception that must be reached rather than
the default that must be escaped: `GraphQLDiscrepancy` can only be built from
a `DeterminedValue`, which is only constructible from a stored
server-authoritative row. Timeouts, missing rows, unparseable outcomes and
unrecognised format versions are therefore not branches someone could get
wrong; they are the absence of that value.

### Full peer connectivity, not a quorum

Peer-adjudicated play runs only while the server is unreachable, **every**
participant is reachable, and the GM is among them. A quorum would let two
subsets each satisfy a majority and both make progress — two irreconcilable
histories and no rule that could merge them without destroying somebody's
work. Requiring everyone means at most one group is ever playing, so there is
never a second history to merge. Both halves of a partition stop; neither
wins. If the GM's client is not there, play stops rather than electing a
replacement, because a replacement is a second adjudicator in one session.

Nothing in `Adjudication` decides whether the server is up. That is the
heartbeat's answer (`apps/web/src/engine/world/sync/heartbeat.ts`), the one
liveness signal this feature has, and it is passed in. A second opinion about
connectivity is how a client ends up queueing edits during an idle moment
while reporting a healthy connection it does not have.

### A change leaves the outbox only once the server has spoken about it

`OutboxStore::forget_resolved` deletes exactly the rows named in the server's
outcomes and nothing else, and returns everything still queued afterwards. A
delete that fails leaves the row queued, which replays it next time —
replaying an applied change is safe (the server gives one outcome per
submitted change), while dropping one is not recoverable. `queue::apply_outcomes` has
the same shape and returns `Vec<UnresolvedChange>`: the
existence of that return value is what makes silent loss of someone's work a
value the caller has to handle rather than an omission it can overlook.

A rejection counts as *resolved* — the server accounted for it and said no —
so it is not replayed forever. Queued commands are stored verbatim and never
interpreted by either crate, which is what keeps the outbox an outbox rather
than a second simulator: on reconnection the server replays each through the
ordinary mutation path, so re-authorization against *current* permissions is
automatic rather than a mechanism of its own to get right.

The outbox is also the one store that deliberately survives sign-out.
Everything else the cache holds is a copy of content the server still has,
worth nothing once it cannot be decrypted; a queued change is the only copy of
work the server has never seen, and its entries are plaintext commands that do
not depend on the session key at all.

### The session key is non-extractable, and losing it is not an error

The AES-GCM key is generated with `extractable: false` and stored as a
`CryptoKey` *object* in IndexedDB, so what persists is a handle to key
material the browser holds, not the material. An XSS payload running with page
privileges can use the key while it is running; what it cannot do is
exfiltrate it, so it cannot read cached bytes copied off the machine, and its
access ends when the page does. Nothing in `crypto.rs` calls `exportKey`, and
there is no API there that returns key bytes.

Sign-out deletes the record, which makes the blobs on disk unreadable
immediately — before the slow reclamation of the bytes has to finish. That is
not enough on its own: a tab still holding the live `CryptoKey` in memory
keeps decrypting happily, so `signal.rs` announces the discard over both a
`BroadcastChannel` (which reaches workers) and a `localStorage` write (which
fires a `storage` event in every other window but not in workers). Neither
carrier alone covers the cases, both carry the same payload, and the handler
is idempotent.

## Where the boundaries sit

- **TypeScript decides nothing.** The manifest is built and the plan applied
  inside the engine, in Rust, alongside the index that produces it. TS may
  trigger a sync and read the summary. Orchestrating from TS was rejected
  because TS would then hold, even briefly, a second account of what is
  cached.
- **Peer transfer is gated in TypeScript**, before `peer::enable` is ever
  called, because what the setting prevents is the *connection* — the IP
  exposure happens when a channel opens, not when bytes move.
- **The engine, not `cache-browser`, owns the world store and the OPFS
  handles.** The blob reader the serving path uses and the callback an
  adjudicated move applies through are both injected, so this crate
  physically cannot read or write anything the engine has not agreed to
  expose.
- **The server is a post box for signaling.** `send_peer_signal_impl` relays
  opaque SDP and ICE payloads and never interprets or stores them. Membership
  is re-checked per signal for *both* ends, because a subscription is
  long-lived and a player removed an hour into a session would otherwise keep
  signaling on a check made when they still belonged. `fromSessionId` is
  treated as a claim and verified against the session registry — without that,
  a member could forge it and impersonate another participant on the very
  channel the recipient is about to trust for SDP.

## Known limits

Stated because a document that only lists strengths is not trusted twice.

- **Conflict marks live in process.** `conflict::resolve` needs to know what
  already landed on an item and who put it there; that memory is an
  in-process map in `src/server/src/graphql/mutations_reconcile.rs`, and **a
  server restart forgets it**. Two players reconnecting either side of a
  restart both apply, and the later one silently wins on last-write. The
  window is the minutes between one client reconnecting and the next, and the
  cost is a token position — recoverable, visible, re-doable. Making it
  durable means a table keyed by (world, item) written in the same
  transaction as the edit; that becomes the right trade if offline authoring
  ever widens past token position/rotation/scale.
- **SC-002 is NOT MET** as measured 2026-08-26: 5748ms cold versus 5743ms
  warm, no improvement. The cause is understood and is not the cache — both
  timings are dominated by instantiating the engine WASM bundle, which this
  feature does not touch. See `specs/028-client-world-cache/spec.md`.
- **SC-024 is NOT VERIFIED as written**, 2026-08-28. Neither half of the
  measurement exists: prefetching has no off switch to compare against, and
  5% is below the noise floor while engine startup dominates. T120 instead
  pins the property the budget protects — the active scene's bytes are
  requested before any speculative byte is.
- **Offline authoring is narrow on purpose.** Token position, rotation and
  scale only. Creation and deletion are refused offline, because precedence
  cannot resolve a create/delete conflict without destroying work.
- **`peerAvailable` is reachability, not holdings.** It says a peer exists,
  never that any peer has the bytes, and a `false` must never suppress a
  server fetch.
- **A peer lost mid-transfer costs the whole item.** The contract's failure
  table reads "fall back to the server for the remainder"; there is no
  resumption in the code, and the partial buffer is discarded, so the item is
  refetched entire. The outcome is correct and no worse than not having used
  a peer, but it is not a partial fetch.
- **No telemetry.** The FR-049 activity indicator reports counters only —
  connected peers, bytes from peers, verification failures — with no peer
  identities, addresses or timings. Distrust of a peer is session-lifetime
  and is never persisted or reported, because a peer behind a broken proxy
  and a malicious peer are indistinguishable from here and the response to
  both is identical.

## Where the contracts and the code differ

Checked while writing this. In each case the code is the truth.

- `contracts/cache-core-api.md` gives `compute_plan`'s second argument as
  `BTreeMap<ItemId, Option<Fingerprint>>`. It is
  `BTreeMap<ItemId, delta::CurrentItem>`, which pairs that optional
  fingerprint with a `byte_size` — the plan has to carry sizes so the client
  can budget and so a peer's `OFFER` can be checked against the server's
  figure.
- The same contract gives `conflict::resolve` as taking
  `(&QueuedChange, Role, ReconnectSeq)` tuples. It takes two
  `conflict::Contender { role, reconnect_seq }` values and never sees the
  change at all, which is why its tests need no fixture.
- That contract's `budget` section predates `admit_speculative` and
  `speculative_headroom`, added for FR-071. Both are in the crate and
  described above.
- `queries/world_sync_plan.rs`'s module docs still say the contract's summary
  table lists "client claims an item it may not see" as *omitted from both
  lists*. That was an earlier draft;
  `contracts/graphql-delta-sync.md` has since been corrected to "item id in
  `evict` — byte-identical to a deleted item", which is what `compute_plan`
  does and what its unit test asserts. Contract and code agree; only that
  comment is stale.

## A separate recent crate: `thunderforge-pg-sockets`

Not part of this feature, and mentioned only so it is not mistaken for one of
the two above. [`crates/thunderforge-pg-sockets`](../crates/thunderforge-pg-sockets)
was extracted from the server's `network/listener.rs` for the same reason
`thunderforge-opfs` was extracted from the cache: the decisions worth testing
were welded to I/O that only exists at runtime. It holds one `broadcast`
channel per world instead of one per process — the fix for a measured 20,000×
delivery amplification at 100k connections — and the rule for how far the
"seen everything up to here" cursor may advance, which is what stops an event
committing out of id order from being lost. The Postgres I/O stays in the
server, where the pool and the migrations live. Its module docs carry the
measurements and the reasoning for keeping `LISTEN/NOTIFY` as a wake rather
than as delivery.
