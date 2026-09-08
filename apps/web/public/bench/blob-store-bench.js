/*
 * Spec 043 US3 — the measurement that decides whether the feature is built.
 *
 * Deliberately throwaway, and deliberately not built on `thunderforge-opfs`.
 * The question this answers is "is the synchronous handle materially faster
 * than `createWritable` on this machine, in this browser, at the sizes this
 * cache actually stores" — and answering it through the real store would mean
 * building the real store first, which is the thing the answer is supposed to
 * authorise. So this talks to OPFS directly, both ways, and knows nothing about
 * encryption, indexes or fingerprints.
 *
 * Served from `public/` rather than bundled so it can be opened on its own,
 * without the engine and without the app's module graph in the timing.
 */

/** Sizes taken from the repository's own example content, not invented. */
export const CORPUS = [
  { label: "4 KB", bytes: 4 * 1024 },
  { label: "64 KB", bytes: 64 * 1024 },
  { label: "640 KB", bytes: 640 * 1024 },
  { label: "4 MB", bytes: 4 * 1024 * 1024 },
  { label: "8 MB", bytes: 8 * 1024 * 1024 },
];

/** Repetitions per size. Small blobs need more to say anything. */
function repeats(bytes) {
  if (bytes <= 64 * 1024) return 40;
  if (bytes <= 640 * 1024) return 20;
  return 8;
}

/*
 * Incompressible, and generated once per size.
 *
 * Deterministic rather than `crypto.getRandomValues` so a rerun measures the
 * same bytes, and non-uniform because a buffer of zeroes lets a filesystem or
 * a disk cheat in ways real ciphertext does not.
 */
function payload(bytes) {
  const buf = new Uint8Array(bytes);
  let x = 0x9e3779b9;
  for (let i = 0; i < bytes; i += 1) {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    buf[i] = x & 0xff;
  }
  return buf;
}

async function benchDir() {
  const root = await navigator.storage.getDirectory();
  return root.getDirectoryHandle("spec-043-bench", { create: true });
}

/** The path the cache uses today: `createWritable`, on this thread. */
async function measureAsync(dir, size) {
  const bytes = payload(size.bytes);
  const n = repeats(size.bytes);

  const writeStart = performance.now();
  for (let i = 0; i < n; i += 1) {
    const file = await dir.getFileHandle(`async-${i}`, { create: true });
    const writable = await file.createWritable();
    await writable.write(bytes);
    await writable.close();
  }
  const writeMs = performance.now() - writeStart;

  const readStart = performance.now();
  let read = 0;
  for (let i = 0; i < n; i += 1) {
    const file = await dir.getFileHandle(`async-${i}`);
    const blob = await file.getFile();
    read += (await blob.arrayBuffer()).byteLength;
  }
  const readMs = performance.now() - readStart;

  for (let i = 0; i < n; i += 1) {
    await dir.removeEntry(`async-${i}`).catch(() => {});
  }

  return {
    store: "async-main-thread",
    size: size.label,
    bytes: size.bytes,
    repeats: n,
    writeBytesPerSecond: (size.bytes * n) / (writeMs / 1000),
    readBytesPerSecond: read / (readMs / 1000),
  };
}

/** Ask the worker for the same numbers through a synchronous handle. */
function measureSync(size) {
  return new Promise((resolve, reject) => {
    const worker = new Worker("/bench/blob-store-bench.worker.js", {
      type: "module",
    });
    const done = (fn) => (value) => {
      worker.terminate();
      fn(value);
    };
    worker.onmessage = (event) =>
      event.data.error
        ? done(reject)(new Error(event.data.error))
        : done(resolve)(event.data);
    worker.onerror = (event) =>
      done(reject)(new Error(event.message || "worker failed"));
    worker.postMessage({ size, repeats: repeats(size.bytes) });
  });
}

/**
 * Run everything and return rows.
 *
 * Each store is measured for every size before moving on, rather than running
 * one store to completion first, so that a machine that thermally throttles or
 * a browser that warms up affects both equally.
 */
export async function run() {
  const dir = await benchDir();
  const rows = [];
  for (const size of CORPUS) {
    rows.push(await measureAsync(dir, size));
    try {
      rows.push(await measureSync(size));
    } catch (error) {
      // The interesting failure. Research R1 predicts a browser where one of
      // these two paths does not exist; recording which is the point.
      rows.push({
        store: "sync-in-worker",
        size: size.label,
        bytes: size.bytes,
        unavailable: String(error && error.message ? error.message : error),
      });
    }
  }
  return rows;
}
