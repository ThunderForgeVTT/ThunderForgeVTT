import { expect, type Page } from "@playwright/test";

/**
 * Spec 090 US1: open the built landing the way a visitor sees it, with
 * nothing left to load and nothing reaching past the preview server.
 *
 * GitHub (`/gh/*` and the avatars) is answered here with a fixed sky, so
 * the star chart and the nav count render the same on every run and never
 * depend on api.github.com. Any console error, page error or failed
 * same-origin request fails the test that opened the page (FR-007).
 */

export type Scheme = "light" | "dark";
export const SCHEMES: Scheme[] = ["light", "dark"];

const people = (prefix: string, n: number) =>
  Array.from({ length: n }, (_, i) => ({
    login: `${prefix}${i + 1}`,
    avatar_url: `https://avatars.githubusercontent.com/u/${1000 + i}?v=4`,
    html_url: `https://github.com/${prefix}${i + 1}`,
    type: "User",
  }));

const AVATAR =
  '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#7a6a58"/></svg>';

export interface Opened {
  /** Everything the guard caught, for a test that wants to assert on it. */
  problems: string[];
  /** Every same-origin URL the page requested, in order. */
  requests: string[];
}

export async function openLanding(
  page: Page,
  { scheme = "light" as Scheme, settle = true } = {},
): Promise<Opened> {
  const problems: string[] = [];
  const requests: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !isMetaProbe(m.location().url)) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`pageerror: ${e.message}`));
  page.on("request", (r) => requests.push(r.url()));
  page.on("requestfailed", (r) => {
    // A navigation away (a CTA click) aborts what was in flight; that is
    // the browser's business, not a broken page.
    if (r.failure()?.errorText === "net::ERR_ABORTED") return;
    problems.push(`requestfailed: ${r.url()} ${r.failure()?.errorText}`);
  });
  page.on("response", (r) => {
    if (r.status() >= 400 && !(r.status() === 404 && isMetaProbe(r.url()))) {
      problems.push(`http ${r.status()}: ${r.url()}`);
    }
  });

  await page.route("**/gh/repo", (route) =>
    route.fulfill({ json: { stargazers_count: 128 } }),
  );
  await page.route("**/gh/contributors", (route) =>
    route.fulfill({ json: people("builder", 3) }),
  );
  await page.route("**/gh/stargazers*", (route) =>
    route.fulfill({ json: people("stargazer", 24) }),
  );
  await page.route("https://avatars.githubusercontent.com/**", (route) =>
    route.fulfill({ contentType: "image/svg+xml", body: AVATAR }),
  );

  await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
  await page.goto("/");
  if (settle) await settleLanding(page);
  return { problems, requests };
}

/** Scroll through once so every lazy section renders, then wait for it. */
export async function settleLanding(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const step = window.innerHeight * 0.8;
    for (let y = 0; y < document.body.scrollHeight; y += step) {
      window.scrollTo(0, y);
      await new Promise((r) => setTimeout(r, 40));
    }
    window.scrollTo(0, 0);
  });
  await expect(page.locator(".tray--loading")).toHaveCount(0);
  await expect(page.locator(".legend-layout--loading")).toHaveCount(0);
  await expect(page.getByText("Pick a roll.")).toBeVisible();
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(
      [...document.images].map((img) => img.decode().catch(() => undefined)),
    );
  });
}

/**
 * Bevy asks for `<image>.meta` before every image it loads
 * (`AssetMetaCheck::Always`), and a 404 is the answer it needs: the server's
 * asset routes give the same one on purpose (`assets_serve/canvas.rs`). So a
 * 404 on a `.meta` path is the engine working, not a broken page.
 */
const isMetaProbe = (url: string) => url.endsWith(".meta");

/** Fails with every problem the guard saw. */
export function expectClean(opened: Opened): void {
  expect(opened.problems, opened.problems.join("\n")).toEqual([]);
}
