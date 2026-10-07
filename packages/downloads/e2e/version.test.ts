/**
 * Spec 080 T027 (SC-005): the file is replaced while its parts are on the
 * way. Twenty times over, at a different moment each time, the result is
 * one version or the other, never a splice of the two.
 */
import assert from "node:assert/strict";
import { after, it } from "node:test";
import { download, downloadBytes, VersionChangedError } from "../src/index.ts";
import { fileServer, generated, sha256 } from "./server.ts";

const SIZE = 6 * 1024 * 1024;
const SETTINGS = { threshold: 1024 * 1024, partSize: 512 * 1024, concurrency: 3 };
const server = await fileServer(generated(SIZE, 1));
after(() => server.close());

const RUNS = 20;

for (let run = 0; run < RUNS; run++) {
  it(`run ${run + 1}: one version only`, async () => {
    const a = generated(SIZE, 1000 + run);
    const b = generated(SIZE, 2000 + run);
    server.swap(a);
    // Answers 1..n are parts; swap before a different one each run.
    const swapBefore = 1 + (run % 8);
    server.onAnswer = (index) => {
      if (index === swapBefore) server.swap(b);
    };
    server.requests.length = 0;

    // The swap always lands before the last part, so the whole-file reader
    // starts again and ends with the new version alone.
    const whole = await downloadBytes(server.url, { settings: SETTINGS });
    assert.equal(sha256(whole), sha256(b), "downloadBytes returned a splice");

    // A stream has already handed part 0 on, so it cannot start again: it
    // fails rather than continue with the other version's bytes.
    server.swap(a);
    server.requests.length = 0;
    const got = await download(server.url, { settings: SETTINGS });
    await assert.rejects(
      new Response(got.body).arrayBuffer(),
      VersionChangedError,
    );
  });
}
