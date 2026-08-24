import { test, expect } from "@playwright/test";
import {
  clickPlay,
  ensureSidebarOpen,
  graphql,
  registerAndCreateWorld,
  waitForEngineReady,
} from "./fixtures/helpers";

/**
 * Spec 018 (Genie), quickstart.md Scenario 7 / tasks.md T059: a complete
 * combat encounter using only the Genie system pack, combining the four
 * legs already individually verified elsewhere into one spec: staging an
 * NPC (Scenario 3), a Manifestation roll (Scenario 1), a scene-topology
 * switch (Scenario 2), and a condition applied and cleared (Scenario 4).
 */
test.describe("Spec 018 Scenario 7: a full combat encounter using only Genie content", () => {
  test("stage an NPC, roll a Manifestation check, switch scene topology, and apply/clear a condition", async ({
    page,
  }) => {
    test.setTimeout(90_000);
    const worldName = `E2E Genie Encounter ${Date.now()}`;
    const worldId = await registerAndCreateWorld(page, worldName, "e2eenc");

    // --- Scenario 3 leg: stage an NPC ---
    const npc = await graphql<{ data: { createActor: { id: string } } }>(
      page,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Encounter NPC", isNpc: true, gameSystemId: "genie" } },
    );
    const npcId = npc.data.createActor.id;
    await graphql(
      page,
      `mutation($input: GraphQLUpdateActorSystemDataInput!) { updateActorSystemData(input: $input) { id } }`,
      { input: { actorId: npcId, gameSystemId: "genie", dataType: "trait_data", data: { size_category: "large" } } },
    );

    // --- Scenario 4 leg (part 1): a PC to apply/clear a condition on ---
    const pc = await graphql<{ data: { createActor: { id: string } } }>(
      page,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Encounter PC", isNpc: false, gameSystemId: "genie" } },
    );
    const pcId = pc.data.createActor.id;

    await page.goto(`/world/${worldId}/actor/${pcId}/edit`);
    const editor = page.getByTestId("genie-condition-editor");
    await expect(editor).toBeVisible({ timeout: 15_000 });
    const boundCheckbox = editor.getByLabel("Bound");
    await boundCheckbox.click();
    await expect(boundCheckbox).toBeChecked({ timeout: 10_000 });
    await boundCheckbox.click();
    await expect(boundCheckbox).not.toBeChecked({ timeout: 10_000 });

    // --- Scenario 1 leg: a Manifestation roll ---
    await page.goto(`/world/${worldId}/staging`);
    await clickPlay(page);
    await waitForEngineReady(page);

    const dicePanel = page.getByTestId("dice-roller-panel");
    await expect(dicePanel).toBeVisible({ timeout: 15_000 });
    await page.getByTestId("dice-formula-input").fill("4d6kh3x=6cs>=4");
    await page.getByTestId("dice-roll-button").click();
    await expect(page.getByTestId("dice-roll-result")).toBeVisible({ timeout: 10_000 });

    // --- Scenario 2 leg: a scene-topology switch ---
    const createGridless = await graphql<{
      data: { createScene: { sceneId: string; name: string } };
    }>(
      page,
      `mutation($input: GraphQLCreateSceneInput!) { createScene(input: $input) { sceneId gridType name } }`,
      { input: { worldId, name: "Wish-Warped Encounter Zone", gridType: "gridless" } },
    );
    const gridlessScene = createGridless.data.createScene;

    await page.reload();
    await waitForEngineReady(page);
    await ensureSidebarOpen(page);
    await page.getByTestId("scene-switcher").click();
    await page.getByRole("option", { name: gridlessScene.name }).click();
    await expect(page.getByTestId("scene-switcher")).toContainText(gridlessScene.name, {
      timeout: 10_000,
    });
  });
});
