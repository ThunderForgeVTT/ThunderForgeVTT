# Quickstart: Validating the Client-Side World Cache

**Feature**: 028-client-world-cache | **Date**: 2026-08-26

How to prove this feature works end to end. Every scenario maps to a success
criterion in [spec.md](./spec.md); if a scenario cannot be run, that
criterion is unverified and the feature is not done (Constitution Principle
V).

---

## Prerequisites

```bash
docker compose up -d postgres rustfs     # backing services
pnpm run dev                             # server + vite + engine
```

A world with at least one scene, one map background, and several tokens with
art. Two user accounts — a GM and a player — for the permission and conflict
scenarios.

---

## Layer 1 — Shared policy (no browser)

The point of `thunderforge-cache-core` is that its rules are testable
without a browser or a database. This should be the fastest and largest part
of the suite.

```bash
cargo test -p thunderforge-cache-core
```

**Expected**: fingerprint stability across row orderings and float
round-trips; `compute_plan` omitting matched items and fetching
`None`-fingerprinted ones; `plan_eviction` never selecting the open world;
`resolve` total across every role/order pair; `apply_outcomes` surfacing
unmatched changes.

If any of this needs a browser to test, the crate boundary has been eroded —
see [cache-core-api.md](./contracts/cache-core-api.md).

---

## Layer 2 — Server fingerprints and plan

```bash
cargo test -p thunderforge --test cache_sync
```

**Expected**:
- Uploading an image populates `content_hash` with the SHA-256 of the
  **stored WebP**, not the uploaded original.
- `worldSyncPlan` with a matching manifest returns empty `fetch`.
- A `NULL` `content_hash` lands the item in `fetch`, never omitted.
- A non-member's query fails identically to any other non-member access,
  revealing nothing about whether the world exists.
- An item the caller may not see appears in **neither** list.

---

## Layer 3 — End-to-end (Playwright)

```bash
cd apps/web && ./node_modules/.bin/playwright test e2e/world-cache.spec.ts
```

### SC-001 / SC-002 — repeat visit is cheap and fast

1. Open a world with an empty cache; record bytes transferred and
   time-to-interactive.
2. Reload; record both again.

**Expected**: second visit transfers ≤5% of the first's bytes and reaches
interactive ≥3× faster.

> Measure world-content bytes only. Engine bundle download must be excluded
> or the numbers are meaningless — see the Assumptions section of the spec.

### SC-003 — single changed asset

1. Cache a world. As GM, replace one map background. Reopen.

**Expected**: bytes transferred within 10% of that one asset's size.

### SC-004 / SC-004a — revocation and sign-out

1. Cache a world as a player. As GM, remove them from the world.
2. As the player, reopen.

**Expected**: access denied, local data discarded, nothing rendered.

3. Sign out with a populated cache. Inspect OPFS **directly**, not through
   the app, before any background cleanup runs.

**Expected**: stored bytes unreadable. This must be asserted against the
store itself — testing through the app only proves the app does not read it.

### SC-005 — repair

1. Cache a world. Corrupt a blob's bytes; delete another blob while leaving
   its index entry; then reopen.

**Expected**: renders correctly, no user-visible error, diagnostics report
repairs.

### SC-006 — budget

1. With a small configured limit, visit more worlds than fit.

**Expected**: stays within budget; LRU worlds released; the open world never
evicted.

### SC-012 / SC-014 — peer safety

1. Two clients in one session; one holds an asset the other needs.
2. Then: a peer returns bytes that do not match the requested fingerprint.
3. Then: a peer holds content the requester lacks permission for.

**Expected**: (1) transfers peer-to-peer; (2) rejected, falls back to server,
nothing stored; (3) never obtained — the requester's plan never contained
it, so it is never even requested.

### SC-013 — peer transfer is optional

Run the whole suite with peer transfer disabled.

**Expected**: every outcome identical; only timing differs.

### SC-015 / SC-016 — offline and conflicts

1. Load a world, sever the connection, move a token, restore.
2. Then: GM and player both edit the same token offline; **player reconnects
   first**, GM second.

**Expected**: (1) change applied, outcome reported; (2) player's change
applied on their reconnect, then superseded when the GM reconnects, **and
the player is told**. Both clients converge.

> Scenario 2 is the sharpest edge in the feature. A test that only checks
> convergence and not the player's notification misses the requirement that
> matters (FR-041).

### SC-009 / SC-010 / SC-011 — engine loading

1. Throttle the network, clear cache, load the app.
2. Then simulate a failed engine download.

**Expected**: loading state within 1s; progress advancing and never
regressing; download and startup visibly distinct; never at maximum before
interactive. Failure yields explanation plus a working retry, never an
indefinite spinner.

### SC-017 / SC-018 — diagnostics, no telemetry

Open the diagnostics panel after an ordinary session; separately, record all
outbound requests for the session.

**Expected**: hit rate, bytes saved, peer-vs-server, repairs — enough to
confirm SC-001..003 without a test harness. **No request carries cache
statistics or usage telemetry.**

---

## Manual smoke checks

Worth doing once by hand; they read as product bugs rather than test
failures:

- Switch between two recently-visited scenes — should feel instant, no
  loading state (SC-008).
- Pull the network cable mid-session — disconnection should be obvious and
  play should continue.
- Sign in as a second user on the same browser profile — none of the first
  user's worlds visible or fast.
- Open the same world in two tabs and edit in both — no corruption, no
  half-written content.

---

## Definition of done

- [ ] `cargo test -p thunderforge-cache-core` green
- [ ] Server fingerprint/plan tests green
- [ ] Every SC scenario above has a runnable, passing test
- [ ] SC-004a asserted against the store directly, not via the app
- [ ] SC-016 asserts the player is *notified* of supersession
- [ ] Cache measurements exclude engine download time
- [ ] **ADR-052 written and accepted** — Principle IV gate, blocking

The last item is not paperwork. This feature amends ADR-046's
server-authoritative model; ADR-048 exists because that obligation was
skipped once already and had to be recorded after the fact.
