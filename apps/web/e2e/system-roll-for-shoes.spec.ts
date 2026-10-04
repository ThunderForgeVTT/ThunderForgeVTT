import type { Page } from "@playwright/test";

import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  setWorldSystem,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * specs/061-roll-for-shoes: the whole game, played once through the browser.
 *
 * Roll for Shoes is six rules, and this walks all of them that a single
 * session can reach: a character starts with one skill, a roll is judged
 * against what the Game Master named, a failure pays 1 XP, and XP buys a die
 * into a six purely to reach an advancement — which then asks the player to
 * name what they just learnt.
 *
 * Nothing here forces a roll. There is no dice seed (research D9), so the
 * test is built out of facts that hold for every roll:
 *
 *   - one die per skill level, each face between 1 and 6, the total their sum;
 *   - a single d6 can never beat an opposition of 6, because beating it means
 *     beating it — so this roll always fails, and always pays its 1 XP.
 *
 * That second fact is the whole reason the opposition is 6 and not 3.
 */

const STARTING_SKILL_ID = "starting-skill";

async function createCharacter(page: Page): Promise<{
  worldId: string;
  actorId: string;
}> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Roll for Shoes ${Date.now()}`,
    "e2erfs",
  );
  await setWorldSystem(page, worldId, "roll_for_shoes");
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
  return { worldId, actorId: actor.data.createActor.id };
}

test("a character starts with one skill, fails its way to XP, and spends it to learn something", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createCharacter(page);

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);

  const sheet = page.getByTestId("rfs-sheet");
  await expect(sheet).toBeVisible({ timeout: 15_000 });

  // Rule 3: a character who has never been saved still holds exactly one
  // skill. There is nothing to fill in, so an empty sheet would be a bug.
  const startingSkill = page.getByTestId(`rfs-skill-${STARTING_SKILL_ID}`);
  await expect(startingSkill).toBeVisible();
  await expect(startingSkill).toContainText("Do Anything");
  await expect(startingSkill).toContainText("1");
  await expect(page.getByTestId("rfs-xp")).toHaveText("0");

  // Rule 2: the Game Master names what has to be beaten.
  await page.getByTestId("rfs-opposition").fill("6");
  await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();

  const total = page.getByTestId("rfs-total");
  await expect(total).toBeVisible({ timeout: 15_000 });

  // Rule 1: one die per level, and this skill is level 1.
  const dice = page.locator('[data-testid^="rfs-die-"]');
  await expect(dice).toHaveCount(1);

  const faces = (await dice.allInnerTexts()).map((text) => Number(text.trim()));
  for (const face of faces) {
    expect(Number.isInteger(face)).toBe(true);
    expect(face).toBeGreaterThanOrEqual(1);
    expect(face).toBeLessThanOrEqual(6);
  }
  const sum = faces.reduce((running, face) => running + face, 0);
  expect(Number((await total.innerText()).trim())).toBe(sum);

  // Rule 2 again: a tie is not a win. One d6 cannot exceed 6, so whatever
  // was rolled, this failed — and rule 5 says a failure pays.
  await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);
  await expect(page.getByTestId("rfs-xp")).toHaveText("1", {
    timeout: 15_000,
  });

  // Rule 6: XP buys a die into a six, for advancement and nothing else. The
  // one die may already have shown a six, in which case the advancement is
  // already on offer and there is nothing left to buy.
  const advancement = page.getByTestId("rfs-advancement");
  if (!(await advancement.isVisible())) {
    await page.getByTestId("rfs-spend-xp").click();
    await expect(page.getByTestId("rfs-xp")).toHaveText("0", {
      timeout: 15_000,
    });
  }

  // Rule 4: every die shows a six, so the character has learnt something —
  // and the player, not the product, says what.
  await expect(advancement).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);

  await page.getByTestId("rfs-advancement-name").fill("Run Away Barefoot");
  await page.getByTestId("rfs-advancement-confirm").click();

  // The new skill sits one level above the one rolled, and the skill it grew
  // out of is still there — a lineage, not a replacement.
  const learnt = page
    .locator('[data-testid^="rfs-skill-"]')
    .filter({ hasText: "Run Away Barefoot" });
  await expect(learnt).toBeVisible({ timeout: 15_000 });
  await expect(learnt).toContainText("2");
  await expect(startingSkill).toBeVisible();

  // It is the character's now, not this page's.
  await page.reload();
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });
  await expect(
    page.locator('[data-testid^="rfs-skill-"]').filter({
      hasText: "Run Away Barefoot",
    }),
  ).toBeVisible({ timeout: 15_000 });

  // Scoped to the sheet, because that is what this pack owns. The rest of the
  // actor page is the host's, and a regression there belongs to the host's
  // specs rather than breaking Roll for Shoes.
  await expectNoAxeViolations(page, '[data-testid="rfs-sheet"]');
});

/**
 * Spec 067 Story 3: the verdict and the experience are the server's.
 *
 * Called without the sheet, because the claim is about what a client cannot
 * do. The mutation has no field for a pool, a total or a result; this proves
 * the rest — that a failure is paid where the roll is recorded, that the
 * record carries the verdict, and who is refused.
 */
const ROLL_SKILL = `
  mutation ($input: RollForShoesRollSkillInput!) {
    rollForShoesRollSkill(input: $input) {
      roll {
        formula
        dice { finalValue }
        outcome { verdict label }
      }
      total
      opposition
      xpAwarded
      xp
    }
  }
`;

interface SkillRollAnswer {
  data?: {
    rollForShoesRollSkill: {
      roll: {
        formula: string;
        dice: { finalValue: number }[];
        outcome: { verdict: string; label: string } | null;
      };
      total: number;
      opposition: number | null;
      xpAwarded: number;
      xp: number;
    } | null;
  };
  errors?: { message: string }[];
}

test("the server judges a skill roll, pays a failure, and keeps the verdict with the roll", async ({
  page,
  browser,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createCharacter(page);
  const input = (extra: Record<string, unknown>) => ({
    input: { worldId, actorId, skillId: STARTING_SKILL_ID, ...extra },
  });

  // One d6 cannot beat 6, so this fails whatever is rolled.
  const failed = await graphql<SkillRollAnswer>(
    page,
    ROLL_SKILL,
    input({ opposition: 6 }),
  );
  const roll = failed.data?.rollForShoesRollSkill;
  expect(roll, JSON.stringify(failed.errors ?? failed)).toBeTruthy();
  expect(roll!.roll.formula).toBe("1d6");
  expect(roll!.roll.dice).toHaveLength(1);
  expect(roll!.roll.outcome).toEqual({ verdict: "FAILURE", label: "Failure" });
  expect(roll!.xpAwarded).toBe(1);
  expect(roll!.xp).toBe(1);

  // Paid on the server: the sheet was never open, and the experience is there.
  const stored = await graphql<{
    data: { actorSystemData: { resourceData: { xp: number } } | null };
  }>(
    page,
    `
      query ($actorId: UUID!) {
        actorSystemData(actorId: $actorId) {
          resourceData
        }
      }
    `,
    { actorId },
  );
  expect(stored.data.actorSystemData?.resourceData.xp).toBe(1);

  // With nothing to beat, the roll is made and not judged, and pays nothing.
  const unopposed = await graphql<SkillRollAnswer>(page, ROLL_SKILL, input({}));
  expect(unopposed.data?.rollForShoesRollSkill?.roll.outcome).toBeNull();
  expect(unopposed.data?.rollForShoesRollSkill?.xp).toBe(1);

  // The world's roll history carries the verdict beside each roll.
  const history = await graphql<{
    data: {
      worldRollRecords: {
        resolution: { outcome: { verdict: string } | null };
      }[];
    };
  }>(
    page,
    `
      query ($worldId: UUID!) {
        worldRollRecords(worldId: $worldId) {
          resolution {
            outcome {
              verdict
            }
          }
        }
      }
    `,
    { worldId },
  );
  const verdicts = history.data.worldRollRecords.map(
    (record) => record.resolution.outcome?.verdict ?? null,
  );
  expect(verdicts).toHaveLength(2);
  expect(verdicts).toContain("FAILURE");
  expect(verdicts).toContain(null);

  // A skill the character does not have is refused, and nothing is rolled.
  const madeUp = await graphql<SkillRollAnswer>(
    page,
    ROLL_SKILL,
    input({ skillId: "made-up", opposition: 6 }),
  );
  expect(madeUp.data?.rollForShoesRollSkill ?? null).toBeNull();
  expect(madeUp.errors?.[0]?.message ?? "").toContain("no such skill");

  // A player who does not hold the character cannot roll it for experience.
  const player = await inviteAndJoinAsPlayer(browser, page, worldId, "e2erfsv");
  const notTheirs = await graphql<SkillRollAnswer>(
    player,
    ROLL_SKILL,
    input({ opposition: 6 }),
  );
  expect(notTheirs.data?.rollForShoesRollSkill ?? null).toBeNull();
  expect(notTheirs.errors?.length ?? 0).toBeGreaterThan(0);

  const after = await graphql<SkillRollAnswer>(page, ROLL_SKILL, input({}));
  expect(after.data?.rollForShoesRollSkill?.xp).toBe(1);
});

test("the sheet works in the play dock, where the player holding the character keeps it up to date", async ({
  page,
  browser,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createCharacter(page);

  // The dock opens a character inside the pane only for the person playing it
  // (spec 031 FR-002); everybody else gets a new tab, and the server refuses a
  // claim from a Game Master outright — "The GM does not claim characters".
  // So this needs a second account at the table, not a shortcut.
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

  const player = await inviteAndJoinAsPlayer(browser, page, worldId, "e2erfsp");

  // Claiming is all the player does. The claim grants them Editor on the
  // character (spec 063), which is what lets the XP a failed roll earns be
  // written — rule 5, the whole of progression. No grant by hand here: this
  // test is the proof that none is needed.
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

  // The dock mounts a pack's sheet with the viewer's real edit right — the
  // same answer the full actor page gives. Roll for Shoes is mostly played
  // with no map at all, so for this player the dock *is* their seat: a sheet
  // they could read here and not change would be a character that cannot be
  // kept up to date where it is played.
  await player.goto(`/world/${worldId}/play`);
  await player.getByTestId("world-dock-tab-actors").click();
  await player.getByTestId(`actor-view-${actorId}`).click();

  const body = player.getByTestId("in-pane-sheet-body");
  await expect(body).toBeVisible({ timeout: 15_000 });
  await expect(player.getByTestId("in-pane-sheet-unavailable")).toHaveCount(0);

  const sheet = player.getByTestId("rfs-sheet");
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(
    player.getByTestId(`rfs-skill-${STARTING_SKILL_ID}`),
  ).toBeVisible();

  // The claim made this player an Editor, so the description is a box to
  // type in rather than text — and what is typed is written when the box is
  // left, which is checked further down, once the sheet has been closed and
  // opened again and can only be showing what the server holds.
  const description = player.locator('textarea[data-testid="rfs-description"]');
  await expect(description).toHaveCount(1);
  await description.fill("Has never owned a pair of shoes.");
  await description.blur();
  await expect(player.getByTestId("rfs-error")).toHaveCount(0);

  // The dock is one narrow column. A sheet that overflows it is unusable
  // however correct its contents are.
  const overflow = await sheet.evaluate(
    (element) => element.scrollWidth - element.clientWidth,
  );
  expect(overflow).toBeLessThanOrEqual(1);

  // The Game Master is at the table too, reading the chat. A roll is a public
  // act; they should not have to take the player's word for it.
  await page.goto(`/world/${worldId}/play`);
  await page.getByTestId("world-dock-tab-chat").click();
  await expect(page.getByTestId("chat-panel")).toBeVisible({ timeout: 15_000 });

  // Rolling is the one thing a player must still be able to do here.
  await player.getByTestId("rfs-opposition").fill("6");
  await player.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();

  await expect(player.getByTestId("rfs-total")).toBeVisible({
    timeout: 15_000,
  });
  await expect(player.locator('[data-testid^="rfs-die-"]')).toHaveCount(1);
  await expect(player.getByTestId("rfs-result")).toContainText(/fail/i);

  // The sheet reports a refused write in a badge rather than throwing, so
  // without this an XP that never reached the server would read as a plain
  // disagreement about a number.
  await expect(player.getByTestId("rfs-error")).toHaveCount(0);
  await expect(player.getByTestId("rfs-xp")).toHaveText("1", {
    timeout: 15_000,
  });

  // The roll was said where everybody reads, dice and all.
  await expect(
    page.getByTestId("chat-message").filter({
      hasText: /Barefoot rolls Do Anything \(1d6\): \d against 6 — fails/,
    }),
  ).toHaveCount(1, { timeout: 20_000 });

  // The dock unmounts the sheet when the player looks away. The attempt — and
  // with it any advancement still unanswered — must be there on return.
  await player.getByTestId("in-pane-sheet-dismiss").click();
  await player.getByTestId(`actor-view-${actorId}`).click();
  await expect(player.getByTestId("rfs-total")).toBeVisible({
    timeout: 15_000,
  });
  await expect(player.getByTestId("rfs-result")).toContainText(/fail/i);

  // And the description is the character's now, not the closed sheet's: this
  // mount began with nothing typed, so what it shows was read back.
  await expect(
    player.locator('textarea[data-testid="rfs-description"]'),
  ).toHaveValue("Has never owned a pair of shoes.", { timeout: 15_000 });

  await expectNoAxeViolations(player, '[data-testid="rfs-sheet"]');
});
