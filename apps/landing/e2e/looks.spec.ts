import { expect, test, type Page } from "@playwright/test";
import { expectClean, openLanding, SCHEMES } from "./support/page.ts";

/**
 * Spec 090 US1 (T010): every section of the front page looks the way it was
 * drawn, at this project's width.
 *
 * The baselines are taken under `colorScheme: light`. The landing has one
 * designed scheme, so `dark` is compared against the SAME file (the
 * snapshot name carries no scheme): a browser default leaking through in
 * dark mode fails as a pixel difference.
 *
 * Masked, because they are not the page's to decide:
 * - the star counts (nav, star chart lede, support) and the star chart:
 *   they come from GitHub;
 * - the dice tray's felt (results and history): the tray's seed comes from
 *   `crypto`.
 */

const SECTIONS: { name: string; selector: string }[] = [
  { name: "hero", selector: '[data-section="hero"]' },
  { name: "dream", selector: '[data-section="dream"]' },
  { name: "dice", selector: '[data-section="dice"]' },
  { name: "map", selector: '[data-section="map"]' },
  // The room carries no data-section either: telemetry counts badges, never
  // the room (spec 091).
  { name: "room", selector: "section#room" },
  // Systems carries no data-section (telemetry does not track it).
  { name: "systems", selector: "section.systems" },
  { name: "numbers", selector: '[data-section="numbers"]' },
  { name: "self-host", selector: '[data-section="self-host"]' },
  { name: "telemetry", selector: '[data-section="telemetry"]' },
  { name: "stance", selector: '[data-section="stance"]' },
  { name: "stars", selector: '[data-section="stars"]' },
  { name: "support", selector: '[data-section="support"]' },
  { name: "footer", selector: '[data-section="footer"]' },
];

const masks = (page: Page) => [
  page.locator(".count"),
  page.locator(".stars-count"),
  page.locator(".star-field"),
  page.locator(".tray-felt"),
];

for (const scheme of SCHEMES) {
  test.describe(`${scheme} scheme`, () => {
    test("every section matches its baseline", async ({ page }) => {
      const opened = await openLanding(page, { scheme });
      for (const { name, selector } of SECTIONS) {
        const section = page.locator(selector);
        await expect(section).toHaveCount(1);
        await section.scrollIntoViewIfNeeded();
        await expect.soft(section, `${name} (${scheme})`).toHaveScreenshot(`${name}.png`, {
          animations: "disabled",
          caret: "hide",
          maxDiffPixelRatio: 0.01,
          mask: masks(page),
        });
      }
      expectClean(opened);
    });
  });
}

// A section screenshot is cropped to the section, so one that loses its
// centring but keeps its width still matches. That shipped once: a class
// shared with the numbers' `.measure` set "What we measure" flush left on
// a wide screen. Wider than the column, every section shares its edges.
test("on a wide screen every section sits in the one column", async ({ page }, info) => {
  test.skip(info.project.name !== "desktop", "one wide width is enough");
  await page.setViewportSize({ width: 2000, height: 1000 });
  const opened = await openLanding(page);
  const edges = await page.locator("main > .section").evaluateAll((all) =>
    all.map((e) => {
      const r = e.getBoundingClientRect();
      return `${e.className}: ${Math.round(r.left)}-${Math.round(r.right)}`;
    }),
  );
  expect(edges.length).toBeGreaterThan(5);
  const column = edges[0].split(": ")[1];
  expect(edges).toEqual(edges.map((e) => `${e.split(": ")[0]}: ${column}`));
  expectClean(opened);
});
