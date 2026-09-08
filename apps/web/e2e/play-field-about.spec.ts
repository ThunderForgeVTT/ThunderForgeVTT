import { expect, test } from "./fixtures/demo-world";

/**
 * The play field is the one shell with no footer, and it does not therefore
 * get to be the one shell with no way to reach the copyright contact.
 *
 * Spec 039 FR-056 does not carve out an exception for people who are mid-game.
 * The canvas fills the viewport, so the links live behind a small "About"
 * control in the corner instead — and the point of this spec is that they are
 * genuinely still reachable, not that a button exists.
 */

test.describe("The play field's About control", () => {
  test("reaches the same legal links the footer carries, without leaving the session", async ({
    page,
    demoWorld,
  }) => {
    await page.goto(`/world/${demoWorld.worldId}/play`);

    // Deliberately not waiting for the engine: the point of this control is
    // that it is available on the play field, and a person looking for the
    // copyright contact should not have to wait for a canvas first.
    const about = page.getByTestId("play-field-about");
    await expect(about).toBeVisible({ timeout: 60_000 });

    // There is no footer here — that is the premise, and if one appeared this
    // control would be redundant rather than necessary.
    await expect(page.getByTestId("app-footer")).toHaveCount(0);

    await about.click();
    const links = page.getByTestId("play-field-about-links");
    await expect(links).toBeVisible();
    await expect(links).toContainText("Copyright notices");
    await expect(links).toContainText("Terms of service");
    await expect(links).toContainText("Privacy policy");

    // Every link opens in a new tab. Following one in place would drop a
    // player out of a live session to read a legal page, and the way back is a
    // full reload of the engine.
    for (const name of ["Copyright notices", "Terms of service"]) {
      await expect(links.getByRole("link", { name })).toHaveAttribute(
        "target",
        "_blank",
      );
    }

    // And the demo invitation is not offered to somebody already at a table.
    await expect(links).not.toContainText("Enter demo workspace");
  });
});
