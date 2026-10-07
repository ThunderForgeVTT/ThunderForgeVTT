import { expect, test, type Route } from "./fixtures/test";
import {
  clickPlay,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 080 T023 (US2) and T031: the engine arrives in parts, and a small
 * file does not.
 *
 * Nothing is overridden here. The engine is far above the 16 MB threshold,
 * so the downloader's real numbers are what put it into parts, and the small
 * file below is fetched with those same numbers. Routing the requests also
 * turns off the browser's HTTP cache, so the context is cold for both.
 */

/** The `[start, end]` of a `Range: bytes=a-b` header. */
function bounds(range: string): [number, number] {
  const match = /^bytes=(\d+)-(\d+)$/.exec(range);
  if (!match) throw new Error(`not a closed byte range: ${range}`);
  return [Number(match[1]), Number(match[2])];
}

test("the engine downloads in several parts and still starts", async ({
  page,
}) => {
  test.setTimeout(300_000);

  // Watched, not routed: a route would buffer the whole first answer before
  // the page saw a byte of it. A fresh context has nothing cached anyway.
  const wasm: { range: string | null; status: number; length: number }[] = [];
  const recorded: Promise<void>[] = [];
  page.on("response", (response) => {
    // The engine's wasm itself, not the dev server's `?url` module for it.
    const url = new URL(response.url());
    if (!url.pathname.endsWith("engine_bg.wasm") || url.search) return;
    recorded.push(
      (async () => {
        const sent = await response.request().allHeaders();
        const got = await response.allHeaders();
        wasm.push({
          range: sent["range"] ?? null,
          status: response.status(),
          length: Number(got["content-length"] ?? NaN),
        });
      })(),
    );
  });

  await registerAndCreateWorld(
    page,
    `E2E Engine Parts ${uniqueSuffix()}`,
    "e2eparts",
  );
  await clickPlay(page);

  // Progress never moves backwards while the parts arrive.
  const loader = page.getByTestId("engine-load-indicator");
  const bar = page.getByTestId("engine-loader-progress");
  const seen: number[] = [];
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    if (!(await loader.isVisible().catch(() => false))) break;
    const raw = await bar
      .getAttribute("aria-valuenow", { timeout: 1_000 })
      .catch(() => null);
    if (raw !== null) seen.push(Number(raw));
    await page.waitForTimeout(250);
  }
  for (let i = 1; i < seen.length; i += 1) {
    expect(
      seen[i],
      `progress went backwards: ${seen.join(", ")}`,
    ).toBeGreaterThanOrEqual(seen[i - 1]);
  }

  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("engine-load-error")).toHaveCount(0);

  await Promise.all(recorded);
  const plain = wasm.filter(({ range }) => range === null);
  expect(plain, `one plain first answer: ${JSON.stringify(wasm)}`).toHaveLength(
    1,
  );
  const size = plain[0].length;
  const parts = wasm.filter(({ status }) => status === 206);
  expect(
    parts.length,
    `the engine must arrive in several parts: ${JSON.stringify(wasm)}`,
  ).toBeGreaterThan(1);
  const ranges = parts.map(({ range }) => range!);
  expect(new Set(ranges).size, "every part is a different range").toBe(
    ranges.length,
  );

  // Part 0 is read off the plain first answer; the ranges tile the rest,
  // each starting where the last ended and the last ending at the file's end.
  const sorted = ranges.map(bounds).sort((a, b) => a[0] - b[0]);
  expect(sorted[0][0]).toBeGreaterThan(0);
  for (let i = 1; i < sorted.length; i += 1) {
    expect(sorted[i][0]).toBe(sorted[i - 1][1] + 1);
  }
  expect(sorted[sorted.length - 1][1]).toBe(size - 1);

  // T031: a small file, same numbers, same cold context — one plain request.
  const small: (string | null)[] = [];
  await page.route("**/brand-mark.svg?probe*", async (route: Route) => {
    small.push(route.request().headers()["range"] ?? null);
    await route.fallback();
  });
  const length = await page.evaluate(async () => {
    const get = (
      globalThis as {
        __thunderforgeDownloadBytes?: (url: string) => Promise<Uint8Array>;
      }
    ).__thunderforgeDownloadBytes;
    if (!get) throw new Error("the page downloader was never installed");
    return (await get("/brand-mark.svg?probe=1")).length;
  });
  expect(length).toBeGreaterThan(0);
  expect(small, "a small file is one request with no Range").toEqual([null]);

  await page.unrouteAll({ behavior: "ignoreErrors" });
});
