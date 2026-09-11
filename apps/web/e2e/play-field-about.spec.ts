import { expect, test } from "./fixtures/demo-world";

/**
 * The play field is the one shell with no footer, and it does not therefore
 * get to be the one shell with no way to reach the copyright contact — or to
 * send feedback.
 *
 * Spec 039 FR-056 does not carve out an exception for people who are mid-game,
 * and spec 037 FR-001 wants feedback from any screen. Both used to float over
 * the map, where the tool rail and the dock painted over them (playtest
 * 2026-09-10 P4). They now have a dock button of their own, and the point of
 * this spec is that they are genuinely reachable there, not that a button
 * exists.
 */

test.describe("The play field's About & feedback section", () => {
  test("reaches the footer's legal links and the feedback form, without leaving the session", async ({
    page,
    demoWorld,
  }) => {
    await page.goto(`/world/${demoWorld.worldId}/play`);

    // Deliberately not waiting for the engine: a person looking for the
    // copyright contact should not have to wait for a canvas first.
    const tab = page.getByTestId("world-dock-tab-help");
    await expect(tab).toBeVisible({ timeout: 60_000 });

    // There is no footer here — that is the premise — and nothing floats over
    // the map any more.
    await expect(page.getByTestId("app-footer")).toHaveCount(0);
    await expect(page.getByTestId("play-field-about")).toHaveCount(0);
    await expect(page.getByTestId("feedback-launcher")).toHaveCount(0);

    await tab.click();
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

    // Feedback, from the same place, opens the same form the rest of the app
    // uses — without navigating away.
    const url = page.url();
    await page.getByTestId("play-field-feedback").click();
    await expect(page.getByRole("dialog")).toBeVisible();
    expect(page.url()).toBe(url);
  });
});
