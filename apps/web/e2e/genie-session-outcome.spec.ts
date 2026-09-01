import { test, expect } from "@playwright/test";
import { registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 018 User Story 7, quickstart.md Scenario 9: a full session reaches
 * a definitive win or loss. Each outcome needs its own fresh world/session
 * (advanceDoomClock/advancePuzzleClock both reject once
 * session.status != "active" — see mutations_genie_session.rs), so this
 * is two independent tests rather than one shared session.
 *
 * Both tests use the session the world already has. Creating a Genie world
 * starts one (`graphql.rs`, `is_genie_world`, doomClockMax 6), and a world
 * may hold only one active session at a time — `startGenieSession` refuses a
 * second while one is live.
 *
 * These tests used to start their own with a smaller doomClockMax, to save
 * clicks. That quietly left the world holding *two* live sessions, and since
 * `genieSession(worldId)` returns the newest, concluding the test's session
 * resurfaced the auto-created one: the loss test filled its clock, the server
 * correctly recorded `lost`, and the page then showed an untouched "0 / 6"
 * from a session the test had never heard of. The saved clicks were not worth
 * the invariant, and the invariant is now enforced server-side.
 */
test.describe("Spec 018 Scenario 9: a Genie session reaches a definitive win or loss", () => {
  test("the Doom Clock filling ends the session in a loss", async ({ page }) => {
    test.setTimeout(60_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie Loss ${Date.now()}`, "e2eloss");

    await page.goto(`/world/${worldId}/staging`);
    const clocks = page.getByTestId("session-clocks");
    await expect(clocks).toBeVisible({ timeout: 15_000 });
    await expect(clocks).toContainText("0 / 6");

    // Filled one click at a time, the way a GM fills it.
    for (let segment = 1; segment <= 6; segment += 1) {
      await clocks.getByRole("button", { name: "Advance Doom Clock" }).click();
      await expect(clocks).toContainText(`${segment} / 6`, { timeout: 10_000 });
    }
    // genieSession(worldId) only ever returns the *active* session for a
    // world (queries/genie_session.rs filters status="active"), so a
    // concluded session reads back as null by design — the UI's own
    // "Session lost" text (driven by the same mutation response) is the
    // real verification here, not a post-hoc re-query.
    await expect(clocks).toContainText("Session lost", { timeout: 10_000 });
  });

  test("resolving every Puzzle Clock ends the session in a win", async ({ page }) => {
    test.setTimeout(60_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie Win ${Date.now()}`, "e2ewin");

    await page.goto(`/world/${worldId}/staging`);
    const clocks = page.getByTestId("session-clocks");
    await expect(clocks).toBeVisible({ timeout: 15_000 });

    await page.locator("#new-clock-label").fill("Escape the Vault");
    await page.locator("#new-clock-segments").fill("1");
    await clocks.getByRole("button", { name: "Create" }).click();
    await expect(clocks).toContainText("Escape the Vault", { timeout: 10_000 });

    await clocks.getByRole("button", { name: "Advance", exact: true }).click();
    await expect(clocks).toContainText("(Resolved)", { timeout: 10_000 });
    await expect(clocks).toContainText("Session won", { timeout: 10_000 });
  });
});
