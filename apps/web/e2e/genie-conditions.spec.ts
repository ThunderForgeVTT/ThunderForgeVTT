import { test, expect } from "./fixtures/demo-world";
import { graphql } from "./fixtures/helpers";

/**
 * Spec 018 (Genie), quickstart.md Scenario 4: applying and clearing a
 * condition on a character, verified on the character sheet's Conditions
 * tab. Previously blocked (tasks.md T059/T063): `packs/systems/genie/web`'s
 * `ConditionTrack`/`CharacterSheet` were never mounted anywhere in
 * apps/web — see `ActorDetailPage.tsx`'s `GenieActorSheet`, added to close
 * that gap.
 */
test.describe("Spec 018 Scenario 4: applying and clearing a condition", () => {
  test("a GM toggles a condition on a Genie character and it persists across reload", async ({
    page,
    demoWorld,
  }) => {
    test.setTimeout(60_000);

    await page.goto(`/world/${demoWorld.worldId}/staging`);

    const createActor = await graphql<{
      data: { createActor: { id: string } };
    }>(
      page,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      {
        input: {
          worldId: demoWorld.worldId,
          label: "Condition Test Genie",
          isNpc: false,
          gameSystemId: "genie",
        },
      },
    );
    const actorId = createActor.data.createActor.id;

    await page.goto(`/world/${demoWorld.worldId}/actor/${actorId}/edit`);

    const sheet = page.getByTestId("genie-actor-sheet");
    await expect(sheet).toBeVisible({ timeout: 15_000 });

    const editor = page.getByTestId("genie-condition-editor");
    await expect(editor).toBeVisible();

    const boundCheckbox = editor.getByLabel("Bound");
    await expect(boundCheckbox).not.toBeChecked();
    // A plain `.click()` rather than `.check()`: this is a controlled
    // checkbox driven by an async mutate-then-refetch round trip, so it
    // stays unchecked until that resolves — `.check()`'s own built-in
    // "did the click take" verification is too tight a window for that,
    // unlike a normal `expect().toBeChecked()` poll below.
    await boundCheckbox.click();
    await expect(boundCheckbox).toBeChecked({ timeout: 10_000 });

    // Conditions tab on the sheet itself should reflect it too.
    await page.getByRole("tab", { name: "Conditions" }).click();
    await expect(page.getByTestId("genie-condition-track-sheet").getByText("Bound")).toBeVisible();

    await page.reload();
    await expect(page.getByTestId("genie-actor-sheet")).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId("genie-condition-editor").getByLabel("Bound")).toBeChecked();

    // Clear it.
    const boundAfterReload = page.getByTestId("genie-condition-editor").getByLabel("Bound");
    await boundAfterReload.click();
    await expect(boundAfterReload).not.toBeChecked({ timeout: 10_000 });
    await page.reload();
    await expect(page.getByTestId("genie-actor-sheet")).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId("genie-condition-editor").getByLabel("Bound")).not.toBeChecked();
  });
});
