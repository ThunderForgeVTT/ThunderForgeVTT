# Contract: `thunderforge-cache-core` Public API

**Feature**: 028-client-world-cache

The shared policy crate. Compiled into **both** the server binary and the
engine's WASM bundle. Contains no I/O, no `web-sys`, no Diesel, no network —
which is precisely what lets it run under plain `cargo test`, the same
property ADR-038 split `thunderforge-canvas-core` out to obtain.

**If a function here needs to touch the filesystem, the network, or a
database, it belongs in one of the adapter crates instead.** That boundary
is the crate's whole value; erode it and the rules become browser-only
testable again.

---

## Module: `fingerprint`

```rust
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn of_bytes(bytes: &[u8]) -> Self;
    pub fn to_hex(&self) -> String;
    pub fn from_hex(s: &str) -> Result<Self, ParseError>;
}

/// Verify received content against what was promised.
/// The ONLY sanctioned way to trust bytes from any source.
pub fn verify(bytes: &[u8], expected: &Fingerprint) -> Result<(), IntegrityError>;
```

**Contract**

- `of_bytes` is SHA-256, over the bytes **as stored** (post-transcode).
- `from_hex` rejects anything not 64 lowercase hex characters. It never
  coerces — a malformed fingerprint is an error, not a miss.
- `verify` is the single choke point for trusting bytes. Peer-supplied
  content, server-supplied content, and content read back off disk all pass
  through it (FR-010, FR-046, FR-018).

---

## Module: `manifest`

```rust
pub struct Manifest { world_id: Uuid, items: BTreeMap<ItemId, Fingerprint> }

impl Manifest {
    pub fn insert(&mut self, id: ItemId, fp: Fingerprint);
    pub fn remove(&mut self, id: &ItemId);
    pub fn to_wire(&self) -> Vec<HeldItem>;   // deterministic order
}
```

**Contract**

- `BTreeMap`, not `HashMap`: two clients in identical states must serialize
  identically, or the wire format becomes nondeterministic and diffing
  becomes untestable.
- A `Manifest` is a claim about possession only. It carries no permission
  meaning, and the server must not read one into it.

---

## Module: `delta`

```rust
/// Server side: what should this client fetch and discard?
pub fn compute_plan(
    held: &[HeldItem],
    authorized_current: &BTreeMap<ItemId, CurrentItem>,
) -> SyncPlan;
```

**Contract**

- `authorized_current` contains **only** what the caller may see. Filtering
  happens before this function, never inside it — so the function cannot
  leak, and its tests do not need an auth fixture.
- `None` fingerprint (un-backfilled) ⇒ the item lands in `fetch`. Never
  treated as unchanged (R3).
- Held items absent from `authorized_current` ⇒ `evict`. This covers both
  deleted and newly-forbidden, indistinguishably and deliberately.
- Matching fingerprints ⇒ omitted from both lists.
- Pure and total: same inputs, same plan, no ambient state.

---

## Module: `budget`

```rust
pub fn limit_bytes(reported_quota: u64) -> u64;

pub fn plan_eviction(
    index: &[IndexEntry],
    limit: u64,
    incoming: u64,
    open_world: Uuid,
) -> BudgetPlan;
```

**Contract**

- `limit_bytes` = `min(quota / 2, 20 GiB)` (R8).
- `plan_eviction` MUST NOT select any entry belonging to `open_world`
  (FR-023), even if that means returning a plan that does not free enough —
  the caller then degrades to fetching without storing (FR-024), which is
  correct.
- Whole worlds are preferred over individual items, least-recently-used
  first.
- Deterministic: ties broken by world id, then item id, so tests are stable.

---

## Module: `queue`

```rust
pub fn enqueue(outbox: &mut Vec<QueuedChange>, change: QueuedChange);
pub fn replay_order(outbox: &[QueuedChange]) -> Vec<&QueuedChange>;
pub fn apply_outcomes(outbox: &mut Vec<QueuedChange>, outcomes: &[ReconcileOutcome])
    -> Vec<UnresolvedChange>;
```

**Contract**

- `replay_order` preserves enqueue order within a world. A user's own
  sequential edits must not reorder against each other.
- `apply_outcomes` returns anything left **unresolved** — a change with no
  outcome. That return value existing is the enforcement of FR-041: silent
  loss becomes a value the caller must handle, not an omission it can
  overlook.
- Never consults `enqueued_at` for anything but diagnostics.

---

## Module: `conflict`

```rust
pub enum Role { GameMaster, Player }
pub enum Winner { A, B }

pub struct Contender { pub role: Role, pub reconnect_seq: ReconnectSeq }

pub fn resolve(a: Contender, b: Contender) -> Winner;
```

**Contract**

- `GameMaster` beats `Player` regardless of reconnect order (FR-040).
- Same role ⇒ lower `ReconnectSeq` wins (FR-040a).
- **Total**: every pair resolves. No ties, no `Option`, no "it depends".
- Never reads a client-supplied timestamp (FR-040a).
- **Corrected against the code 2026-08-28:** it never sees the change at
  all. Passing `&QueuedChange` as this originally specified would have let a
  future edit decide precedence on the *content* of an edit, which is
  precisely the door FR-040a closes — the rule is about who and when, and a
  function that cannot read the change cannot be talked into anything else.
- Consumed by *both* sides: the server to decide, the client to predict what
  it will be told. Divergence between those two is exactly what this crate
  exists to prevent, so the client MUST NOT reimplement the rule locally
  "for responsiveness."

---

## Testing obligations

Because this crate is pure, everything below is an ordinary `cargo test` —
no browser, no database, no fixtures:

- Fingerprint stability: identical logical state hashes identically across
  row orderings and float round-trips.
- Canonical-version change invalidates every scene fingerprint.
- `compute_plan` omits matched items, fetches `None`-fingerprinted items,
  evicts unknown items.

**Corrected against the code 2026-08-28.** `CurrentItem` pairs the optional
fingerprint with a `byte_size`, which the plan needs twice over: to budget a
fetch before making it, and to check a peer's `OFFER` against the server's
own figure so a hostile offer cannot be used as an allocation primitive. The
earlier `Option<Fingerprint>` here described an argument that never shipped.
- `plan_eviction` never touches the open world, even under pressure.
- `admit_speculative(in_use, limit, incoming)` and `speculative_headroom`
  were added for FR-071 after this contract was first written. They take no
  index and no open world, deliberately: a gate that only ever refuses needs
  neither, so there is no eviction list to return and no way for a later
  edit to turn "short by 3MB" into "free 3MB". A `limit` of zero — a refused
  storage estimate — reads as stop, which mirrors rather than contradicts
  "a refused estimate evicts nothing": without a limit there is no way to
  *demonstrate* room, and speculation is permitted only on room already
  demonstrated.
- `resolve` is total and antisymmetric across every role/order combination.
- `apply_outcomes` surfaces every unmatched change.

A property test asserting `verify(bytes, of_bytes(bytes))` always succeeds,
and that any single-bit mutation fails, is cheap and worth having.
