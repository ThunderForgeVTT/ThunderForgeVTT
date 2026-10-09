import { expect, test } from "./fixtures/test";

/**
 * Spec 086 US6, the "off" state (FR-017, SC-013): every test stack runs with
 * `TELEMETRY=false`, so the served config says off, the app never downloads
 * its telemetry chunk, and no request leaves the instance's origin.
 */

/** What only the lazy chunk would fetch. */
const CHUNK =
  /\/src\/telemetry\/|\/telemetry\/src\/(browser|otlp)|web-vitals|assets\/chunks\/telemetry-/;

test.describe("Telemetry, switched off", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("the config says off, the chunk is never fetched, and nothing leaves the origin", async ({
    page,
    baseURL,
  }) => {
    const urls: string[] = [];
    page.on("request", (r) => urls.push(r.url()));

    const config = await page.request.get("/telemetry.json");
    expect(config.status()).toBe(200);
    expect(config.headers()["cache-control"]).toBe("no-store");
    expect(await config.json()).toEqual({ enabled: false });

    const asked = page.waitForRequest((r) =>
      new URL(r.url()).pathname.endsWith("/telemetry.json"),
    );
    await page.goto("/login");
    await asked;
    await page.waitForLoadState("load");
    await expect(page.getByTestId("footer-legal-links")).toBeVisible();
    // Long enough for a chunk that was going to arrive after `load` to ask.
    await page.waitForTimeout(1500);

    expect(urls.filter((u) => CHUNK.test(u))).toEqual([]);
    const origin = new URL(baseURL ?? page.url()).origin;
    const away = urls.filter((u) => {
      const url = new URL(u);
      if (url.protocol === "data:" || url.protocol === "blob:") return false;
      return url.origin !== origin;
    });
    expect(away).toEqual([]);
  });
});
