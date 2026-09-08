/*
 * Spec 043 US3 — the synchronous half, which is why this file is a worker.
 *
 * `createSyncAccessHandle` is available only in a dedicated worker. Everything
 * here is deliberately synchronous once the handle is open: that is the thing
 * being measured, and wrapping it in promises would measure the wrapping.
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

self.onmessage = async (event) => {
  const { size, repeats } = event.data;

  try {
    const root = await navigator.storage.getDirectory();
    const dir = await root.getDirectoryHandle("spec-043-bench", {
      create: true,
    });

    // Probed before timing anything. A method that exists and throws on use is
    // the case a feature-detect misses, and it is the case this whole feature
    // turns on.
    const probeHandle = await dir.getFileHandle("probe", { create: true });
    if (typeof probeHandle.createSyncAccessHandle !== "function") {
      throw new Error("createSyncAccessHandle is not a function here");
    }
    const probe = await probeHandle.createSyncAccessHandle();
    probe.close();
    await dir.removeEntry("probe").catch(() => {});

    const bytes = payload(size.bytes);

    const writeStart = performance.now();
    for (let i = 0; i < repeats; i += 1) {
      const file = await dir.getFileHandle(`sync-${i}`, { create: true });
      // Default mode: exclusive `readwrite`, and the `mode` option is not
      // passed at all — research R2. Passing it would tie this measurement to
      // one engine, which is the opposite of what the Firefox run is for.
      const handle = await file.createSyncAccessHandle();
      try {
        handle.truncate(0);
        handle.write(bytes, { at: 0 });
        // Not optional. A handle closed without flushing may leave a short
        // file, which readers correctly treat as absent — so skipping it would
        // measure writing nothing, very quickly.
        handle.flush();
      } finally {
        // Always. An exclusive handle leaked here locks that file for every
        // tab until the worker dies.
        handle.close();
      }
    }
    const writeMs = performance.now() - writeStart;

    const readStart = performance.now();
    let read = 0;
    for (let i = 0; i < repeats; i += 1) {
      const file = await dir.getFileHandle(`sync-${i}`);
      const handle = await file.createSyncAccessHandle();
      try {
        const buf = new Uint8Array(handle.getSize());
        handle.read(buf, { at: 0 });
        read += buf.byteLength;
      } finally {
        handle.close();
      }
    }
    const readMs = performance.now() - readStart;

    for (let i = 0; i < repeats; i += 1) {
      await dir.removeEntry(`sync-${i}`).catch(() => {});
    }

    self.postMessage({
      store: "sync-in-worker",
      size: size.label,
      bytes: size.bytes,
      repeats,
      writeBytesPerSecond: (size.bytes * repeats) / (writeMs / 1000),
      readBytesPerSecond: read / (readMs / 1000),
    });
  } catch (error) {
    self.postMessage({ error: String(error && error.message ? error.message : error) });
  }
};
