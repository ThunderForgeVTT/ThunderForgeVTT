import { test, expect } from "@playwright/test";
import { graphql, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 019 (Scenario 6, previously unreachable): Wish Points scale on
 * level-up. Genie had no `level` concept anywhere in its data model
 * before this — added to `trait_data` (mirroring dnd5e's own `level`
 * field, which lives in the same place), with `resource_data.max_wish_points`
 * kept in sync via `calculateMaxWishPoints` on every level change.
 */
test.describe("Spec 019 Scenario 6: Wish Points scale on level-up", () => {
  test("changing a character's level recalculates their max Wish Points", async ({ page }) => {
    test.setTimeout(60_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie Leveling ${Date.now()}`, "e2elevel");

    const actor = await graphql<{ data: { createActor: { id: string } } }>(
      page,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Leveling Test Genie", isNpc: false, gameSystemId: "genie" } },
    );
    const actorId = actor.data.createActor.id;

    await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
    await expect(page.getByTestId("genie-actor-sheet")).toBeVisible({ timeout: 15_000 });
    await page.getByRole("tab", { name: "Resources" }).click();

    const levelInput = page.getByTestId("genie-level-input");
    await expect(levelInput).toHaveValue("1");
    const resourcesTab = page.getByTestId("genie-resources-tab");
    await expect(resourcesTab).toContainText("/ 2 max"); // level 1 -> 2 max wish points

    await levelInput.fill("5");
    // onLevelChange (ActorDetailPage.tsx's GenieActorSheet) fires two
    // sequential, un-awaited mutations (trait_data, then resource_data)
    // from a plain onChange handler — blur() only waits for the DOM
    // event to dispatch, not for either mutation to resolve. Reloading
    // immediately after would abort the second one mid-flight (same
    // class of bug as fixtures/helpers.ts's register() race found
    // earlier this session), so wait for both to actually settle first.
    await levelInput.blur();
    await page.waitForTimeout(1_500);

    // Persisted server-side, not just local state: reload and re-check.
    await page.reload();
    await expect(page.getByTestId("genie-actor-sheet")).toBeVisible({ timeout: 15_000 });
    await page.getByRole("tab", { name: "Resources" }).click();
    await expect(page.getByTestId("genie-level-input")).toHaveValue("5", { timeout: 10_000 });
    await expect(page.getByTestId("genie-resources-tab")).toContainText("/ 6 max"); // level 5 -> 6 max wish points

    // Verify server-side via the real actorSystemData query too.
    const systemData = await graphql<{
      data: { actorSystemData: { traitData: { level: number }; resourceData: { max_wish_points: number } } };
    }>(page, `query($actorId: UUID!) { actorSystemData(actorId: $actorId) { traitData resourceData } }`, {
      actorId,
    });
    expect(systemData.data.actorSystemData.traitData.level).toBe(5);
    expect(systemData.data.actorSystemData.resourceData.max_wish_points).toBe(6);
  });
});
