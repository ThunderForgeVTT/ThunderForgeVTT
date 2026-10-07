import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { download, downloadBytes, type DownloadProgress } from "./download.ts";
import { DownloadError, VersionChangedError } from "./errors.ts";
import {
  DEFAULT_DOWNLOAD_SETTINGS,
  retryDelay,
  resolveSettings,
} from "./settings.ts";
import { fakeServer, patterned } from "./testing/fakeServer.ts";

const URL_ = "https://example.test/file.bin";
/** Small numbers so a 100-byte file is "large". */
const SMALL = {
  threshold: 10,
  partSize: 10,
  concurrency: 3,
  retries: 3,
  retryDelayMs: 1,
};

const ranges = (s: ReturnType<typeof fakeServer>) =>
  s.requests.map((r) => r.range);

describe("settings", () => {
  it("has one set of defaults", () => {
    assert.equal(DEFAULT_DOWNLOAD_SETTINGS.threshold, 16 * 1024 * 1024);
    assert.equal(DEFAULT_DOWNLOAD_SETTINGS.partSize, 8 * 1024 * 1024);
    assert.equal(DEFAULT_DOWNLOAD_SETTINGS.concurrency, 4);
    assert.equal(resolveSettings({ concurrency: 0 }).concurrency, 1);
  });

  it("pauses longer each try, up to a cap", () => {
    const s = resolveSettings({ retryDelayMs: 250 });
    assert.deepEqual(
      [1, 2, 3, 4, 5, 6].map((a) => retryDelay(s, a)),
      [250, 500, 1000, 2000, 4000, 4000],
    );
  });
});

describe("one plain request (FR-016)", () => {
  const cases: [
    string,
    Parameters<typeof fakeServer>[1],
    Partial<typeof SMALL & { enabled: boolean }>,
  ][] = [
    ["below the threshold", {}, { ...SMALL, threshold: 1000 }],
    ["no ranges offered", { acceptRanges: false }, SMALL],
    ["a compressed answer", { encoding: "br" }, SMALL],
    ["no version tag", { etag: null }, SMALL],
    ["a weak version tag only", { etag: 'W/"v1"' }, SMALL],
    ["parts switched off", {}, { ...SMALL, enabled: false }],
  ];
  for (const [name, serverOptions, settings] of cases) {
    it(name, async () => {
      const file = patterned(100);
      const server = fakeServer(file, serverOptions);
      const got = await download(URL_, { fetch: server.fetch, settings });
      assert.equal(got.mode, "plain");
      const bytes = new Uint8Array(await new Response(got.body).arrayBuffer());
      assert.deepEqual(bytes, file);
      assert.deepEqual(ranges(server), [null]);
    });
  }

  it("reports an unknown total for a compressed answer", async () => {
    const server = fakeServer(patterned(100), { encoding: "gzip" });
    const got = await download(URL_, { fetch: server.fetch, settings: SMALL });
    assert.equal(got.total, null);
    await got.body.cancel();
  });

  it("takes a last-modified date when there is no tag", async () => {
    const file = patterned(100);
    const server = fakeServer(file, {
      etag: null,
      lastModified: "Wed, 21 Oct 2015 07:28:00 GMT",
    });
    const bytes = await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
    });
    assert.deepEqual(bytes, file);
    assert.equal(server.requests[1].ifRange, "Wed, 21 Oct 2015 07:28:00 GMT");
  });
});

describe("parts (FR-012, FR-013)", () => {
  it("hands bytes on in order when parts finish out of order", async () => {
    const file = patterned(100);
    // Earlier parts are slower, so later ones finish first.
    const server = fakeServer(file, {
      behave: (r) =>
        r.range ? { delayMs: Math.max(0, 12 - r.index * 2) } : { delayMs: 6 },
    });
    const got = await download(URL_, { fetch: server.fetch, settings: SMALL });
    assert.equal(got.mode, "parts");
    assert.equal(got.total, 100);
    const bytes = new Uint8Array(await new Response(got.body).arrayBuffer());
    assert.deepEqual(bytes, file);
  });

  it("uses the first answer as part 0 and fetches no byte twice", async () => {
    const server = fakeServer(patterned(95));
    await downloadBytes(URL_, { fetch: server.fetch, settings: SMALL });
    assert.deepEqual(ranges(server), [
      null,
      ...Array.from(
        { length: 9 },
        (_, i) => `bytes=${(i + 1) * 10}-${Math.min((i + 2) * 10, 95) - 1}`,
      ),
    ]);
    for (const r of server.requests.slice(1)) assert.equal(r.ifRange, '"v1"');
  });

  it("never opens more than `concurrency` parts ahead of the reader", async () => {
    const server = fakeServer(patterned(100));
    const got = await download(URL_, { fetch: server.fetch, settings: SMALL });
    const reader = got.body.getReader();
    await reader.read();
    await new Promise((r) => setTimeout(r, 30));
    assert.equal(
      server.requests.length,
      SMALL.concurrency,
      "first answer + 2 ranged parts",
    );
    // Reading on lets more start, never more than three at once.
    for (;;) if ((await reader.read()).done) break;
    assert.ok(server.maxOpen <= SMALL.concurrency, `maxOpen ${server.maxOpen}`);
  });

  it("gives a Response that compiles like the original", async () => {
    const server = fakeServer(patterned(100));
    const got = await download(URL_, { fetch: server.fetch, settings: SMALL });
    const response = got.toResponse();
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("content-length"), "100");
    assert.equal((await response.arrayBuffer()).byteLength, 100);
  });
});

describe("retry and resume (FR-014)", () => {
  it("asks a broken part again for only the bytes it lacks", async () => {
    const file = patterned(100);
    let cut = false;
    const server = fakeServer(file, {
      behave: (r) => {
        if (r.range === "bytes=20-29" && !cut) {
          cut = true;
          return { cutAfter: 4 };
        }
        return undefined;
      },
    });
    const bytes = await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
    });
    assert.deepEqual(bytes, file);
    assert.ok(
      ranges(server).includes("bytes=24-29"),
      JSON.stringify(ranges(server)),
    );
    assert.equal(ranges(server).filter((r) => r === "bytes=20-29").length, 1);
  });

  it("resumes part 0 from where the first answer broke", async () => {
    const file = patterned(100);
    const server = fakeServer(file, {
      behave: (r) => (r.index === 0 ? { cutAfter: 7 } : undefined),
    });
    const bytes = await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
    });
    assert.deepEqual(bytes, file);
    assert.ok(ranges(server).includes("bytes=7-9"));
  });

  it("retries a server error", async () => {
    let failures = 2;
    const server = fakeServer(patterned(100), {
      behave: (r) =>
        r.range === "bytes=50-59" && failures-- > 0
          ? { status: 503 }
          : undefined,
    });
    const bytes = await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
    });
    assert.equal(bytes.length, 100);
  });

  it("fails with one error once the retries run out, delivering nothing as whole (FR-019)", async () => {
    const server = fakeServer(patterned(100), {
      behave: (r) => (r.range === "bytes=30-39" ? { status: 503 } : undefined),
    });
    await assert.rejects(
      downloadBytes(URL_, { fetch: server.fetch, settings: SMALL }),
      (e: unknown) => e instanceof DownloadError && e.status === 503,
    );
    assert.equal(
      ranges(server).filter((r) => r === "bytes=30-39").length,
      SMALL.retries + 1,
    );
  });

  it("counts progress once per byte, never backwards (FR-017)", async () => {
    const seen: DownloadProgress[] = [];
    let cut = false;
    const server = fakeServer(patterned(100), {
      behave: (r) => {
        if (r.range === "bytes=40-49" && !cut) {
          cut = true;
          return { cutAfter: 5 };
        }
        return undefined;
      },
    });
    await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
      onProgress: (p) => seen.push(p),
    });
    for (let i = 1; i < seen.length; i++)
      assert.ok(seen[i].loaded >= seen[i - 1].loaded);
    assert.deepEqual(seen.at(-1), { loaded: 100, total: 100 });
  });
});

describe("one version only (FR-015)", () => {
  it("fails a stream that already handed on bytes of the old version", async () => {
    const server = fakeServer(patterned(100), {
      behave: (r) => {
        if (r.index === 1) server.replace(patterned(100, 7), '"v2"');
        return undefined;
      },
    });
    const got = await download(URL_, { fetch: server.fetch, settings: SMALL });
    await assert.rejects(new Response(got.body).arrayBuffer(), (e: unknown) => {
      // Response wraps a stream error; the cause is ours.
      return (
        e instanceof VersionChangedError ||
        (e as Error).cause instanceof VersionChangedError ||
        /changed/.test(String(e))
      );
    });
  });

  it("starts again on the new version when the whole file is wanted", async () => {
    const next = patterned(100, 7);
    let replaced = false;
    const server = fakeServer(patterned(100), {
      behave: (r) => {
        if (r.range === "bytes=30-39" && !replaced) {
          replaced = true;
          server.replace(next, '"v2"');
        }
        return undefined;
      },
    });
    const bytes = await downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
    });
    assert.deepEqual(bytes, next);
  });

  it("treats a part with another tag as a change, even if the server ignored If-Range", async () => {
    const parts: string[] = [];
    const server = fakeServer(patterned(100));
    const sneaky: typeof fetch = async (input, init) => {
      const res = await server.fetch(input, init);
      const range = new Headers(init?.headers).get("range");
      if (range) parts.push(range);
      if (range === "bytes=60-69") {
        const h = new Headers(res.headers);
        h.set("etag", '"v2"');
        return new Response(res.body, { status: res.status, headers: h });
      }
      return res;
    };
    const got = await download(URL_, { fetch: sneaky, settings: SMALL });
    await assert.rejects(new Response(got.body).arrayBuffer());
  });
});

describe("refusal and cancellation", () => {
  it("does not retry a refusal", async () => {
    const server = fakeServer(patterned(100), {
      behave: () => ({ status: 403 }),
    });
    await assert.rejects(
      download(URL_, { fetch: server.fetch, settings: SMALL }),
      (e: unknown) => e instanceof DownloadError && e.status === 403,
    );
    assert.equal(server.requests.length, 1);
  });

  it("stops every part in flight when cancelled (FR-018)", async () => {
    const server = fakeServer(patterned(100), {
      behave: () => ({ delayMs: 5 }),
    });
    const controller = new AbortController();
    const pending = downloadBytes(URL_, {
      fetch: server.fetch,
      settings: SMALL,
      signal: controller.signal,
    });
    await new Promise((r) => setTimeout(r, 15));
    controller.abort(new Error("left the scene"));
    await assert.rejects(pending, /left the scene/);
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(server.open, 0);
    const count = server.requests.length;
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(
      server.requests.length,
      count,
      "nothing new starts after a cancel",
    );
  });
});
