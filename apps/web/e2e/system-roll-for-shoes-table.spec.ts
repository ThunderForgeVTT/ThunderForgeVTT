import type { Page } from "@playwright/test";

import {
  graphql,
  inviteAndJoinAsPlayer,
  openDockTab,
  registerAndCreateWorld,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * Roll for Shoes at a table: a Game Master and a player, two seats, no map.
 *
 * The other two Roll for Shoes specs prove the rules, mostly from one seat.
 * This one proves the things that only exist *between* seats, which is where
 * a theatre-of-the-mind game is actually played:
 *
 *   - the Game Master says what has to be beaten, once, and the player's
 *     sheet shows it and uses it — the player neither types it nor can change
 *     it, on the sheet or behind it;
 *   - the player's roll is said where the Game Master reads;
 *   - taking the difficulty away hands each sheet back its own entry;
 *   - a skill the Game Master writes on by hand appears on the sheet the
 *     player already has open;
 *   - the player keeps their own character up to date from the dock, which
 *     for a game with no map is the whole of their seat;
 *   - an advancement nobody has answered is never rolled over.
 *
 * Nobody places a token and nobody opens a map. The world has only the scene
 * it was created with, and nothing below asks for more.
 *
 * As everywhere in these specs, no roll is forced (there is no dice seed). A
 * single d6 can never beat 6, so the roll below always fails and always pays
 * its 1 XP — and 1 XP always buys the die into a six, which is how the
 * advancement is reached on every run rather than one run in six.
 */

const STARTING_SKILL_ID = "starting-skill";

/** A Roll for Shoes world with statuses on, and one character to claim. */
async function setTheTable(page: Page): Promise<{
  worldId: string;
  actorId: string;
}> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Roll for Shoes Table ${Date.now()}`,
    "e2erfst",
  );

  // Statuses are an Extra, off unless the world asks. The mutation is a
  // whole-row write, so everything else is stated as the core game has it.
  const settings = await graphql<{
    data?: { updateRollForShoesWorldSettings?: { statusesEnabled: boolean } };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: UpdateRollForShoesWorldSettingsInput!) {
        updateRollForShoesWorldSettings(input: $input) {
          statusesEnabled
        }
      }
    `,
    {
      input: {
        worldId,
        difficultyMode: "free",
        tieSucceeds: false,
        statusesEnabled: true,
        skillSlotsEnabled: false,
        startingSkills: [],
      },
    },
  );
  expect(
    settings.data?.updateRollForShoesWorldSettings?.statusesEnabled,
    `settings refused: ${JSON.stringify(settings.errors ?? settings)}`,
  ).toBe(true);

  const actor = await graphql<{ data: { createActor: { id: string } } }>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        worldId,
        label: "Barefoot",
        isNpc: false,
        gameSystemId: "roll_for_shoes",
      },
    },
  );
  const actorId = actor.data.createActor.id;

  await graphql(
    page,
    `
      mutation ($actorId: UUID!) {
        setActorAvailability(actorId: $actorId, available: true) {
          id
        }
      }
    `,
    { actorId },
  );

  return { worldId, actorId };
}

/** The player's own character, in the dock, beside whatever the table shows. */
async function openOwnSheet(player: Page, actorId: string): Promise<void> {
  await openDockTab(player, "actors");
  await player.getByTestId(`actor-view-${actorId}`).click();
  await expect(player.getByTestId("rfs-sheet")).toBeVisible({
    timeout: 15_000,
  });
}

test("a Game Master and a player at one table, with no map between them", async ({
  page,
  browser,
}) => {
  test.setTimeout(300_000);
  const { worldId, actorId } = await setTheTable(page);

  const player = await inviteAndJoinAsPlayer(
    browser,
    page,
    worldId,
    "e2erfstp",
  );
  const claim = await graphql<{
    data?: { claimActor?: { actorId: string } };
    errors?: { message: string }[];
  }>(
    player,
    `
      mutation ($worldId: UUID!, $actorId: UUID!) {
        claimActor(worldId: $worldId, actorId: $actorId) {
          actorId
        }
      }
    `,
    { worldId, actorId },
  );
  expect(
    claim.data?.claimActor?.actorId,
    `claim refused: ${JSON.stringify(claim.errors ?? claim)}`,
  ).toBe(actorId);

  await player.goto(`/world/${worldId}/play`);
  await openOwnSheet(player, actorId);

  await test.step("with nothing set, the player's sheet has its own entry", async () => {
    await expect(player.getByTestId("rfs-opposition")).toBeVisible({
      timeout: 15_000,
    });
    await expect(player.getByTestId("rfs-table-target")).toHaveCount(0);
  });

  // The Game Master's seat. They have no sheet of their own open and no token
  // to click; the control is in the play dock, where they already are.
  await page.goto(`/world/${worldId}/play`);
  await openDockTab(page, "clocks");
  const gmTable = page.getByTestId("rfs-table-panel");
  await expect(gmTable).toBeVisible({ timeout: 20_000 });

  await test.step("the Game Master says what has to be beaten", async () => {
    await expect(gmTable.getByTestId("rfs-table-none")).toBeVisible({
      timeout: 15_000,
    });
    await gmTable.getByTestId("rfs-table-number").fill("6");
    await gmTable.getByTestId("rfs-table-set").click();
    await expect(gmTable.getByTestId("rfs-table-target")).toHaveText("6", {
      timeout: 15_000,
    });
    await expect(gmTable.getByTestId("rfs-table-refusal")).toHaveCount(0);
  });

  await test.step("the player's open sheet shows it, and offers nothing to change it with", async () => {
    // No reload: the sheet was open before the number was set, and learns of
    // it from the world's events.
    await expect(player.getByTestId("rfs-table-target")).toHaveText("6", {
      timeout: 20_000,
    });
    await expect(player.getByTestId("rfs-opposition")).toHaveCount(0);
    await expect(player.getByTestId("rfs-table-number")).toHaveCount(0);
    await expect(player.getByTestId("rfs-table-set")).toHaveCount(0);
    await expect(player.getByTestId("rfs-table-clear")).toHaveCount(0);

    // And not behind the sheet either. What a sheet offers is a courtesy; the
    // server's refusal is the rule.
    const attempt = await graphql<{
      data?: { setRollForShoesTableDifficulty?: { target: number | null } };
      errors?: { message: string }[];
    }>(
      player,
      `
        mutation ($input: SetRollForShoesTableDifficultyInput!) {
          setRollForShoesTableDifficulty(input: $input) {
            target
          }
        }
      `,
      { input: { worldId, target: 1 } },
    );
    expect(
      attempt.errors?.length ?? 0,
      "a player setting the table's difficulty must be refused",
    ).toBeGreaterThan(0);
    await expect(gmTable.getByTestId("rfs-table-target")).toHaveText("6");
  });

  await test.step("the player rolls against it, and the Game Master reads the roll", async () => {
    await openDockTab(page, "chat");
    await expect(page.getByTestId("chat-panel")).toBeVisible({
      timeout: 15_000,
    });

    await player.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
    await expect(player.getByTestId("rfs-total")).toBeVisible({
      timeout: 15_000,
    });
    // One d6 cannot beat the Game Master's 6 — the player typed nothing, so
    // if this reads as anything but a failure the sheet used another number.
    await expect(player.getByTestId("rfs-result")).toContainText(/fail/i);
    await expect(player.getByTestId("rfs-error")).toHaveCount(0);
    await expect(player.getByTestId("rfs-xp")).toHaveText("1", {
      timeout: 15_000,
    });

    await expect(
      page.getByTestId("chat-message").filter({
        hasText: /Barefoot rolls Do Anything \(1d6\): \d against 6 — fails/,
      }),
    ).toHaveCount(1, { timeout: 20_000 });
  });

  await test.step("an advancement nobody has answered holds the next roll", async () => {
    // The die may already have shown a six. If not, the XP the failure just
    // paid buys it into one — either way every die now shows a six.
    const advancement = player.getByTestId("rfs-advancement");
    if (!(await advancement.isVisible())) {
      await player.getByTestId("rfs-spend-xp").click();
    }
    await expect(advancement).toBeVisible({ timeout: 15_000 });

    // A roll replaces the last attempt, and the attempt is where the owed
    // skill lives. So the sheet says what is owed and will not roll over it.
    await expect(player.getByTestId("rfs-advancement-owed")).toBeVisible();
    await expect(
      player.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`),
    ).toBeDisabled();

    await player.getByTestId("rfs-advancement-name").fill("Tiptoe Past It");
    await player.getByTestId("rfs-advancement-confirm").click();
    await expect(
      player
        .locator('[data-testid^="rfs-skill-"]')
        .filter({ hasText: "Tiptoe Past It" }),
    ).toBeVisible({ timeout: 15_000 });

    // Answered, so the table plays on.
    await expect(player.getByTestId("rfs-advancement-owed")).toHaveCount(0);
    await expect(
      player.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`),
    ).toBeEnabled({ timeout: 15_000 });
  });

  await test.step("the Game Master takes it away, and the sheet's own entry comes back", async () => {
    await openDockTab(page, "clocks");
    await expect(gmTable).toBeVisible({ timeout: 15_000 });
    await gmTable.getByTestId("rfs-table-clear").click();
    await expect(gmTable.getByTestId("rfs-table-none")).toBeVisible({
      timeout: 15_000,
    });

    await expect(player.getByTestId("rfs-opposition")).toBeVisible({
      timeout: 20_000,
    });
    await expect(player.getByTestId("rfs-table-target")).toHaveCount(0);
  });

  await test.step("the Game Master writes a skill on by hand, and the player's open sheet has it", async () => {
    await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
    await expect(page.getByTestId("rfs-sheet")).toBeVisible({
      timeout: 15_000,
    });
    await page.getByTestId("rfs-skills-edit").click();
    await expect(page.getByTestId("rfs-skills-add-form")).toBeVisible();

    // Beneath the starting skill, so its level is that skill's plus one and
    // is not asked for.
    await page.getByTestId("rfs-skills-add-name").fill("Haggle Over Laces");
    await page
      .getByTestId("rfs-skills-add-parent")
      .selectOption(STARTING_SKILL_ID);
    await expect(page.getByTestId("rfs-skills-add-level-fixed")).toContainText(
      "2",
    );
    await page.getByTestId("rfs-skills-add").click();
    await expect(page.getByTestId("rfs-error")).toHaveCount(0);

    const written = player
      .locator('[data-testid^="rfs-skill-"]')
      .filter({ hasText: "Haggle Over Laces" });
    await expect(written).toBeVisible({ timeout: 20_000 });
    await expect(written).toContainText("2");
    // What the dice gave is still there beside what was written by hand.
    await expect(
      player
        .locator('[data-testid^="rfs-skill-"]')
        .filter({ hasText: "Tiptoe Past It" }),
    ).toBeVisible();
  });

  await test.step("the player puts a status on their own character from the dock, and it stays", async () => {
    await expect(player.getByTestId("rfs-statuses")).toBeVisible({
      timeout: 15_000,
    });
    await player.getByTestId("rfs-status-name").fill("Stubbed toe");
    await player.getByTestId("rfs-status-modifier").fill("-1");
    await player.getByTestId("rfs-status-add").click();
    await expect(player.getByTestId("rfs-statuses-total")).toContainText("1", {
      timeout: 15_000,
    });
    await expect(player.getByTestId("rfs-error")).toHaveCount(0);

    // Not this tab's: a fresh page can only show what the server holds.
    await player.reload();
    await openOwnSheet(player, actorId);
    await expect(
      player
        .locator('[data-testid^="rfs-status-"]')
        .filter({ hasText: "Stubbed toe" }),
    ).toBeVisible({ timeout: 15_000 });

    // The dock is one narrow column, and an editable sheet has more in it
    // than a read-only one did. It must still fit.
    const overflow = await player
      .getByTestId("rfs-sheet")
      .evaluate((element) => element.scrollWidth - element.clientWidth);
    expect(overflow).toBeLessThanOrEqual(1);
  });
});
