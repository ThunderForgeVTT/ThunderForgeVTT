import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";

/**
 * The landing's accessibility checks, shared by every state the page is
 * checked in (spec 090 FR-003 and FR-004; spec 091 SC-005).
 *
 * Axe runs the WCAG 2.2 A/AA rules and the best-practice set (landmarks,
 * one h1, region, skip link), which the web app's fixture leaves out. No
 * rule is exempted.
 */

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

/** Axe's violations, one line per rule with up to five targets. */
export async function axeReport(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  return violations.map(
    (v) =>
      `${v.id} (${v.impact}): ${v.help}\n${v.nodes
        .slice(0, 5)
        .map((n) => `    ${n.target.join(" ")}`)
        .join("\n")}`,
  );
}

/** Anything past the right edge, and any element that clips its own text. */
export function overflowReport(page: Page): Promise<string[]> {
  return page.evaluate(() => {
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
}

export interface Stop {
  key: string;
  path: string;
  where: string;
  ring: boolean;
  covered: boolean;
  inView: boolean;
}

/** The focused element: whether its ring shows, it is on screen, and uncovered. */
export function focusStop(page: Page): Promise<Stop | null> {
  return page.evaluate(() => {
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
}

/** What is wrong with a stop, if anything. */
export function stopProblems(stop: Stop): string[] {
  const out: string[] = [];
  if (!stop.ring) out.push(`no visible focus: ${stop.path}`);
  if (!stop.inView) out.push(`not fully on screen (${stop.where}): ${stop.path}`);
  if (stop.covered) out.push(`covered by something else: ${stop.path}`);
  return out;
}
