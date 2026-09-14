/**
 * `@playwright/test`, plus one automatic fixture: "did the app even load?"
 *
 * # Why
 *
 * On 2026-09-14 a merge added a dependency nobody installed, and the first-run
 * lane failed with a blank white screenshot and `expect(page).toHaveURL`
 * timing out. The real error — a module Vite could not resolve — was in the
 * browser console, which no report showed. Every failure of that shape costs
 * the same detour: the assertion names the *symptom* on the page, never the
 * reason the page is empty.
 *
 * # What it does
 *
 * For every browser context a test creates — the default one and any made
 * with `browser.newContext()` / `browser.newPage()` — it records console
 * errors, uncaught page errors, and scripts, stylesheets or wasm that failed
 * to arrive (a network failure, or an HTTP error status). A passing test
 * pays for a few listeners and nothing else.
 *
 * When a test fails, the record is attached as `load-diagnostics`. If the
 * record shows the frontend did not load — an asset failed, or a page's
 * `#root` is empty beside an uncaught error — the fixture also adds an error
 * that says so first: `frontend failed to load: <the error>`. It never fails
 * a test that passed.
 *
 * Import `test` and `expect` from here rather than from `@playwright/test`;
 * everything else `@playwright/test` exports is re-exported unchanged.
 */

import {
  test as base,
  type Browser,
  type BrowserContext,
  type Page,
  type Request,
} from "@playwright/test";

export * from "@playwright/test";

type Problem = {
  kind: "console" | "page error" | "request failed" | "http error";
  text: string;
  url?: string;
};

const MAX_PROBLEMS = 100;

/** Resources without which the app cannot render at all. */
function isAppAsset(request: Request): boolean {
  const type = request.resourceType();
  return (
    type === "script" ||
    type === "stylesheet" ||
    /\.wasm(\?|$)/.test(request.url())
  );
}

function watchContext(context: BrowserContext, problems: Problem[]) {
  const push = (problem: Problem) => {
    if (problems.length < MAX_PROBLEMS) problems.push(problem);
  };
  context.on("console", (message) => {
    if (message.type() !== "error") return;
    push({
      kind: "console",
      text: message.text(),
      url: message.location().url || undefined,
    });
  });
  context.on("weberror", (webError) => {
    const error = webError.error();
    push({
      kind: "page error",
      text: error.stack ?? error.message ?? String(error),
      url: webError.page()?.url(),
    });
  });
  context.on("requestfailed", (request) => {
    const reason = request.failure()?.errorText ?? "failed";
    // Navigating away aborts whatever was in flight; that is not a failure.
    if (reason.includes("ERR_ABORTED")) return;
    // A document that could not be fetched at all means no dev server.
    if (!isAppAsset(request) && request.resourceType() !== "document") return;
    push({ kind: "request failed", text: reason, url: request.url() });
  });
  context.on("response", (response) => {
    if (response.status() < 400 || !isAppAsset(response.request())) return;
    push({
      kind: "http error",
      text: `HTTP ${response.status()} ${response.statusText()}`.trim(),
      url: response.url(),
    });
  });
}

/** Pages whose `#root` rendered nothing, within a short, bounded look. */
async function blankPages(pages: Page[]): Promise<string[]> {
  const blank: string[] = [];
  for (const page of pages) {
    if (page.isClosed()) continue;
    const empty = await Promise.race([
      page
        .evaluate(() => {
          const root = document.getElementById("root");
          return root ? root.childElementCount === 0 : false;
        })
        .catch(() => false),
      new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 2000)),
    ]);
    if (empty) blank.push(page.url());
  }
  return blank;
}

function describe(problem: Problem): string {
  const firstLine = problem.text.split("\n")[0];
  return problem.url ? `${firstLine} (${problem.url})` : firstLine;
}

export const test = base.extend<{ loadDiagnostics: void }>({
  loadDiagnostics: [
    async (
      { browser, context }: { browser: Browser; context: BrowserContext },
      use,
      testInfo,
    ) => {
      const problems: Problem[] = [];
      const contexts: BrowserContext[] = [context];
      watchContext(context, problems);

      // Contexts a test makes for a second or third user. `newPage` goes
      // through `newContext` on the same object, so this catches both.
      const original = browser.newContext.bind(browser);
      browser.newContext = async (...args) => {
        const created = await original(...args);
        contexts.push(created);
        watchContext(created, problems);
        return created;
      };

      try {
        await use();
      } finally {
        browser.newContext = original;
      }

      if (testInfo.status === testInfo.expectedStatus) return;

      const blank = await blankPages(contexts.flatMap((c) => c.pages()));
      if (problems.length === 0 && blank.length === 0) return;

      await testInfo.attach("load-diagnostics", {
        contentType: "text/plain",
        body: [
          blank.length
            ? `Pages with an empty #root: ${blank.join(", ")}`
            : "No open page had an empty #root.",
          "",
          ...problems.map((p) => `[${p.kind}] ${describe(p)}`),
        ].join("\n"),
      });

      const assetFailure = problems.find(
        (p) => p.kind === "request failed" || p.kind === "http error",
      );
      const uncaught = problems.find(
        (p) => p.kind === "page error" || p.kind === "console",
      );
      const cause = assetFailure ?? (blank.length ? uncaught : undefined);
      if (cause) {
        throw new Error(
          `frontend failed to load: ${describe(cause)}` +
            (blank.length ? ` — blank page at ${blank[0]}` : "") +
            " (see the load-diagnostics attachment)",
        );
      }
    },
    { auto: true },
  ],
});
