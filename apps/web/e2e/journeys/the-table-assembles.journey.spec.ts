import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { uniqueSuffix, waitForWallsLoaded } from "../fixtures/helpers";
import { createNpcViaCompendium } from "../fixtures/content";
import { seatATable } from "../fixtures/journeyTable";
import {
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "../fixtures/offline";

/**
 * The arc nobody had end to end: two strangers and an empty account become a
 * table with a map under it, a character standing on that map, and enemies
 * placed against them.
 *
 * # Why this journey, when the parts are all covered already
 *
 * Every step below is proven somewhere. `scene-management.spec.ts` imports a
 * map, `canvas-context-menu.spec.ts` places a token, the combat slices fight
 * staged NPCs, and `journeyTable.ts` already seats a Game Master and a player
 * at one world. What none of them proves is that the steps compose **in the
 * order a real table does them, on one world, with one map and one bestiary**.
 * Each spec builds the world state it needs by the shortest route available to
 * it, and the shortest route is rarely the one a person takes — so a path can
 * be green in every part and still be broken as a path.
 *
 * Writing it the long way round found exactly that. The first draft imported
 * the map, un-hid the scene, and then had the table play on the *auto-created
 * starting scene*, because un-hiding is not launching: the fight happened on
 * an empty board while a perfectly good map sat one route away. Every
 * individual assertion still passed. That is the failure mode this lane
 * exists to catch, and the reason the launch below is its own step with the
 * walls asserted after it.
 *
 * # UI only, except to check
 *
 * The journey lane's rule (spec 051), and here it is the point rather than a
 * formality. `playtest/table.ts` can stage a token with one `createToken`
 * mutation, and that is right for a playtest measuring combat arithmetic. It
 * would be wrong here: the thing under test is the staging dialog a Game
 * Master actually opens — including the NPC picker that renders only once the
 * bestiary has something in it, and only for the scene's owner. A mutation
 * would skip precisely the seam this journey covers.
 *
 * The one read that is not a click is `window.__worldProbe.state()`, which is
 * how every engine-facing spec asks a client what it has drawn. Asking the
 * *player's* page is the whole assertion: the server agreeing with itself
 * proves nothing about what the friend can see.
 */

/**
 * A real map, not a fixture of one. `examples/maps` is dev-and-test only and
 * must never be seeded into a world or shipped — which is exactly what this
 * is: a test reading one.
 */
const DEMO_MAP = path.resolve(
  __dirname,
  "../../../../examples/maps/demo.dd2vtt",
);

/** What the Game Master stocks the bestiary with before anyone is invited. */
const BESTIARY = ["Goblin Cutthroat", "Cave Ogre"] as const;

/**
 * Somewhere to play and something to fight, built before a join link exists.
 *
 * Runs inside `seatATable`'s `beforeInvite` seam, so the ordering is the real
 * one: a Game Master does not invite a table to an empty world and then go
 * looking for a map. It stops short of launching, because launching brings
 * the party and at this point in the story there is no party — that is the
 * step that has to wait until someone has made a character.
 */
async function buildTheWorld(
  gm: Page,
  worldId: string,
): Promise<{ sceneName: string; npcIds: string[] }> {
  const sceneName = `The Sunken Chapel ${uniqueSuffix()}`;

  await test.step(`the Game Master makes a scene, "${sceneName}"`, async () => {
    await gm.getByTestId("world-nav-scenes").click();
    await gm.waitForURL(`**/world/${worldId}/scenes`, { timeout: 15_000 });
    await gm.getByTestId("new-scene-name-input").fill(sceneName);
    await gm.getByTestId("add-scene-button").click();
    await expect(gm.getByRole("link", { name: sceneName })).toBeVisible({
      timeout: 15_000,
    });
  });

  await test.step("and imports a map into it", async () => {
    await gm.getByRole("link", { name: sceneName }).click();
    await gm.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), {
      timeout: 15_000,
    });
    await gm.getByRole("button", { name: "Import map" }).click();
    await gm.setInputFiles('input[type="file"]', DEMO_MAP);
    // 120s, for the reason scene-management.spec.ts gives: a dd2vtt import is
    // a real upload plus wall, door and light extraction, and under load it
    // runs well past twenty seconds while still succeeding. A tighter budget
    // reports "import is broken" about a panel that was still working.
    await expect(gm.getByTestId("map-import-success")).toBeVisible({
      timeout: 120_000,
    });
  });

  await test.step("and reveals it, so the table may be taken there", async () => {
    // Scenes start hidden, and a hidden scene cannot be the one play happens
    // on. Revealing is necessary and — see the launch step — not sufficient.
    await gm.getByTestId("scene-hidden-toggle").click();
    await expect(gm.getByTestId("scene-hidden-toggle")).toBeChecked({
      timeout: 15_000,
    });
  });

  const npcIds: string[] = [];
  await test.step(`and stocks the bestiary with a ${BESTIARY.join(" and a ")}`, async () => {
    for (const name of BESTIARY) {
      npcIds.push(
        await createNpcViaCompendium(
          gm,
          worldId,
          `${name} ${uniqueSuffix()}`,
          `Stocked by the-table-assembles as a ${name}.`,
        ),
      );
    }
    expect(npcIds).toHaveLength(BESTIARY.length);
  });

  // Back to the world's own page, which is where `seatATable` resumes to open
  // the world to players and generate the join link.
  await gm.goto(`/world/${worldId}`);
  return { sceneName, npcIds };
}

/**
 * Move the table onto the imported map, bringing the player's character with
 * it.
 *
 * Launch is the only thing that moves the table (spec 031 FR-021), and
 * bringing the party is deliberately a property of that move rather than a
 * button of its own — so this is also how a player character first gets a
 * token anywhere. Nothing earlier in the story could have done it: the
 * character did not exist until the friend made one.
 */
async function launchWithTheParty(
  gm: Page,
  worldId: string,
  sceneName: string,
): Promise<void> {
  await gm.goto(`/world/${worldId}/scenes`);
  await gm.getByRole("link", { name: sceneName }).click();
  await gm.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), {
    timeout: 15_000,
  });

  const bringParty = gm.getByTestId("bring-party-toggle");
  await bringParty.check();
  await expect(bringParty).toBeChecked();

  await gm.getByTestId("launch-scene-button").click();
  // Launch enters play, so arriving at the play view is the confirmation.
  await gm.waitForURL(new RegExp(`/world/${worldId}/play`), {
    timeout: 30_000,
  });
}

/**
 * Stage one bestiary entry on the board through the Game Master's own dialog,
 * returning the token id the server answered with.
 *
 * The NPC picker renders only when the world has NPC actors and only for the
 * scene's owner, so reaching it at all is part of what this proves. The id
 * comes from the `createToken` response rather than a follow-up query,
 * because the click is what must have caused it.
 */
async function stageEnemy(gm: Page, npcId: string): Promise<string> {
  await gm.getByTestId("token-panel-toggle-button").click({ force: true });
  await gm.getByTestId("token-create-trigger").click({ force: true });

  const picker = gm.getByTestId("token-create-npc-select");
  await expect(
    picker,
    "the NPC picker must be offered once the bestiary has entries",
  ).toBeVisible({ timeout: 15_000 });
  await picker.selectOption(npcId);

  const [response] = await Promise.all([
    gm.waitForResponse(
      (resp) =>
        resp.url().includes("/api/graphql") &&
        (resp.request().postData() ?? "").includes("createToken"),
    ),
    gm.getByTestId("token-create-submit").click({ force: true }),
  ]);
  const body = (await response.json()) as {
    data?: { createToken?: { tokenId?: string } };
  };
  const tokenId = body.data?.createToken?.tokenId;
  if (!tokenId) {
    throw new Error(`staging ${npcId} returned no token id`);
  }

  // Out of the dialog and out of the panel, the way the other engine-facing
  // specs do it, so the next interaction reaches the canvas and not an
  // overlay that happens to be on top of it.
  await gm.keyboard.press("Escape");
  await gm.keyboard.press("Escape");
  return tokenId;
}

/** How many tokens this client has actually drawn. */
async function drawnTokens(page: Page): Promise<number> {
  return page.evaluate(() => window.__worldProbe?.state()?.tokens.length ?? 0);
}

test.describe("a table assembles: sign-up to enemies on the map", () => {
  test("two strangers build a world, a character, and a fight between them", async ({
    browser,
  }) => {
    // Two registrations, a world, a dd2vtt import, a bestiary, a launch and
    // two engine boots. The other journeys sit at 420-720s for less than
    // this, and the import alone is budgeted at 120.
    test.setTimeout(900_000);

    let built: { sceneName: string; npcIds: string[] } | undefined;

    // The signing-up, world-making, inviting, character-making and
    // arriving-on-the-playfield half of the arc is `seatATable`'s, shared
    // with three other journeys. Somewhere to play and something to fight go
    // in the middle of it, which is why the seam exists rather than a fourth
    // copy of register-create-invite-join.
    const table = await seatATable(browser, "Table Assembles", {
      beforeInvite: async (gm, worldId) => {
        built = await buildTheWorld(gm, worldId);
      },
    });

    const { gmPage: gm, playerPage: player, worldId } = table;
    if (!built) {
      throw new Error("beforeInvite did not run");
    }
    const { sceneName, npcIds } = built;

    await test.step("the Game Master takes the table to the map, party and all", async () => {
      await launchWithTheParty(gm, worldId, sceneName);

      for (const page of [gm, player]) {
        await waitForEngineReady(page);
      }

      // The map is the assertion, not the import message. A green
      // `map-import-success` only says the file was accepted; walls in the
      // engine store say the imported geometry reached this client — and
      // asking the *player* says the friend is looking at the Game Master's
      // map rather than at whatever scene they happened to land on.
      await waitForWallsLoaded(player);

      // Two clients arriving on one scene produce exactly the refetch storm
      // `waitForTokenTrafficToSettle` documents. Staging into that storm is
      // staging onto a board still being rewritten.
      for (const page of [gm, player]) {
        await waitForTokenTrafficToSettle(page);
      }
    });

    await test.step("the character the player made for themselves is on it", async () => {
      // Bringing the party is what created this token, and nothing before
      // this point in the story could have: the character did not exist
      // until the friend registered and made one.
      await expect
        .poll(() => drawnTokens(player), {
          message: "the party should have arrived on the launched scene",
          timeout: 60_000,
        })
        .toBeGreaterThanOrEqual(1);
    });

    const partyTokens = await drawnTokens(player);
    const staged: string[] = [];

    await test.step("the Game Master stages the enemies against them", async () => {
      for (const npcId of npcIds) {
        staged.push(await stageEnemy(gm, npcId));
      }
      expect(new Set(staged).size, "each staging is its own token").toBe(
        npcIds.length,
      );
    });

    await test.step("and the player's own client draws every one of them", async () => {
      await waitForTokenTrafficToSettle(player);

      // The assertion that makes the whole arc worth running: not that the
      // server holds the enemies, but that the stranger from the top of this
      // test can see them, on the Game Master's map, beside their own
      // character.
      for (const tokenId of staged) {
        await expect
          .poll(() => tokenPosition(player, tokenId), {
            message: `the player's client must draw staged token ${tokenId}`,
            timeout: 60_000,
          })
          .not.toBeNull();
      }

      expect(
        await drawnTokens(player),
        "the player's board holds their character and every staged enemy",
      ).toBeGreaterThanOrEqual(partyTokens + staged.length);
    });

    await Promise.all(table.contexts.map((context) => context.close()));
  });
});
