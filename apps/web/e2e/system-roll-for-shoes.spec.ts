import type { Page } from "@playwright/test";

import { expectNoAxeViolations } from "./fixtures/axe";
import { graphql, registerAndCreateWorld } from "./fixtures/helpers";
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
