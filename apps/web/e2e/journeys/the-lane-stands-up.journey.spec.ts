import { expect, test } from "@playwright/test";

/**
 * The journey lane's own smoke test (spec 051 T066).
 *
 * Proves the things every other journey assumes: the sign-in page renders,
 * the backend behind Vite's proxy is ready, and the page is not the dev stack
 * or an `e2e-parallel` shard, which own the only fixed web ports there are.
 */
test("the sign-in page is served by this run's own instance", async ({
  page,
}) => {
  await page.goto("/login");
  await expect(page.locator("#login-identifier")).toBeVisible({
    timeout: 30_000,
  });

  const ready = await page.request.get("/api/readyz");
  expect(ready.ok(), "the backend behind Vite must be ready").toBe(true);

  // Not the dev stack: the dev and e2e ports are nowhere in this URL.
  expect(new URL(page.url()).port).not.toMatch(/^(5173|52\d\d)$/);
});
