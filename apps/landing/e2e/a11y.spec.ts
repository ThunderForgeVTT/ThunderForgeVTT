import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { expectClean, openLanding, SCHEMES } from "./support/page.ts";

/**
 * Spec 090 US1 (FR-003, FR-004): the whole front page, at this project's
 * width, in both colour schemes.
 *
 * Axe runs the WCAG 2.2 A/AA rules and the best-practice set (landmarks,
 * one h1, region, skip link), which the web app's fixture leaves out. No
 * rule is exempted.
 */

const TAGS = [
  "wcag2a",
  "wcag2aa",
  "wcag21a",
  "wcag21aa",
  "wcag22aa",
  "best-practice",
];

for (const scheme of SCHEMES) {
  test.describe(`${scheme} scheme`, () => {
    test("axe finds nothing on the whole page", async ({ page }) => {
      const opened = await openLanding(page, { scheme });
      const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
      const report = violations.map(
        (v) =>
          `${v.id} (${v.impact}): ${v.help}\n${v.nodes
            .slice(0, 5)
            .map((n) => `    ${n.target.join(" ")}`)
            .join("\n")}`,
      );
      expect(report, report.join("\n")).toEqual([]);
      expectClean(opened);
    });

    test("nothing runs past the right edge, and no text is clipped", async ({ page }) => {
      const opened = await openLanding(page, { scheme });
      const found = await page.evaluate(() => {
        const width = document.documentElement.clientWidth;
        const out: string[] = [];
        const name = (el: Element) =>
          `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""}${
            el.classList.length ? `.${[...el.classList].join(".")}` : ""
          }`;
        if (document.documentElement.scrollWidth > width) {
          out.push(`page scrolls sideways: ${document.documentElement.scrollWidth} > ${width}`);
        }
        for (const el of document.querySelectorAll("body *")) {
          const style = getComputedStyle(el);
          if (style.display === "none" || style.visibility === "hidden") continue;
          if (el.closest("[aria-hidden='true'], .sr-only, svg")) continue;
          const box = el.getBoundingClientRect();
          if (box.width === 0 || box.height === 0) continue;
          // Inside a sideways scroller (the atlas strip, the terminal) running
          // past the edge is the design; the scroller itself must still fit.
          const scroller = (start: Element | null) => {
            for (let p = start; p; p = p.parentElement) {
              if (["auto", "scroll"].includes(getComputedStyle(p).overflowX)) return true;
            }
            return false;
          };
          if (style.position !== "fixed" && box.right > width + 1 && !scroller(el.parentElement)) {
            out.push(`${name(el)} ends at ${Math.round(box.right)} > ${width}`);
          }
          const clips = ["hidden", "clip"].includes(style.overflowX) ||
            ["hidden", "clip"].includes(style.overflowY);
          const hasText = [...el.childNodes].some(
            (n) => n.nodeType === Node.TEXT_NODE && n.textContent!.trim() !== "",
          );
          if (
            clips &&
            hasText &&
            (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1)
          ) {
            out.push(`${name(el)} clips its text`);
          }
        }
        return out;
      });
      expect(found, found.join("\n")).toEqual([]);
      expectClean(opened);
    });
  });
}

test("the keyboard walk starts with a skip link, and every stop is shown and on screen", async ({
  page,
}) => {
  const opened = await openLanding(page);
  await page.keyboard.press("Tab");
  const first = page.locator(":focus");
  await expect(first).toHaveText(/skip to content/i);
  await expect(first).toBeInViewport();
  await page.keyboard.press("Enter");
  await expect(page.locator(":focus")).toHaveAttribute("id", "content");

  // From the top again: every stop, in order, until focus leaves the page
  // or comes round to the start.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.locator("body").focus();
  const problems: string[] = [];
  const seen = new Set<string>();
  for (let i = 0; i < 200; i++) {
    await page.keyboard.press("Tab");
    const stop = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const box = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      const ring =
        (style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0) ||
        style.boxShadow !== "none";
      // What is drawn at the centre of the stop: the stop itself, something
      // inside it, or something on top of it.
      const cx = Math.min(Math.max(box.left + box.width / 2, 0), window.innerWidth - 1);
      const cy = Math.min(Math.max(box.top + box.height / 2, 0), window.innerHeight - 1);
      const top = document.elementFromPoint(cx, cy);
      const covered = !!top && top !== el && !el.contains(top) && !top.contains(el);
      const path = el.outerHTML.slice(0, 90);
      return {
        key: path + Math.round(box.top + window.scrollY),
        path,
        where: `${Math.round(box.left)},${Math.round(box.top)} to ${Math.round(box.right)},${Math.round(box.bottom)} in ${window.innerWidth}x${window.innerHeight}`,
        ring,
        covered,
        inView:
          box.top >= -1 &&
          box.left >= -1 &&
          box.bottom <= window.innerHeight + 1 &&
          box.right <= window.innerWidth + 1,
      };
    });
    if (!stop || seen.has(stop.key)) break;
    seen.add(stop.key);
    if (!stop.ring) problems.push(`no visible focus: ${stop.path}`);
    if (!stop.inView) problems.push(`not fully on screen (${stop.where}): ${stop.path}`);
    if (stop.covered) problems.push(`covered by something else: ${stop.path}`);
  }
  expect(seen.size).toBeGreaterThan(10);
  expect(problems, problems.join("\n")).toEqual([]);
  expectClean(opened);
});

test("the GitHub link in the nav keeps its name when the star count fails", async ({ page }) => {
  await page.route("**/gh/repo", (route) => route.fulfill({ status: 503, body: "" }));
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await expect(
    page.locator("nav").getByRole("link", { name: /ThunderForge on GitHub/ }),
  ).toBeVisible();
});
