import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  BOOT_TIMEOUT,
  BUILT,
  bootDiceFrame as boot,
  diceFrame as frame,
  engine,
  frames,
  isEngineFile,
  landed,
  landedOf,
  readManifest,
  seeded,
  throwButton,
} from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * Spec 090 US2 and US3: the real engine in the dice section, booted only on
 * the click, from the demo's own build (served at `/demo/` by the preview
 * server, T036).
 *
 * The engine is the demo's dev or release build, hundreds of megabytes in
 * dev, so the tests that boot it run at the desktop width only. What needs
 * no engine (nothing fetched before the click, the fallbacks) runs at every
 * width.
 */

test.describe("before the click", () => {
  test("no engine file is requested, scrolled top to bottom", async ({ page }) => {
    const opened = await openLanding(page);
    await expect(throwButton(page)).toBeVisible();
    expect(opened.requests.filter(isEngineFile)).toEqual([]);
    // Spec 091 FR-025: the room's art waits for its click, and the map the
    // room shares with the legend is fetched once.
    const paths = opened.requests.map((u) => new URL(u).pathname);
    expect(paths.filter((p) => p.startsWith("/room/"))).toEqual([]);
    expect(paths.filter((p) => p === "/maps/demo.webp").length).toBeLessThanOrEqual(1);
    expectClean(opened);
  });

  test("without WebGL2 the button is held back, with the reason and the demo", async ({ page }) => {
    await page.addInitScript(() => {
      const get = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, kind: string, ...rest: unknown[]) {
        return kind === "webgl2" ? null : (get as (...a: unknown[]) => unknown).call(this, kind, ...rest);
      } as typeof get;
    });
    const opened = await openLanding(page);
    await expect(throwButton(page)).toHaveCount(0);
    await expect(frame(page)).toContainText("can't run the table here (no WebGL2)");
    await expect(frame(page).getByRole("link", { name: "Try the demo" })).toBeVisible();
    expect(opened.requests.filter(isEngineFile)).toEqual([]);
    expectClean(opened);
  });

  test("a device under 4 GB is held back too", async ({ page }) => {
    await page.addInitScript(() => Object.defineProperty(navigator, "deviceMemory", { get: () => 2 }));
    await openLanding(page);
    await expect(frame(page)).toContainText("under 4 GB of memory");
  });

  test("so is one that asks to save data", async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "deviceMemory", { get: () => 8 });
      Object.defineProperty(navigator, "connection", { get: () => ({ saveData: true }) });
    });
    await openLanding(page);
    await expect(frame(page)).toContainText("asks to save data");
  });
});

test.describe("the engine", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  test("the click loads the demo's engine, with its progress in words", async ({ page }) => {
    const opened = await openLanding(page);
    await frame(page).scrollIntoViewIfNeeded();
    await throwButton(page).click();
    await expect(frame(page)).toContainText(/Loading the table: \d+\.\d of \d+\.\d MB\. Once; the demo reuses it\./);
    await expect(frame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
    const m = readManifest();
    const paths = opened.requests.filter(isEngineFile).map((u) => new URL(u).pathname);
    expect(paths).toContain("/demo/engine.json");
    expect(paths).toContain(`/demo/${m.wasm}`);
    expect(paths).toContain(`/demo/${m.js}`);
    const canvas = frame(page).locator("canvas");
    await expect(canvas).toHaveAttribute("aria-hidden", "true");
    await expect(canvas).toHaveAttribute("tabindex", "-1");
    expectClean(opened);
  });

  test("seeded throws land on the crate's faces, Fate and d100 included", async ({ page }) => {
    const opened = await openLanding(page);
    await boot(page);
    for (const [i, formula] of ["1d20+5", "4dF", "1d100", "4d6kh3", "8d6"].entries()) {
      const { detail, json } = seeded(formula, i + 3, `seeded-${i}`);
      await engine(page, "apply_world_command", json);
      const thrown = await landedOf(page, `seeded-${i}`);
      expect(thrown.dice.map((d) => [d.face, d.kept]), formula).toEqual(
        detail.dice.map((d) => [d.final_value, d.kept]),
      );
    }
    expectClean(opened);
  });

  test("tray rolls are thrown; quick re-rolls never lose a result", async ({ page }) => {
    const opened = await openLanding(page);
    await boot(page);
    const before = (await landed(page)).length;
    const attack = page.getByRole("button", { name: /Attack/ });
    await attack.click();
    await attack.click();
    await attack.click();
    await expect(page.getByRole("list", { name: "Earlier rolls" }).getByRole("listitem")).toHaveCount(2);
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBe(before + 3);
    const last = (await landed(page)).at(-1)!;
    const shown = await page.getByRole("list", { name: "Dice rolled" }).getByRole("listitem").allTextContents();
    expect(last.dice.map((d) => String(d.face))).toEqual(shown);
    expectClean(opened);
  });

  test("reduced motion follows the setting, before the first throw and on change", async ({ page }) => {
    const opened = await openLanding(page); // emulates reduce
    await boot(page);
    await page.getByRole("button", { name: /Attack/ }).click();
    expect((await landed(page)).at(-1)!.reducedMotion).toBe(true);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    const count = (await landed(page)).length;
    await page.getByRole("button", { name: /Fireball/ }).click();
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBe(count + 1);
    expect((await landed(page)).at(-1)!.reducedMotion).toBe(false);
    expectClean(opened);
  });

  test("frames stop out of view and in a hidden tab, and resume", async ({ page }) => {
    const opened = await openLanding(page);
    await boot(page);
    const a = await frames(page);
    await expect.poll(() => frames(page)).toBeGreaterThan(a);

    await page.evaluate(() => window.scrollTo(0, 0));
    await expect(frame(page)).toHaveClass(/engine-frame--paused/);
    await page.waitForTimeout(300);
    const stopped = await frames(page);
    await page.waitForTimeout(1_000);
    expect(await frames(page), "frames drawn while out of view").toBe(stopped);

    await frame(page).scrollIntoViewIfNeeded();
    await expect(frame(page)).toHaveClass(/engine-frame--running/);
    await expect.poll(() => frames(page)).toBeGreaterThan(stopped);

    const setHidden = (hidden: boolean) =>
      page.evaluate((hidden) => {
        Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
        document.dispatchEvent(new Event("visibilitychange"));
      }, hidden);
    await setHidden(true);
    await expect(frame(page)).toHaveClass(/engine-frame--paused/);
    await page.waitForTimeout(300);
    const hidden = await frames(page);
    await page.waitForTimeout(1_000);
    expect(await frames(page), "frames drawn in a hidden tab").toBe(hidden);
    await setHidden(false);
    await expect.poll(() => frames(page)).toBeGreaterThan(hidden);
    expectClean(opened);
  });

  test("a broken manifest restores the poster, says so, keeps the tray, and retries", async ({ page }) => {
    const opened = await openLanding(page);
    await page.route("**/demo/engine.json", (route) => route.fulfill({ status: 200, body: "{}" }));
    await frame(page).scrollIntoViewIfNeeded();
    await throwButton(page).click();
    await expect(frame(page).getByRole("alert")).toHaveText("The table didn't load. The dice above still work.");
    await expect(frame(page).locator(".engine-poster")).toBeVisible();
    await page.getByRole("button", { name: /Fireball/ }).click();
    await expect(page.locator(".total-num")).toBeVisible();

    await page.unroute("**/demo/engine.json");
    await frame(page).getByRole("button", { name: "Try again" }).click();
    await expect(frame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
    expectClean(opened);
  });

  test("axe still finds nothing with the engine running, after a throw", async ({ page }) => {
    const opened = await openLanding(page);
    await boot(page);
    await page.getByRole("button", { name: /Attack/ }).click();
    const { violations } = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"])
      .analyze();
    expect(violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
    expectClean(opened);
  });
});
