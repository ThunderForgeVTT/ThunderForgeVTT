import { expect, test, type Page } from "./fixtures/test";
import { graphql, uniqueSuffix } from "./fixtures/helpers";
import { serverTokenPosition, tokenPosition } from "./fixtures/offline";
import {
  addCombatant,
  advanceTurn,
  startCombat,
  type Combat,
} from "../playtest/combat";
import {
  closeTable,
  expectAgreed,
  must,
  openTable,
  placeCast,
  sitDown,
  tryDrag,
  type Seat,
} from "../playtest/table";

/**
 * Spec 046 tasks Phase 4 (plan phase 2): turn order holds.
 *
 * The independent test: on the ogre's turn, a player's move is refused with
 * "It is Ogre's turn" on every path a player's move takes — a drag on the
 * board, `moveOwnToken` asked directly, and a move queued offline and
 * replayed through `reconcileQueuedChanges` — and the token does not move on
 * any board. With the ogre's name hidden the sentence reads "Unknown". The
 * Game Master moves the ogre, and the hero, freely; a player's token that is
 * not in the fight is not held; and on the player's own turn the move lands.
 *
 * The offline path is exercised through the mutation the offline queue
 * replays into, with the command shape the queue stores, rather than by
 * taking a browser offline: what C1 claims is that the *server* refuses at
 * replay, and `offline-*` specs already prove the queue delivers.
 */

async function moveOwnTokenRefusal(
  page: Page,
  tokenId: string,
  x: number,
  y: number,
): Promise<string[]> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($tokenId: UUID!, $x: Float!, $y: Float!) {
        moveOwnToken(tokenId: $tokenId, x: $x, y: $y) {
          tokenId
        }
      }
    `,
    { tokenId, x, y },
  );
  return (result.errors ?? []).map((error) => error.message);
}

async function replayQueuedMove(
  page: Page,
  worldId: string,
  tokenId: string,
  at: { x: number; y: number },
): Promise<{
  applied: boolean;
  reason: string | null;
  refusal: string | null;
}> {
  const { reconcileQueuedChanges } = await must<{
    reconcileQueuedChanges: {
      applied: boolean;
      reason: string | null;
      refusal: string | null;
    }[];
  }>(
    page,
    `mutation ($worldId: UUID!, $changes: [QueuedChangeInput!]!) {
      reconcileQueuedChanges(worldId: $worldId, changes: $changes) {
        applied reason refusal
      }
    }`,
    {
      worldId,
      changes: [
        {
          localId: `turn-${uniqueSuffix()}`,
          command: {
            type: "upsert_token",
            token: { id: tokenId, x: at.x, y: at.y },
          },
        },
      ],
    },
  );
  return reconcileQueuedChanges[0];
}

function activeLabel(combat: Combat): string | undefined {
  return combat.combatants.find((c) => c.id === combat.activeCombatantId)
    ?.label;
}

test("a player cannot move on somebody else's turn, on any path", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(6 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria", "Brom"],
    sceneName: "The Ogre's Den",
  });
  const [aria, brom] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -192, y: 0 },
      seat: aria,
    });
    const ogre = await placeCast(table, {
      label: "Ogre",
      at: { x: 192, y: 128 },
      tokenType: "npc",
    });
    // Brom is at the table and not in this fight: exploring is not held.
    const wanderer = await placeCast(table, {
      label: "Brom",
      at: { x: -192, y: -192 },
      seat: brom,
    });

    let combat = await startCombat(table);
    combat = await addCombatant(table, combat.id, {
      label: "Ogre",
      actorId: ogre.actorId,
      tokenId: ogre.tokenId,
      initiative: 18,
      isNpc: true,
    });
    combat = await addCombatant(table, combat.id, {
      label: "Aria",
      actorId: hero.actorId,
      tokenId: hero.tokenId,
      initiative: 9,
    });
    combat = await advanceTurn(table, combat.id);
    expect(activeLabel(combat)).toBe("Ogre");

    for (const client of [table.gm, aria.page, brom.page]) {
      await sitDown(table, client);
    }
    const start = await expectAgreed(
      table,
      hero.tokenId,
      "everyone starts with Aria in the same place",
    );

    await test.step("a drag on the ogre's turn is refused and put back", async () => {
      await tryDrag(aria, hero.tokenId, { x: 128, y: 0 });
      await expect(
        aria.page.getByText("It is Ogre's turn").first(),
        "Aria is told whose turn it is",
      ).toBeVisible({ timeout: 10_000 });
      const settled = await expectAgreed(
        table,
        hero.tokenId,
        "every board and the server agree on Aria after the refusal",
      );
      expect(settled, "Aria's token has not moved on any board").toEqual(start);
    });

    await test.step("moveOwnToken asked directly is refused", async () => {
      const refusal = await moveOwnTokenRefusal(
        aria.page,
        hero.tokenId,
        start.x + 256,
        start.y,
      );
      expect(refusal).toContain("It is Ogre's turn");
      expect(
        await serverTokenPosition(table.gm, table.sceneId, hero.tokenId),
      ).toEqual(start);
    });

    await test.step("a queued offline move is refused at replay", async () => {
      const outcome = await replayQueuedMove(
        aria.page,
        table.worldId,
        hero.tokenId,
        { x: start.x + 256, y: start.y },
      );
      expect(outcome.applied).toBe(false);
      expect(outcome.reason).toBe("NOT_YOUR_TURN");
      expect(outcome.refusal).toBe("It is Ogre's turn");
      expect(
        await serverTokenPosition(table.gm, table.sceneId, hero.tokenId),
      ).toEqual(start);
    });

    await test.step("with the ogre's name hidden, it is Unknown's turn", async () => {
      await must(
        table.gm,
        `mutation ($tokenId: UUID!, $visible: Boolean!) {
          setTokenNameVisibility(tokenId: $tokenId, visible: $visible) { tokenId }
        }`,
        { tokenId: ogre.tokenId, visible: false },
      );
      const refusal = await moveOwnTokenRefusal(
        aria.page,
        hero.tokenId,
        start.x + 256,
        start.y,
      );
      expect(refusal).toContain("It is Unknown's turn");
      expect(refusal.join(" ")).not.toContain("Ogre");
      await must(
        table.gm,
        `mutation ($tokenId: UUID!, $visible: Boolean!) {
          setTokenNameVisibility(tokenId: $tokenId, visible: $visible) { tokenId }
        }`,
        { tokenId: ogre.tokenId, visible: true },
      );
    });

    await test.step("a player not in the fight explores freely", async () => {
      const moved = await tryDrag(brom, wanderer.tokenId, { x: 128, y: 0 });
      expect(moved, "Brom's token is not a combatant, so it moves").toBe(true);
      const at = await expectAgreed(
        table,
        wanderer.tokenId,
        "Brom's move reaches every board",
      );
      expect(at.x).not.toBe(-192);
    });

    await test.step("the Game Master moves freely on anyone's turn", async () => {
      const ogreFrom = await serverTokenPosition(
        table.gm,
        table.sceneId,
        ogre.tokenId,
      );
      const gm: Seat = { name: "Game Master", page: table.gm, userId: "" };
      expect(
        await tryDrag(gm, ogre.tokenId, { x: 0, y: -128 }),
        "the Game Master drags the ogre",
      ).toBe(true);
      await expect
        .poll(
          async () =>
            (await serverTokenPosition(table.gm, table.sceneId, ogre.tokenId))
              ?.y,
          { timeout: 10_000, message: "the ogre's move reaches the server" },
        )
        .not.toBe(ogreFrom?.y);
      expect(
        await tryDrag(gm, hero.tokenId, { x: 0, y: 128 }),
        "and Aria's token, on the ogre's turn",
      ).toBe(true);
      const heroNow = await expectAgreed(
        table,
        hero.tokenId,
        "the Game Master's move of Aria reaches every board",
      );
      expect(heroNow.y).not.toBe(start.y);
    });

    await test.step("on Aria's own turn, her move lands", async () => {
      combat = await advanceTurn(table, combat.id);
      expect(activeLabel(combat)).toBe("Aria");
      const before = await tokenPosition(aria.page, hero.tokenId);
      expect(
        await tryDrag(aria, hero.tokenId, { x: 128, y: 0 }),
        "Aria drags her token on her turn",
      ).toBe(true);
      const landed = await expectAgreed(
        table,
        hero.tokenId,
        "Aria's move on her own turn reaches every board",
      );
      expect(landed.x).not.toBe(before?.x);
    });
  } finally {
    await closeTable(table);
  }
});
