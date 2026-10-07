/**
 * Spec 080 T017 (SC-001): a 40 MB file over a real socket, one part of which
 * is cut halfway, with the real default settings. The result is the file,
 * and almost nothing is sent twice.
 */
import assert from "node:assert/strict";
import { after, it } from "node:test";
import { DEFAULT_DOWNLOAD_SETTINGS, downloadBytes } from "../src/index.ts";
import { fileServer, generated, sha256 } from "./server.ts";

const SIZE = 40 * 1024 * 1024;
const file = generated(SIZE, 80);
const server = await fileServer(file);
after(() => server.close());

it("resumes a part cut halfway from the byte it stopped at", async () => {
  const { partSize, concurrency } = DEFAULT_DOWNLOAD_SETTINGS;
  const cutAt = 2 * partSize;
  let cut = false;
  server.cut = (request, start) => {
    if (cut || start !== cutAt || !request.headers.range) return null;
    cut = true;
    return partSize / 2;
  };

  const bytes = await downloadBytes(server.url);

  assert.ok(cut, "the part was never cut");
  assert.equal(bytes.length, SIZE);
  assert.equal(sha256(bytes), sha256(file));

  // The resume asks from inside the part, never from its start again.
  const resumed = server.requests.find((r) => {
    const from = Number(r.range?.match(/^bytes=(\d+)-/)?.[1] ?? -1);
    return from > cutAt && from < cutAt + partSize;
  });
  assert.ok(resumed, JSON.stringify(server.requests));

  // SC-001: what the cut costs is bounded by the parts in flight, not the
  // file. Bytes written but lost in the socket are all that is sent twice.
  const sent = server.requests.reduce((n, r) => n + r.written, 0);
  assert.ok(
    sent - SIZE <= concurrency * partSize,
    `sent ${sent - SIZE} bytes beyond the file`,
  );
});
