import { test, expect, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 045 US7: a player's map remembers where they have been, and a Game
 * Master can reset it.
 *
 * Run against a real browser rather than a mocked store, because the thing
 * under test *is* the browser: what a player explored lives in their own
 * IndexedDB, it has to survive a reload, and it has to be dropped when the
 * server's epoch says a Game Master reset it. None of that is observable with
 * a fake.
 *
 * The engine's own accumulation is covered by its unit tests. What this
 * proves is the part no unit test can reach — that the three parties agree:
 * the server holds the epoch, the browser holds the cells, and the engine
 * decides what was seen.
 */

/** What this browser has stored for the signed-in player, across all scenes. */
async function storedCells(page: Page): Promise<number> {
  return page.evaluate(async () => {
    const open = () =>
      new Promise<IDBDatabase | null>((resolve) => {
        const request = indexedDB.open("thunderforge-exploration", 1);
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => resolve(null);
        request.onupgradeneeded = () => {
          // Opening at the same version the application uses means this can
          // create the store if the application has not yet — which would
          // otherwise read as "nothing stored" on a timing difference.
          const db = request.result;
          if (!db.objectStoreNames.contains("areas")) {
            db.createObjectStore("areas");
          }
        };
      });
    const db = await open();
    if (!db) return 0;
    return new Promise<number>((resolve) => {
      const tx = db.transaction("areas", "readonly");
      const all = tx.objectStore("areas").getAll();
      all.onsuccess = () => {
        const records = all.result as { cells?: unknown[] }[];
        resolve(
          records.reduce(
            (total, record) => total + (record.cells?.length ?? 0),
            0,
          ),
        );
      };
      all.onerror = () => resolve(0);
    });
  });
}

test.describe("Explored areas (spec 045 US7)", () => {
  test("a player's map remembers where they walked, survives a reload, and a Game Master can reset it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Exploration ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Dark Crypt");
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneHidden(sceneId: $sceneId, hidden: false) {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    await launchSceneByName(page, worldId, "Dark Crypt");

    // Off until a Game Master says otherwise (FR-070). Asserted before
    // turning it on, so "it remembers" cannot pass on a default.
    const before = await graphql<{
      data?: { sceneExploration?: { enabled: boolean; mine: number } };
    }>(
      page,
      `
        query ($sceneId: UUID!) {
          sceneExploration(sceneId: $sceneId) {
            enabled
            mine
          }
        }
      `,
      { sceneId },
    );
    expect(before.data?.sceneExploration?.enabled).toBe(false);
    expect(before.data?.sceneExploration?.mine).toBe(0);

    const enable = await graphql<{ errors?: { message: string }[] }>(
      page,
      `
        mutation ($sceneId: UUID!) {
          setSceneExploration(sceneId: $sceneId, enabled: true)
        }
      `,
      { sceneId },
    );
    expect(enable.errors).toBeUndefined();

    // A player, with a token of their own to see through — the fog is
    // accumulated from a player's own token and from nothing else.
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    const me = await graphql<{ data?: { me?: { id: string } } }>(
      player,
      `
        query {
          me {
            id
          }
        }
      `,
      {},
    );
    const playerUserId = me.data!.me!.id;

    const token = await graphql<{
      data?: { createToken?: { tokenId: string } };
    }>(
      page,
      `
        mutation ($input: GraphQLCreateTokenInput!) {
          createToken(input: $input) {
            tokenId
          }
        }
      `,
      { input: { sceneId, x: 0, y: 0, tokenType: "character" } },
    );
    const tokenId = token.data!.createToken!.tokenId;
    await graphql(
      page,
      `
        mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
          updateToken(tokenId: $tokenId, input: $input) {
            tokenId
          }
        }
      `,
      { tokenId, input: { ownerUserId: playerUserId, isPrimary: true } },
    );

    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);

    // The save runs on a ten-second timer, so this waits rather than polling
    // in a tight loop: a shorter timer would be a worse product to make a
    // faster test.
    await expect
      .poll(() => storedCells(player), {
        timeout: 45_000,
        intervals: [2_000],
        message: "the player's browser keeps what their token has seen",
      })
      .toBeGreaterThan(0);
    const explored = await storedCells(player);

    // It survives a reload. Without this the fog is a session's memory, not a
    // map — and a map is what FR-071 asks for.
    await player.reload();
    await waitForEngineReady(player);
    expect(
      await storedCells(player),
      "what was explored is still there after a reload",
    ).toBeGreaterThan(0);

    // The Game Master resets it for everyone.
    const reset = await graphql<{
      data?: { resetSceneExploration?: number };
      errors?: { message: string }[];
    }>(
      page,
      `
        mutation ($sceneId: UUID!) {
          resetSceneExploration(sceneId: $sceneId)
        }
      `,
      { sceneId },
    );
    expect(reset.errors).toBeUndefined();
    expect(
      reset.data?.resetSceneExploration,
      "a reset moves the epoch past what any browser is holding",
    ).toBeGreaterThan(0);

    // And the player's stored map goes, without the player doing anything.
    // Their surroundings come back — they are still standing there and can
    // still see — so this asserts the map shrank rather than that it emptied.
    await expect
      .poll(() => storedCells(player), {
        timeout: 45_000,
        intervals: [2_000],
        message:
          "the Game Master's reset reaches the player's own browser (FR-078)",
      })
      .toBeLessThan(explored);

    await player.context().close();
  });

  test("a player may not turn exploration on, or reset it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Exploration Auth ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Locked Room");
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);

    const turnOn = await graphql<{ errors?: { message: string }[] }>(
      player,
      `
        mutation ($sceneId: UUID!) {
          setSceneExploration(sceneId: $sceneId, enabled: true)
        }
      `,
      { sceneId },
    );
    expect(
      turnOn.errors?.length,
      "a player cannot decide what the table remembers",
    ).toBeGreaterThan(0);

    const reset = await graphql<{ errors?: { message: string }[] }>(
      player,
      `
        mutation ($sceneId: UUID!) {
          resetSceneExploration(sceneId: $sceneId)
        }
      `,
      { sceneId },
    );
    expect(
      reset.errors?.length,
      "nor wipe anybody's map, including their own neighbours'",
    ).toBeGreaterThan(0);

    await player.context().close();
  });
});
