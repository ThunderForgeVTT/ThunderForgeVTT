import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";

/**
 * An accessibility check against the page as the browser actually rendered it
 * (spec 051, first used by the paused-play notice and the operator's pause
 * dialog).
 *
 * # What it holds a page to
 *
 * WCAG 2.2 AA, by axe's tags: the A and AA rules of 2.0, 2.1 and 2.2. Axe's
 * "best-practice" rules are deliberately not included — they are advice, not
 * the standard, and a test that fails on advice gets its assertion deleted.
 *
 * # Scope it to what the test is about
 *
 * Pass `selector` to check only the part of the page the test introduced — a
 * dialog, a banner. A whole-page check fails a paused-play test on a contrast
 * problem in the sidebar, which is real but is not that test's finding and
 * will be muted the first time it blocks a merge.
 *
 * # The failure message is the report
 *
 * `expect(violations).toEqual([])` prints axe's raw result objects, which run
 * to hundreds of lines per rule. The message below is one line per rule and
 * one per offending node, which is what a person fixing it needs.
 */

const WCAG_22_AA_TAGS = [
  "wcag2a",
  "wcag2aa",
  "wcag21a",
  "wcag21aa",
  "wcag22aa",
];

export async function expectNoAxeViolations(
  page: Page,
  selector?: string,
): Promise<void> {
  let builder = new AxeBuilder({ page }).withTags(WCAG_22_AA_TAGS);
  if (selector !== undefined) {
    builder = builder.include(selector);
  }
  const { violations } = await builder.analyze();

  const report = violations
    .map((violation) => {
      const nodes = violation.nodes
        .map(
          (node) =>
            `    - ${node.target.map((part) => JSON.stringify(part)).join(" > ")}`,
        )
        .join("\n");
      return `  ${violation.id} [${violation.impact ?? "unknown"}]: ${violation.help}\n${nodes}`;
    })
    .join("\n");

  expect(
    violations.length,
    `axe found ${violations.length} WCAG 2.2 AA violation(s)${
      selector === undefined ? "" : ` within ${selector}`
    }:\n${report}`,
  ).toBe(0);
}
