# Phase 0 — Research: Blob I/O in a Dedicated Worker

**Feature**: 043-worker-blob-store · **Date**: 2026-09-08

Both of the spec's open questions are resolved here, and one finding reverses
the feature's own framing.

---

## R1 — The path we are on is the less portable one

**Finding**: `FileSystemFileHandle.createSyncAccessHandle()` reached
**Baseline "widely available" in March 2023**. `createWritable()` — the call
`thunderforge-opfs`'s current store is built on — reached **Baseline "newly
available" in September 2025**, two and a half years later.

**Decision**: Treat the worker path as the **portable** implementation, not
merely the fast one. Reframe the feature accordingly.

**Rationale**: The spec was written on the assumption that synchronous access
is an optimisation bolted onto a well-supported base, with a fallback for
browsers that lack it. The support data says the opposite: the base is the
narrower API. Safari in particular shipped OPFS with synchronous access handles
in workers well before it shipped `createWritable`, so there is a real window
of browsers where the *fallback* is the path that does not work.

This changes what the fallback is *for*. It is not a browser-support strategy —
it cannot be, because it is the less supported API. It is a safety net for
contexts where a worker cannot be started at all.

**Consequence for the stretch goal**: unlocking Firefox and Safari stops being
a speculative bonus and becomes a plausible outcome of the same work. It is
still a stretch, because the constitution's Chromium-only constraint rests on
more than this one API — WebCrypto and IndexedDB usage across the cache would
each need their own verification — but the single largest portability blocker
in the storage layer is the call this feature replaces.

**Alternatives considered**: Keeping the framing as "an optimisation with a
fallback". Rejected: it would have led to building the fallback as the primary
and testing it hardest, which is backwards given the support data.

---

## R2 — Locking posture (resolves spec Q1)

**Decision**: Use the **default exclusive `readwrite` mode**. Do not pass the
`mode` option at all.

**Rationale**: Two reasons, and the second is decisive.

1. Exclusive is the safe default for FR-021. A second opener is refused
   outright rather than handed a file whose contents are mid-flight, which
   removes the window `BlobShape::Incomplete` exists to describe rather than
   narrowing it.
2. **The `mode` option is not portable.** `createSyncAccessHandle` is Baseline;
   its `mode` argument is a later addition and is not. Passing
   `readwrite-unsafe` would tie the fast path to one engine — which, per R1, is
   precisely the constraint this feature is positioned to relieve. Choosing a
   non-portable option to enable a stretch goal about portability would be
   incoherent.

**What FR-021 becomes** (answering the spec's US4 directly): the guarantee is
unchanged in what it promises and simpler in how it is kept. Today a reader can
open a zero-length file and must be careful not to reclaim it. Under exclusive
mode a reader cannot open the file at all while a writer holds it, so the
refusal — not an empty file — is the observable event. `BlobShape::Incomplete`
must therefore survive, because the *other* way to reach it still exists: a
write interrupted by the tab dying releases the handle with a short file behind
it, and that file is indistinguishable from one being written right now.

**Alternatives considered**: `readwrite-unsafe` with caller-side ordering.
Rejected on portability, and because the cache's cross-tab ordering already
lives in Web Locks; adding a second, weaker ordering mechanism underneath it
would put two answers to one question in the stack.

---

## R3 — What the fallback is for (resolves spec Q2)

**Decision**: Keep the existing asynchronous store, and **narrow its stated
purpose to contexts, not browsers**. It is retained for exactly one case: a
dedicated worker cannot be started or has died. It is not retained as a
compatibility story for browsers lacking synchronous access, because no such
browser has `createWritable` either.

**Rationale**: The codebase has a stated aversion to well-tested, entirely
uninvoked code — `worldCacheStorage.ts` records that this cache has already
shipped that mistake twice. A fallback justified by browsers that do not exist
would be the third. Naming the real reason keeps it honest and keeps it small:
it must be exercised by a test that forces the worker off, and by nothing else.

**Note on secure context**: not a differentiator. OPFS as a whole requires a
secure context, so an instance served over plain HTTP has no cache at all
today; the fast path does not narrow anything that was previously open. This is
worth stating because a self-hosted VTT on a LAN is a real deployment and it
would be reasonable to assume otherwise.

**Alternatives considered**: Removing the asynchronous store entirely once the
worker path lands. Rejected for now: "the worker cannot start" is a real
failure mode (a content policy, an extension, a platform quirk), and a cache
that vanishes under it is a worse product than one that gets slower. Revisit
once the fast path has shipped and the fallback's hit rate is known.

---

## R4 — Bundle cost

**Decision**: The worker loads a **wasm artefact containing the storage crate
only**, not the engine.

**Rationale**: The engine bundle has a concern threshold of 100 MB compressed
and the storage layer is a small fraction of it. Shipping the engine twice to
reach one API would be a large regression against a budget that is already
watched. The `BlobStore` seam makes this feasible: the worker needs paths,
bytes and the OPFS calls, and explicitly not encryption, the index or the sync
protocol.

**Open risk to measure, not assume**: the marginal download of a second wasm
artefact, and whether the worker's start-up cost is paid once per session or
per world. Both are reportable numbers and FR-021 of this spec asks for them.

---

## R5 — Measuring the claim (spec US3, SC-002)

**Decision**: Build the benchmark **first**, before the worker, and run it
against the existing store on both a Chromium and a non-Chromium browser.

**Rationale**: The benchmark is the gate. Building it first means the number
that decides whether to proceed arrives before the architecture is committed
to, and the artefact is independently useful: it measures the store already
shipped, which nobody has done either.

Running it on a second browser at this stage costs almost nothing extra and
directly informs the stretch goal, because it will show whether the *current*
store works there at all — which R1 predicts it may not.

**Corpus**: at least three sizes spanning what this cache actually stores.
Scene backgrounds and map images dominate the large end; token art and sheet
fragments the small. Sizes to be taken from a real world's asset distribution
rather than invented, so the result describes this product rather than a
synthetic curve.

---

## Resolved unknowns

| Spec question | Resolution |
|---|---|
| Q1 — locking posture | Default exclusive `readwrite`; the `mode` option is not portable (R2) |
| Q2 — fallback lifetime | Retained, purpose narrowed to "no worker available", not browser support (R3) |
| Stretch — more browsers | Promoted from speculative to plausible; the replaced call is the layer's largest portability blocker (R1) |
