import { expect, test } from "@playwright/test";
import { freshCredentials, register, uniqueSuffix } from "./fixtures/helpers";

/**
 * The canvas performance readout.
 *
 * Worth an end-to-end test rather than only unit coverage because the two
 * numbers that matter come from places a unit test has to fake: `fps` is
 * mirrored out of a Bevy `World` that `App::run()` owns and never returns
 * from, and the latency is a real round trip to the server. Faking both
 * would leave the test asserting that a component renders its own props.
 */
test("the readout is off until asked for, then shows live frames and latency", async ({
  page,
}) => {
  test.setTimeout(180_000);

  await register(page, freshCredentials("e2emon"));
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(`E2E Monitor ${uniqueSuffix()}`);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  await page.getByTestId("play-button").click({ timeout: 20_000 });
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 20_000 });
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });

  // Diagnostics nobody asked for must not be sitting over the map.
  await expect(page.getByTestId("engine-monitor")).toHaveCount(0);

  await page.getByTestId("world-dock").getByRole("button", { name: /settings/i }).click();
  await page.getByTestId("engine-monitor-toggle").click();

  const monitor = page.getByTestId("engine-monitor");
  await expect(monitor).toBeVisible({ timeout: 10_000 });

  // A real frame rate, not a placeholder and not a confident zero.
  await expect
    .poll(
      async () => {
        const text = await page.getByTestId("engine-monitor-fps").innerText();
        const fps = Number.parseInt(text, 10);
        return Number.isNaN(fps) ? -1 : fps;
      },
      {
        timeout: 30_000,
        message: "the readout should report the engine's actual frame rate",
      },
    )
    .toBeGreaterThan(0);

  // And a real round trip, which only arrives once a heartbeat completes.
  await expect
    .poll(
      async () => {
        const text = await page.getByTestId("engine-monitor-latency").innerText();
        const ms = Number.parseInt(text, 10);
        return Number.isNaN(ms) ? -1 : ms;
      },
      {
        timeout: 30_000,
        message: "the readout should report a real heartbeat round trip",
      },
    )
    .toBeGreaterThanOrEqual(0);

  // The preference outlives the page, or it is not a preference.
  await page.reload();
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("engine-monitor")).toBeVisible({ timeout: 20_000 });
});
