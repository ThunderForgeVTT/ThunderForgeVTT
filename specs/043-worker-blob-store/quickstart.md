# Quickstart — Validating blob I/O in a dedicated worker

**Feature**: 043-worker-blob-store

Scenario A is the gate: run it before anything else is built, and if it fails
its threshold the honest outcome is that the feature stops.

## Prerequisites

```bash
make services-up
pnpm dev            # OPFS needs a secure context; 127.0.0.1 counts as one
```

A browser with devtools open on Application → Storage, and a world with at
least one large scene background.

---

## Scenario A — The measurement, before the architecture (US3, SC-002)

1. Run the benchmark against the **existing** store:
   ```bash
   node scripts/bench-blob-store.mjs --store=async
   ```
2. **Expected**: read and write throughput for at least three blob sizes, with
   the browser, machine and corpus named alongside. A figure without those is
   not reproducible and does not count.
3. Repeat on a second browser. **Expected**: it either produces figures, or it
   fails in a way worth writing down — research R1 predicts the current store
   may not work there at all, and *that* result is the more interesting one.
4. Once the worker path exists, run both and compare.
5. **Expected**: writes of 1 MB and above are at least twice as fast through
   the worker. **If they are not, stop.** Record the numbers, keep the
   benchmark, and close the feature — it is a successful outcome, not a failed
   one.

---

## Scenario B — The table keeps moving (US1, SC-001)

1. Clear site data so the cache is cold.
2. Open a scene with a large background and several hundred tokens.
3. Drag the map continuously from the moment it starts loading.
4. **Expected**: no perceptible catching while assets are written. Compare
   against the same run with caching disabled; the gap is what this closes.
5. Reload. **Expected**: every asset served from cache and byte-identical.

---

## Scenario C — Two tabs, one blob (US4, FR-012, SC-005)

1. Open the same world in two tabs.
2. Drive both into loading the same uncached large asset at once.
3. **Expected**: both finish. Neither shows a broken image. The blob is
   complete afterwards.
4. Repeat 100 times, scripted. **Expected**: no truncated read, no corruption.
5. Now kill one tab mid-write (close it while the asset is loading).
6. **Expected**: the other tab treats the blob as absent and refetches. It does
   **not** report an error, and it does **not** delete a file another tab might
   still be writing.

---

## Scenario D — The fallback (US2, SC-003, SC-004)

1. Force the fallback:
   ```bash
   THUNDERFORGE_BLOB_STORE=async pnpm dev
   ```
2. Repeat Scenarios B and C.
3. **Expected**: everything still works. Slower, and nothing else differs.
4. Check every screen a user sees. **Expected**: no error, no notice, no badge.
   The verdict appears only in the diagnostics panel, which should name the
   reason.
5. Now break it harder — block the worker script in devtools and reload.
6. **Expected**: same outcome. The cache degrades, never fails.

---

## Scenario E — The regression guard (FR-020, SC-007)

The failure this feature is likeliest to suffer is silently reverting to the
slow path where the fast one was available.

1. Run the suite normally. **Expected**: green, and the diagnostics report
   `SyncAccessInWorker`.
2. Break it on purpose: make the probe always return the fallback verdict.
3. **Expected**: a test **fails**, naming the silent regression. If everything
   still passes, SC-007 is not met and the guard is decoration.
4. Restore.

---

## Scenario F — Cost (FR-021)

1. Build for release and record compressed artefact sizes before and after.
2. **Expected**: the worker's wasm contains the storage crate and not the
   engine. A second copy of the engine is a stop-and-rethink, not a regression
   to accept.
3. **Expected**: the engine bundle itself is unchanged, and total growth is
   reported as a number rather than described as small.

---

## Making the guards fail on purpose

House habit. Four here:

- **Leak a handle** — return early from a write without `close()`. Every later
  operation on that blob, in every tab, must block. If it does not, the
  exclusive-lock model is not what this design assumes.
- **Skip `flush()`** before close. The blob must read back as *absent*, not as
  short and not as corrupt.
- **Match responses by arrival order** instead of correlation id. The two-tab
  scenario must fail. If it passes, it is not exercising concurrency.
- **Return `Failed` instead of `Absent`** for a lock refused by another tab.
  Scenario C must fail — a routine outcome surfaced as an error is the thing
  FR-014 exists to prevent.
