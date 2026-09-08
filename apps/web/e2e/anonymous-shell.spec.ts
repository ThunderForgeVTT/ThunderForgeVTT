import { expect, test } from "@playwright/test";

/**
 * What a person who has never signed in is offered by the shell itself.
 *
 * # The two things this pins
 *
 * 1. **The menu offers only what works.** It used to offer the welcome hall,
 *    the world archive and system settings to everybody; all three are behind
 *    authentication, so a visitor got three doors that bounce to `/login` and
 *    one that opens. Nothing failed — every one of them "worked" in the sense
 *    that it navigated somewhere — which is why no test caught it and a person
 *    did.
 * 2. **The copyright contact is reachable without an account.** Spec 039
 *    FR-056 requires it, `/legal/dmca` has rendered it correctly for some
 *    time, and until now nothing in the product linked to that page. It was
 *    reachable by typing the URL, which is not reachable by the person the
 *    requirement is about.
 *
 * These are asserted signed out, in a context with no storage state, because
 * that is the only state in which either claim means anything.
 */

test.describe("The shell, to somebody who has never signed in", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("the menu offers the demo and nothing that would bounce them to a sign-in", async ({
    page,
  }) => {
    await page.goto("/login");
    await page.getByRole("button", { name: "Menu" }).click();

    await expect(page.getByRole("menuitem")).toHaveText([
      "Enter demo workspace",
    ]);
  });

  test("no name or membership is asserted for somebody who has none", async ({
    page,
  }) => {
    await page.goto("/login");
    // The header used to render "Archmage" and the role "Member" where a
    // signed-in person's own identity goes.
    const header = page.getByRole("banner");
    await expect(header).not.toContainText("Archmage");
    await expect(header).not.toContainText("Member");
  });

  test("the copyright contact is reachable from the footer, with no account", async ({
    page,
  }) => {
    await page.goto("/login");

    const legal = page.getByTestId("footer-legal-links");
    await expect(legal).toBeVisible();
    await expect(legal).toContainText("Terms of service");
    await expect(legal).toContainText("Privacy policy");

    // Named for what the person is trying to do rather than for the statute.
    await legal.getByRole("link", { name: "Copyright notices" }).click();
    await page.waitForURL(/\/legal\/dmca$/, { timeout: 20_000 });

    // Arrived, and rendered — not a route that resolves to an empty shell.
    // `.first()` because the legal pages nest their own `<main>` inside the
    // layout's, which is its own small oddity and not this test's subject.
    await expect(page.locator("main").first()).toContainText(/notice/i);
  });

  test("the status page carries the footer too, though it is served ahead of the router", async ({
    page,
  }) => {
    // `/status` is rendered before the router in `App.tsx`, deliberately, so
    // that it still answers when the setup service does not. That exemption is
    // about the data it needs and not about being a page without links — and
    // because it is reached by a full load rather than a route, a footer added
    // to the layout alone would silently miss it.
    await page.goto("/status");
    await expect(page.getByTestId("footer-legal-links")).toBeVisible({
      timeout: 20_000,
    });
    await expect(
      page.getByTestId("footer-legal-links").getByRole("link", {
        name: "Copyright notices",
      }),
    ).toBeVisible();
  });

  test("the legal pages are reachable from each other's footer too", async ({
    page,
  }) => {
    // The footer is in the shared layout, so a visitor who lands on one legal
    // page can reach the others. Asserted because a footer rendered only on
    // the signed-in shell would satisfy every test above and still strand the
    // person this is for.
    await page.goto("/legal/privacy");
    await expect(page.getByTestId("footer-legal-links")).toBeVisible();
    await page
      .getByTestId("footer-legal-links")
      .getByRole("link", { name: "Terms of service" })
      .click();
    await page.waitForURL(/\/legal\/terms$/, { timeout: 20_000 });
  });
});
