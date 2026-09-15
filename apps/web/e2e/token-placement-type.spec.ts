import { expect, test, type Page } from "./fixtures/test";
import { openDockTab } from "./fixtures/helpers";
import { closeTable, must, openTable, sitDown } from "../playtest/table";

/**
 * Playtest note (spec 046 research R4): a token placed for an NPC was typed
 * `character`, so it was drawn in a player character's colour and read as one
 * by everything that looks at a token's kind.
 *
 * `createToken` now takes the kind from the actor when the caller names none.
 * The server test `placement_defaults_for_character_npc_unique_npc_and_actorless`
 * proves that for the request; this proves it for the Place tool, the path a
 * Game Master actually uses: Place in the actors pane, a click on the map,
 * and whatever `usePlaceActorTokens` sends.
 */

/** Click at a pixel offset from the canvas centre, down and up a frame apart
 * (a zero-delay pair can collapse into a press with no release in Bevy). */
async function clickCanvasAt(page: Page, dx: number, dy: number) {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the canvas must be laid out before it is used");
  await page.mouse.move(
    box.x + box.width / 2 + dx,
    box.y + box.height / 2 + dy,
  );
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
}

test("the Place tool puts an NPC on the map as an npc token, and a character as a character", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(4 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: [],
    sceneName: "The Crossroads",
  });

  try {
    const create = async (label: string, isNpc: boolean) =>
      (
        await must<{ createActor: { id: string } }>(
          table.gm,
          `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
          {
            input: {
              worldId: table.worldId,
              label,
              isNpc,
              gameSystemId: table.system,
            },
          },
        )
      ).createActor.id;
    const goblin = await create("Snikt the Goblin", true);
    const hero = await create("Wren Ashdown", false);

    const kindOf = async (actorId: string) => {
      const { tokens } = await must<{
        tokens: { actorId: string | null; tokenType: string }[];
      }>(
        table.gm,
        `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { actorId tokenType } }`,
        { sceneId: table.sceneId },
      );
      return tokens
        .filter((token) => token.actorId === actorId)
        .map((token) => token.tokenType);
    };

    await sitDown(table, table.gm);
    await openDockTab(table.gm, "actors");

    for (const [actorId, offset, kind] of [
      [goblin, -160, "npc"],
      [hero, -60, "character"],
    ] as const) {
      await table.gm.getByTestId(`actor-place-${actorId}`).click();
      await clickCanvasAt(table.gm, offset, 0);
      await expect
        .poll(() => kindOf(actorId), {
          timeout: 15_000,
          message: `Place then a click puts one token for the actor, typed ${kind}`,
        })
        .toEqual([kind]);
    }
  } finally {
    await closeTable(table);
  }
});
