import { test, expect } from "./fixtures/demo-world";

/**
 * Spec 018 (Genie) User Story 7, quickstart.md Scenarios 8-9: the GM
 * session loop — Session Wish Pool and Doom/Puzzle Clocks. Previously
 * blocked (tasks.md T059/T063): the backend
 * (`src/server/src/graphql/{queries/genie_session,mutations_genie_session}.rs`)
 * was fully implemented and tested, but nothing in apps/web ever called
 * it — see `GenieSessionPanel`/`useGenieSession`, added to close that gap.
 *
 * The demo-world fixture's seeded user is not the world's GM (worlds have
 * no `world_members` owner row — see `create_world_impl`'s NOTE — so
 * ownership falls back to `worlds.created_by`, which IS this seeded user,
 * confirmed by `useWorldRole`'s created_by fallback), so this spec runs
 * with full GM controls.
 */
test.describe("Spec 018 Scenarios 8-9: the Genie GM session loop", () => {
  test("a GM starts a session, spends a wish, and advances the doom clock", async ({
    page,
    demoWorld,
  }) => {
    test.setTimeout(60_000);

    await page.goto(`/world/${demoWorld.worldId}/staging`);

    const wrapper = page.getByTestId("genie-session-panel-wrapper");
    await expect(wrapper).toBeVisible({ timeout: 15_000 });

    // The wrapper mounts before `genieSession(worldId)` answers, so at this
    // point neither the panel nor the start button exists yet. Asking
    // `isVisible()` straight away therefore reported "no start button",
    // skipped the click, and then waited out the full timeout for a panel
    // that was never going to render — a seeded world with no session shows
    // the start button, and something has to press it.
    //
    // The two are the query's only resting states, so wait for whichever
    // arrives before deciding.
    const panel = page.getByTestId("genie-session-panel");
    const startButton = page.getByTestId("start-genie-session-button");
    await expect(panel.or(startButton).first()).toBeVisible({ timeout: 15_000 });
    if (await startButton.isVisible().catch(() => false)) {
      await startButton.click();
    }

    await expect(panel).toBeVisible({ timeout: 15_000 });

    const wishPool = page.getByTestId("session-wish-pool");
    await expect(wishPool).toContainText("3 / 3 remaining");

    await wishPool.locator("#wish-narrative-effect").fill("The lock springs open on its own.");
    await wishPool.getByRole("button", { name: "Spend a Wish" }).click();
    await expect(wishPool).toContainText("2 / 3 remaining", { timeout: 10_000 });

    const clocks = page.getByTestId("session-clocks");
    await expect(clocks).toContainText("0 / 6");
    await clocks.getByRole("button", { name: "Advance Doom Clock" }).click();
    await expect(clocks).toContainText("1 / 6", { timeout: 10_000 });
  });
});
