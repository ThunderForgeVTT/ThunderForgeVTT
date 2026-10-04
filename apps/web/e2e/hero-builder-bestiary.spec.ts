import { createHash } from "node:crypto";
import { expect, test, type Page } from "./fixtures/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import { must } from "../playtest/table";

/**
 * Spec 047, FR-030, FR-032 and FR-020: the bestiary, where a Game Master
 * makes monsters.
 *
 * The drawings were built and nothing in the app reached them. This proves
 * the join, against the server's answers rather than the dialog's word:
 *
 *  1. Three goblins asked for at once are three actors, numbered, each with
 *     both image roles stored and a face unlike the others'.
 *  2. An ogre's sheet says `large`, in the slot the 5e manifest declares for
 *     sizes, so the board gives its token four squares. A monster has no
 *     class and no level, and the 5e validator used to refuse a traits slot
 *     without them, which would have refused this write.
 *  3. Each of them can fight. The pack's stat block for the creature is
 *     written onto every actor made — hit points, armour class, challenge
 *     rating — and its attacks are abilities the actor knows, so a goblin
 *     made here is a goblin the combat flow can run, not a portrait with a
 *     blank sheet. Three goblins share one Scimitar ability between them.
 */

interface ImageRow {
  role: string;
  url: string;
}

type GqlResult<T> = { data?: T; errors?: { message: string }[] };

async function fifthEditionWorld(page: Page): Promise<string> {
  await register(page, freshCredentials("e2ebestiary"));
  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: `E2E Bestiary ${Date.now()}`, gameSystemId: "dnd5e" } },
  );
  const worldId = world.data?.createWorld?.id;
  if (!worldId) {
    throw new Error(
      `could not create a 5e world: ${JSON.stringify(world.errors ?? world)}`,
    );
  }
  return worldId;
}

test.describe("Bestiary (spec 047)", () => {
  test("three goblins with three faces, and an ogre whose sheet says large", async ({
    page,
  }) => {
    const worldId = await fifthEditionWorld(page);
    await page.goto(`/world/${worldId}/compendium?tab=npcs`);
    await expect(page.getByTestId("bestiary")).toBeVisible({ timeout: 15_000 });

    const dialog = page.getByTestId("bestiary-dialog");

    // Three goblins.
    await page.getByTestId("bestiary").click();
    await expect(dialog).toBeVisible({ timeout: 15_000 });
    await dialog.getByTestId("bestiary-creature").selectOption("goblin");
    await expect(dialog.getByTestId("bestiary-name")).toHaveValue("Goblin");
    await expect(dialog.getByTestId("bestiary-size")).toHaveText("small");
    await dialog.getByTestId("bestiary-count").fill("3");
    await expect(dialog.getByTestId("bestiary-stat-block")).toContainText(
      "AC 15, 10 hit points, CR 1/4",
    );
    await expect(
      dialog
        .getByTestId("bestiary-preview")
        .getByTestId("hero-preview-portrait"),
    ).toHaveCount(3);
    await dialog.getByTestId("bestiary-create").click();
    await expect(dialog).toBeHidden({ timeout: 45_000 });
    for (const label of ["Goblin 1", "Goblin 2", "Goblin 3"]) {
      await expect(page.getByTestId("npc-catalog-table")).toContainText(label);
    }

    // One ogre, named by the Game Master.
    await page.getByTestId("bestiary").click();
    await expect(dialog).toBeVisible({ timeout: 15_000 });
    await dialog.getByTestId("bestiary-creature").selectOption("ogre");
    await expect(dialog.getByTestId("bestiary-name")).toHaveValue("Ogre");
    await expect(dialog.getByTestId("bestiary-size")).toHaveText("large");
    await dialog.getByTestId("bestiary-name").fill("Grunk");
    await dialog.getByTestId("bestiary-create").click();
    await expect(dialog).toBeHidden({ timeout: 30_000 });
    await expect(page.getByTestId("npc-catalog-table")).toContainText("Grunk");

    const { worldActors } = await must<{
      worldActors: {
        id: string;
        label: string;
        isNpc: boolean;
        images: ImageRow[];
      }[];
    }>(
      page,
      `query ($worldId: UUID!) {
        worldActors(worldId: $worldId) { id label isNpc images { role url } }
      }`,
      { worldId },
    );
    const goblins = worldActors.filter((actor) =>
      actor.label.startsWith("Goblin "),
    );
    expect(goblins.map((actor) => actor.label).sort()).toEqual([
      "Goblin 1",
      "Goblin 2",
      "Goblin 3",
    ]);
    const faces = new Set<string>();
    for (const goblin of goblins) {
      expect(goblin.isNpc).toBe(true);
      expect(goblin.images.map((row) => row.role).sort()).toEqual([
        "portrait",
        "token",
      ]);
      const portrait = goblin.images.find((row) => row.role === "portrait")!;
      const response = await page.request.get(portrait.url);
      expect(response.status()).toBe(200);
      expect(response.headers()["content-type"]).toBe("image/webp");
      faces.add(
        createHash("sha256")
          .update(await response.body())
          .digest("hex"),
      );
    }
    expect(faces.size).toBe(3);

    const sheetOf = async (actorId: string) => {
      const { actorSystemData } = await must<{
        actorSystemData: {
          abilityData: Record<string, unknown> | null;
          resourceData: Record<string, unknown> | null;
          proficiencyData: Record<string, unknown> | null;
          traitData: Record<string, unknown> | null;
        } | null;
      }>(
        page,
        `query ($actorId: UUID!) {
          actorSystemData(actorId: $actorId) {
            abilityData resourceData proficiencyData traitData
          }
        }`,
        { actorId },
      );
      return actorSystemData;
    };
    const attacksOf = async (actorId: string) => {
      const { actorAbilities } = await must<{
        actorAbilities: { abilityId: string; abilityName: string }[];
      }>(
        page,
        `query ($actorId: UUID!) {
          actorAbilities(actorId: $actorId) { abilityId abilityName }
        }`,
        { actorId },
      );
      return actorAbilities;
    };
    const grunk = worldActors.find((actor) => actor.label === "Grunk")!;
    expect(grunk.images.map((row) => row.role).sort()).toEqual([
      "portrait",
      "token",
    ]);
    const ogre = await sheetOf(grunk.id);
    expect(ogre?.traitData?.size).toBe("large");
    expect(ogre?.traitData?.challenge).toBe("2");
    expect(ogre?.resourceData).toMatchObject({ max_hp: 68, current_hp: 68 });
    expect(ogre?.abilityData).toMatchObject({ armor_class: 11, strength: 19 });

    // Every goblin, not just the first: the same printed numbers on each.
    const scimitars = new Set<string>();
    for (const goblin of goblins) {
      const sheet = await sheetOf(goblin.id);
      expect(sheet?.traitData?.size, goblin.label).toBe("small");
      expect(sheet?.traitData?.challenge, goblin.label).toBe("1/4");
      // A monster still has no level: its proficiency bonus is its rating's.
      expect(sheet?.traitData, goblin.label).not.toHaveProperty("level");
      expect(sheet?.resourceData, goblin.label).toMatchObject({
        max_hp: 10,
        current_hp: 10,
      });
      expect(sheet?.abilityData, goblin.label).toMatchObject({
        armor_class: 15,
        dexterity: 15,
      });
      expect(sheet?.proficiencyData?.skill_expertise, goblin.label).toEqual([
        "stealth",
      ]);

      const attacks = await attacksOf(goblin.id);
      expect(
        attacks.map((attack) => attack.abilityName).sort(),
        goblin.label,
      ).toEqual(["Scimitar (Goblin Warrior)", "Shortbow (Goblin Warrior)"]);
      scimitars.add(
        attacks.find((attack) => attack.abilityName.startsWith("Scimitar"))!
          .abilityId,
      );
    }
    // One ability in the world, known by three goblins.
    expect(scimitars.size).toBe(1);
  });
});
