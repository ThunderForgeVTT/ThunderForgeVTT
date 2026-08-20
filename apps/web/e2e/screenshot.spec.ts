import { test } from "@playwright/test";

const routes = ["/", "/login", "/register"];

for (const route of routes) {
  test(`screenshot ${route}`, async ({ page }) => {
    await page.goto(route);
    await page.waitForLoadState("networkidle");
    await page.screenshot({
      path: `e2e/screenshots${route === "/" ? "/home" : route}.png`,
      fullPage: true,
    });
  });
}
