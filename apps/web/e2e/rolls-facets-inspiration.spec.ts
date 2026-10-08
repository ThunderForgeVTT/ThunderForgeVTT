import { expect, test, type Page } from "./fixtures/test";
import { graphql } from "./fixtures/helpers";
import { openChat } from "./fixtures/rolls";
import { claimFor, setAbilityScores } from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";

/**
 * Spec 084 US3 (FR-009 to FR-012, FR-017): a player spends their character's
 * Heroic Inspiration to reroll their own check from the chat.
 *
 * The server rolls the lowest d20 again and keeps the rest, so what is
 * asserted is what every chat shows: the first roll struck through, the new
 * one tagged and saying what was spent, the button only ever on the maker's
 * screen and gone once spent, and the sheet's Inspiration off.
 */

const SCORES = {
  strength: 10,
  dexterity: 16,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};

function dexterityRolls(page: Page) {
  return page.getByTestId("roll-entry").filter({
    has: page.getByTestId("roll-label").getByText(/^Dexterity:/),
  });
}

test("a player spends Heroic Inspiration to reroll their check, once, and every chat shows it", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(5 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Pip"],
  });
  const [pip] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Pip",
      at: { x: -192, y: 0 },
      seat: pip,
    });
    await setAbilityScores(table, hero.actorId, SCORES);
    await claimFor(table, pip, hero.actorId);
    // Only someone who may still act for the character may spend for it;
    // claiming does not yet make the player an Editor of it.
    await must(
      table.gm,
      `mutation ($input: SetActorPermissionInput!) {
        setActorPermission(input: $input) { actorId }
      }`,
      {
        input: { actorId: hero.actorId, userId: pip.userId, level: "EDITOR" },
      },
    );

    for (const client of [table.gm, pip.page]) {
      await sitDown(table, client);
      await openChat(client);
    }

    // 1. The GM grants Inspiration on the sheet.
    const sheetUrl = `/world/${table.worldId}/actor/${hero.actorId}`;
    const gmSheet = await table.gm.context().newPage();
    await gmSheet.goto(`${sheetUrl}/edit`);
    const inspiration = gmSheet.getByTestId("dnd5e-inspiration");
    await expect(inspiration).not.toBeChecked({ timeout: 30_000 });
    // The box is controlled: it ticks once the sheet write is confirmed.
    await inspiration.click();
    await expect(inspiration).toBeChecked({ timeout: 15_000 });
    await expect(inspiration).toBeEnabled({ timeout: 15_000 });

    // 2. The player rolls a check; only their chat offers the reroll.
    const sheetTab = await pip.page.context().newPage();
    await sheetTab.goto(`${sheetUrl}/view`);
    const checks = sheetTab.getByTestId("system-checks");
    await expect(checks).toBeVisible({ timeout: 30_000 });
    await checks.getByTestId("system-check-dexterity").click();
    await expect(checks.getByTestId("system-check-result")).toBeVisible({
      timeout: 15_000,
    });
    await sheetTab.close();
    // The play view can come up again behind the sheet tabs (the board
    // reloads), and its dock comes back closed; open the chats once more.
    for (const client of [table.gm, pip.page]) {
      await openChat(client);
    }

    const mine = dexterityRolls(pip.page).last();
    await expect(mine).toBeVisible({ timeout: 15_000 });
    const firstId = await mine.getAttribute("data-roll-id");
    expect(firstId).not.toBeNull();
    const reroll = mine.getByTestId("roll-reroll-inspiration");
    await expect(reroll).toHaveText("Reroll (Heroic Inspiration)", {
      timeout: 15_000,
    });
    const gmFirst = table.gm.locator(`[data-roll-id="${firstId}"]`);
    await expect(gmFirst).toBeVisible({ timeout: 15_000 });
    await expect(gmFirst.getByTestId("roll-reroll-inspiration")).toHaveCount(0);

    // 3. The player spends it.
    await reroll.click();

    // 4. Both chats strike the first and tag the new one.
    for (const client of [table.gm, pip.page]) {
      const first = client.locator(`[data-roll-id="${firstId}"]`);
      await expect(first).toHaveAttribute("data-rerolled", "true", {
        timeout: 15_000,
      });
      await expect(first.getByTestId("roll-rerolled")).toBeVisible();
      await expect(dexterityRolls(client)).toHaveCount(2, { timeout: 15_000 });
      const second = dexterityRolls(client).last();
      await expect(second).not.toHaveAttribute("data-roll-id", firstId!);
      await expect(second.getByTestId("roll-facet")).toHaveText([
        "Heroic Inspiration",
      ]);
      await expect(second.getByTestId("roll-spent")).toHaveText(
        "Rerolled with Heroic Inspiration",
      );
    }

    // 5. The sheet's Inspiration is spent.
    await gmSheet.reload();
    await expect(gmSheet.getByTestId("dnd5e-inspiration")).not.toBeChecked({
      timeout: 30_000,
    });
    await gmSheet.close();

    // 6. No button is left to press, on either roll.
    await expect(
      pip.page.getByTestId("roll-entry").getByTestId("roll-reroll-inspiration"),
    ).toHaveCount(0);

    // 7. The server refuses a second reroll of the same roll.
    const again = await graphql<{ errors?: { message: string }[] }>(
      pip.page,
      `
        mutation ($worldId: UUID!, $rollId: UUID!, $spend: String!) {
          rerollRoll(worldId: $worldId, rollId: $rollId, spend: $spend) {
            id
          }
        }
      `,
      { worldId: table.worldId, rollId: firstId, spend: "inspiration" },
    );
    expect(again.errors?.[0]?.message).toContain("already been rerolled");
  } finally {
    await closeTable(table);
  }
});
