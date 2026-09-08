# Measurements — 043 blob I/O in a dedicated worker

Everything here is produced by `scripts/bench-blob-store.mjs`. Figures are
appended by the tool rather than typed, so a number in this file was measured
rather than remembered.

## The corpus, and where it came from (T012)

Sizes were taken from the distribution of real assets in this repository
(`examples/`, `packs/`, `assets/` — 27 image and map files), not invented:

| Statistic | Size |
|---|---|
| smallest | 900 B |
| median | 640 KB |
| 90th percentile | 4.2 MB |
| largest | 7.8 MB |

The corpus therefore runs **4 KB · 64 KB · 640 KB · 4 MB · 8 MB**: the small
end covers token art and sheet fragments, the median is a typical asset, and
the top two cover scene backgrounds and imported maps. Three of the five sit at
or above SC-002's 1 MB threshold, so the criterion is measured at more than one
point rather than at a single convenient one.

Repetitions vary by size (40 · 40 · 20 · 8 · 8) so that the small sizes, where
per-operation overhead dominates, are not decided by a single sample.

## What is being compared

- **`async-main-thread`** — `createWritable()` on the main thread. What the
  cache does today.
- **`sync-in-worker`** — `createSyncAccessHandle()` inside a dedicated worker,
  default exclusive mode, `flush()` before `close()`. What spec 043 proposes.

Neither path goes through `thunderforge-opfs`. This is deliberate: the number
is meant to authorise building that, so it must not require it. What survives
the crate boundary and the message protocol is a separate figure, recorded by
T028 if the feature proceeds.

## The verdict rule (SC-002)

At 1 MB and above, the synchronous path must write **at least twice as fast**.
The tool computes the ratio and prints the verdict; it is not eyeballed from
the table. If it is not met, the feature stops here and this file is the
deliverable.

---

## Results

### chromium 151.0.7922.34

- **Machine**: Linux 7.1.5-76070105-generic · AMD Ryzen 9 5950X 16-Core Processor · 31 GB
- **Commit**: 91821da · **When**: 2026-09-08T22:34:27.685Z

| Size | Store | Write MB/s | Read MB/s | Reps |
|---|---|---:|---:|---:|
| 4 KB | async-main-thread | 1.0 | 3.6 | 40 |
| 4 KB | sync-in-worker | 3.6 | 3.8 | 40 |
| 64 KB | async-main-thread | 21.6 | 63.5 | 40 |
| 64 KB | sync-in-worker | 50.6 | 116.8 | 40 |
| 640 KB | async-main-thread | 158.2 | 366.6 | 20 |
| 640 KB | sync-in-worker | 188.3 | 311.7 | 20 |
| 4 MB | async-main-thread | 249.8 | 1115.0 | 8 |
| 4 MB | sync-in-worker | 271.4 | 408.7 | 8 |
| 8 MB | async-main-thread | 285.0 | 873.1 | 8 |
| 8 MB | sync-in-worker | 265.0 | 418.0 | 8 |

**SC-002**: write speed-up at 1 MB and above — 4 MB ×1.09, 8 MB ×0.93.
**Not met** (worst case ×0.93 < 2). Per the plan, the feature stops here and this measurement is the deliverable.

_Console errors during the run: 1_

### firefox 153.0

- **Machine**: Linux 7.1.5-76070105-generic · AMD Ryzen 9 5950X 16-Core Processor · 31 GB
- **Commit**: 91821da · **When**: 2026-09-08T22:34:57.573Z

| Size | Store | Write MB/s | Read MB/s | Reps |
|---|---|---:|---:|---:|
| 4 KB | async-main-thread | 0.2 | 7.4 | 40 |
| 4 KB | sync-in-worker | 0.2 | 0.7 | 40 |
| 64 KB | async-main-thread | 5.1 | 92.6 | 40 |
| 64 KB | sync-in-worker | 6.4 | 13.5 | 40 |
| 640 KB | async-main-thread | 58.7 | 403.2 | 20 |
| 640 KB | sync-in-worker | 60.1 | 173.6 | 20 |
| 4 MB | async-main-thread | 248.1 | 533.3 | 8 |
| 4 MB | sync-in-worker | 280.7 | 533.3 | 8 |
| 8 MB | async-main-thread | 376.5 | 528.9 | 8 |
| 8 MB | sync-in-worker | 481.2 | 810.1 | 8 |

**SC-002**: write speed-up at 1 MB and above — 4 MB ×1.13, 8 MB ×1.28.
**Not met** (worst case ×1.13 < 2). Per the plan, the feature stops here and this measurement is the deliverable.

---

## Decision: 043 stops here

**SC-002 is not met, on either browser.** The threshold was ×2 write
throughput at 1 MB and above. Measured:

| Browser | 4 MB | 8 MB |
|---|---:|---:|
| Chromium 151 | ×1.09 | ×0.93 |
| Firefox 153 | ×1.13 | ×1.28 |

Nothing here is close to ×2, and on Chromium at 8 MB the synchronous path was
*slower* than the one already shipped. Per plan.md and the Phase 3 checkpoint,
the feature stops and this file is the deliverable.

### What the numbers actually say

Three things worth keeping, none of which were known this morning.

1. **The premise was backwards.** The claim was "markedly faster **for large
   blobs**". The gain, where there is one, is at the **small** end — the first
   Chromium run measured ×4.2 at 4 KB — because that is where per-operation
   overhead dominates and the synchronous call has less of it. At the sizes
   this cache actually stores the most of, the two paths converge, and the
   large end is where the advantage disappears entirely.

2. **Reads are worse, sometimes much worse.** `getFile().arrayBuffer()` is
   consistently faster than a synchronous `read()` into a buffer, by roughly
   ×5 at 4 MB on Chromium. That is not a surprise in hindsight — one hands back
   a lazily-materialised blob, the other copies bytes into an array this code
   allocated — but it means a wholesale switch would have made the read path,
   which the cache does far more often than writing, slower.

3. **Firefox runs both paths.** Research R1 predicted the current store might
   not work there at all, on the grounds that `createWritable` reached Baseline
   two and a half years after `createSyncAccessHandle`. Firefox 153 runs it
   fine. **R1's portability argument does not survive contact with a real
   browser**, and the stretch goal loses the reason it was promoted.

### Threats to this measurement's validity

Stated because a result that ends a feature deserves more scrutiny than one
that starts it, not less.

- **The comparison may be unfair to the synchronous path.** `flush()` is called
  before `close()` on every synchronous write, which forces persistence.
  `createWritable().close()` is not documented to guarantee the same, so the
  asynchronous path may be being credited for writes that had not reached disk
  when the clock stopped. A fairer comparison would need a durability barrier
  on both sides, and there is no portable way to impose one. **If the async
  figures are inflated by buffering, the real gap is smaller than measured, not
  larger** — which strengthens rather than weakens the decision.
- **One machine, one filesystem, warm cache.** A desktop with 31 GB of RAM and
  an NVMe disk is the friendliest case for the path that buffers. A phone might
  answer differently, and this says nothing about one.
- **The harness is not the product.** It measures raw OPFS calls with no
  encryption, no index and no message protocol. The real store would carry all
  three, and every one of them costs the worker path more than the main-thread
  path, because they would cross a boundary.
- **A console error was logged during the Chromium runs.** It comes from the
  app page the harness was loaded onto, not from the harness, and the harness
  reports its own failures through the returned rows rather than the console.

### What is kept

- `scripts/bench-blob-store.mjs` and `apps/web/public/bench/` — they measure the
  store this product actually ships, which nobody had done, and they will
  answer this question again cheaply if the platform changes.
- This file.

### What would reopen it

Not a better implementation — the ceiling measured here is the platform's, not
the code's. It would take a change in the platform or in what the cache stores:
blobs an order of magnitude smaller and far more numerous, where the small-blob
advantage lives, or a browser where `createWritable` genuinely is absent. The
harness is the cheap way to check either.
