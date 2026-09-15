import { test, expect } from "./fixtures/test";
import { waitForEngineReady } from "./fixtures/helpers";
import {
  engineTokenIds,
  hiddenTokens,
  storeCounts,
} from "./fixtures/lightingProbe";
import {
  joinPlayer,
  openDarkDnd5eScene,
  placeCreature,
} from "./fixtures/visionTable";

/**
 * Owner decision 2026-09-15: the engine's demo tokens are the sandbox's.
 *
 * The engine used to spawn a red "player" square at (-180, 0) and a blue
 * "npc" at (180, 0) at startup, in every session. In a world they had no
 * server row, widened the darkness pass, and turned up in a player's
 * `hiddenTokens()` beside the real creatures. Now only `apps/engine-sandbox`
 * asks for them (`spawn_demo_tokens`; `scripts/demo-tokens-check.mjs` proves
 * the sandbox still has them). This proves the other half, asked of the
 * engine on both chairs: a world session draws the scene's tokens and nothing
 * else.
 */
test.describe("A world session's tokens (owner decision 2026-09-15)", () => {
  test("the engine draws only the scene's own tokens, for the Game Master and a player", async ({
    page,
    browser,
  }) => {
    test.setTimeout(240_000);

    const table = await openDarkDnd5eScene(page, "No Demo Tokens");
    const aria = await joinPlayer(browser, table);
    const hero = await placeCreature(table, {
      label: "Aria",
      x: 25,
      y: 25,
      ownerUserId: aria.userId,
    });
    const goblin = await placeCreature(table, {
      label: "Goblin",
      x: 225,
      y: 25,
    });
    const scene = [hero.tokenId, goblin.tokenId].sort();

    for (const seat of [table.gm, aria.page]) {
      await seat.goto(`/world/${table.worldId}/play`);
      await waitForEngineReady(seat);
      await expect
        .poll(() => storeCounts(seat), { timeout: 20_000 })
        .toMatchObject({ tokens: 2 });
      await expect
        .poll(() => engineTokenIds(seat), {
          timeout: 20_000,
          message: "the engine draws the scene's two tokens and no others",
        })
        .toEqual(scene);
    }

    // Held for a while, not sampled once: the demo tokens were spawned by a
    // startup system, and a late spawn is the failure worth catching.
    await table.gm.waitForTimeout(2_000);
    for (const seat of [table.gm, aria.page]) {
      const ids = await engineTokenIds(seat);
      expect(ids).not.toContain("player");
      expect(ids).not.toContain("npc");
      expect(ids).toEqual(scene);
    }

    // In the dark, what a player's board hides is only ever a real creature.
    const hidden = await hiddenTokens(aria.page);
    expect(hidden).not.toContain("player");
    expect(hidden).not.toContain("npc");
    for (const id of hidden) expect(scene).toContain(id);

    await aria.page.context().close();
  });
});
