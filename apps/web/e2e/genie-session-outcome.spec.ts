import { test, expect } from "@playwright/test";
import { graphql, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 018 User Story 7, quickstart.md Scenario 9: a full session reaches
 * a definitive win or loss. Each outcome needs its own fresh world/session
 * (advanceDoomClock/advancePuzzleClock both reject once
 * session.status != "active" — see mutations_genie_session.rs), so this
 * is two independent tests rather than one shared session.
 *
 * Sessions are started via a direct GraphQL call with a small
 * doomClockMax/segmentsMax rather than clicking through
 * GenieSessionPanel's hardcoded "Start Genie session" (doomClockMax: 6)
 * button — driving a real Doom Clock to 6/6 through the UI one click at a
 * time works but adds nothing over asserting the same server-side
 * transition with a smaller number; the *reading* of the resulting
 * won/lost state below is still real UI end-to-end.
 */
test.describe("Spec 018 Scenario 9: a Genie session reaches a definitive win or loss", () => {
  test("the Doom Clock filling ends the session in a loss", async ({ page }) => {
    test.setTimeout(60_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie Loss ${Date.now()}`, "e2eloss");

    await graphql(
      page,
      `mutation($input: StartGenieSessionInput!) { startGenieSession(input: $input) { id } }`,
      { input: { worldId, doomClockMax: 2 } },
    );

    await page.goto(`/world/${worldId}/staging`);
    const clocks = page.getByTestId("session-clocks");
    await expect(clocks).toBeVisible({ timeout: 15_000 });
    await expect(clocks).toContainText("0 / 2");

    await clocks.getByRole("button", { name: "Advance Doom Clock" }).click();
    await expect(clocks).toContainText("1 / 2", { timeout: 10_000 });
    await clocks.getByRole("button", { name: "Advance Doom Clock" }).click();
    await expect(clocks).toContainText("2 / 2", { timeout: 10_000 });
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

    await graphql(
      page,
      `mutation($input: StartGenieSessionInput!) { startGenieSession(input: $input) { id } }`,
      { input: { worldId, doomClockMax: 6 } },
    );

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
