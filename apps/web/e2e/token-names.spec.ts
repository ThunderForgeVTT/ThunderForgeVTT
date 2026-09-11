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
 * Playtest 2026-09-10 P7: token names above tokens.
 *
 * Every token shows its name above it, following it as it moves; the Game
 * Master can hide any token's name from players and always sees every name
 * themselves, dimmed where the table cannot.
 *
 * "Hidden" means the server never sends it: a player's canvas has nothing to
 * draw, and neither their token list nor the combat tracker carries it. So
 * this watches what reaches a player — the engine's nameplates and the
 * response bodies — not merely what is on screen. (The character list is a
 * separate change, decided with the owner: it is not asserted on here.)
 */

const NAME = "The Lich of Vael";

type Nameplate = { tokenId: string; text: string; dimmed: boolean };

async function nameplates(page: Page): Promise<Nameplate[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { nameplates?: () => Nameplate[] };
        }
      ).__engineProbe?.nameplates?.() ?? [],
  );
}

async function plateFor(page: Page, tokenId: string) {
  return (await nameplates(page)).find((p) => p.tokenId === tokenId) ?? null;
}

async function selectToken(page: Page, tokenId: string): Promise<void> {
  await page.evaluate(async (tokenId) => {
    const engine = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    engine
      .getBoundWorldStore()
      ?.dispatch({ type: "select_token", tokenId }, "ui");
  }, tokenId);
}

test.describe("Token names (playtest 2026-09-10 P7)", () => {
  test("a Game Master hides a name, and no player is sent it — on the map or in the tracker", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Token Names ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Named Scene");
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
    await launchSceneByName(page, worldId, "Named Scene");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    // An NPC, and a token for it with no name of its own: the name comes
    // from the character, which is where most tokens' names come from.
    const actor = await graphql<{ data?: { createActor?: { id: string } } }>(
      page,
      `
        mutation ($input: CreateActorInput!) {
          createActor(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, label: NAME, isNpc: true, gameSystemId: "genie" } },
    );
    const actorId = actor.data!.createActor!.id;
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
      { input: { sceneId, x: 0, y: 0, actorId, tokenType: "npc" } },
    );
    const tokenId = token.data!.createToken!.tokenId;

    // The Game Master sees the name, above the token.
    await expect
      .poll(() => plateFor(page, tokenId), { timeout: 15_000 })
      .toEqual({ tokenId, text: NAME, dimmed: false });

    // Hidden from players, through the Tokens panel.
    await selectToken(page, tokenId);
    await page.getByTestId("gm-tool-tokens").click();
    const toggle = page.getByTestId("token-tool-name-hidden");
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-pressed", "true");
    // Still the Game Master's to read — dimmed, so they can see who cannot.
    await expect
      .poll(() => plateFor(page, tokenId), { timeout: 15_000 })
      .toEqual({ tokenId, text: NAME, dimmed: true });

    // In combat, where a label would otherwise be copied in plain text.
    const combat = await graphql<{ data?: { startCombat?: { id: string } } }>(
      page,
      `
        mutation ($input: StartCombatInput!) {
          startCombat(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, sceneId } },
    );
    const combatId = combat.data!.startCombat!.id;
    const added = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($input: AddCombatantInput!) {
          addCombatant(input: $input) {
            id
          }
        }
      `,
      { input: { combatId, label: NAME, actorId, tokenId, isNpc: true } },
    );
    expect(added.errors).toBeUndefined();

    // A player, recording every token and combat answer they are sent.
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    const received: { at: number; body: string }[] = [];
    player.on("response", (response) => {
      if (!response.url().includes("/api/graphql")) return;
      void response
        .text()
        .then((body) => {
          if (body.includes('"tokens":') || body.includes('"activeCombat":')) {
            received.push({ at: Date.now(), body });
          }
        })
        .catch(() => {});
    });
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    await expect
      .poll(
        () =>
          player.evaluate(
            () => window.__worldProbe?.state()?.counts.tokens ?? 0,
          ),
        { timeout: 15_000 },
      )
      .toBeGreaterThan(0);
    // Long enough for a name that was going to arrive to have arrived.
    await player.waitForTimeout(1_500);
    expect(
      await plateFor(player, tokenId),
      "the player's canvas has no name to draw",
    ).toBeNull();

    const trackerForPlayer = await graphql<{
      data?: { activeCombat?: { combatants: { label: string }[] } };
    }>(
      player,
      `
        query ($worldId: UUID!) {
          activeCombat(worldId: $worldId) {
            combatants {
              label
            }
          }
        }
      `,
      { worldId },
    );
    expect(
      trackerForPlayer.data?.activeCombat?.combatants.map((c) => c.label),
    ).toEqual(["Unknown"]);

    const hiddenUntil = Date.now();
    expect(received.length, "the player was sent its tokens").toBeGreaterThan(
      0,
    );
    for (const { at, body } of received) {
      if (at > hiddenUntil) continue;
      expect(body, "a token or combat answer to a player").not.toContain(NAME);
    }

    // Shown again: the player's canvas follows without a reload.
    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await expect
      .poll(() => plateFor(player, tokenId), { timeout: 15_000 })
      .toEqual({ tokenId, text: NAME, dimmed: false });

    await player.context().close();
  });
});
