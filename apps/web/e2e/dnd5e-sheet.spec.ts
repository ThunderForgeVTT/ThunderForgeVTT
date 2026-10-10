import type { Locator, Page } from "@playwright/test";

import { expectNoAxeViolations } from "./fixtures/axe";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * The D&D 5e character sheet, as a table would use it.
 *
 * What this proves, in order:
 *
 *  1. The pack's own sheet is what the actor page mounts for a 5e actor, laid
 *     out in columns on a wide screen and one column at 375px.
 *  2. A score typed into the sheet shows its modifier; a level sets the
 *     proficiency bonus; both survive a reload.
 *  3. A proficiency ticked on the sheet reaches the *server's* roll. The
 *     validator used to demand a map of booleans while the rules read a list,
 *     so a proficiency saved in the documented shape never changed a roll.
 *     This test rolls Stealth through `rollCheck` and checks the arithmetic.
 *  4. Hit points, a spellcasting ability and notes write through their slots.
 *  5. The view route is read-only: the same regions, nothing to change.
 *  6. A player who claimed the character changes its numbers themselves —
 *     a score, hit points, a proficiency — with no Game Master in the loop,
 *     and finds them there after a reload.
 *  7. A Game Master gives an existing NPC a stat block from its own page, is
 *     asked before a second one replaces the first, and can then change any
 *     number by hand on the same sheet.
 */

const REGION_ORDER = [
  "identity",
  "abilities",
  "combat",
  "skills",
  "spellcasting",
  "proficiencies",
  "features",
  "notes",
];

type GqlResult<T> = { data?: T; errors?: { message: string }[] };

const ROLL_CHECK = `
  mutation RollCheck($worldId: UUID!, $actorId: UUID!, $checkId: String!) {
    rollCheck(worldId: $worldId, actorId: $actorId, checkId: $checkId) {
      formula
      resultValue
      dice { numericSides finalValue }
    }
  }
`;

const ACTOR_SYSTEM_DATA = `
  query ActorSystemData($actorId: UUID!) {
    actorSystemData(actorId: $actorId) {
      abilityData
      resourceData
      proficiencyData
      traitData
      spellData
    }
  }
`;

interface Resolution {
  formula: string;
  resultValue: number;
  dice: { numericSides: number | null; finalValue: number }[];
}

async function createFifthEditionCharacter(page: Page): Promise<{
  worldId: string;
  actorId: string;
}> {
  await register(page, freshCredentials("e2e5esheet"));
  // The world has to *be* a 5e world: `rollCheck` resolves a check id against
  // the world's system, so a default (genie) world declares no Stealth.
  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: `E2E 5e Sheet ${Date.now()}`, gameSystemId: "dnd5e" } },
  );
  const worldId = world.data?.createWorld?.id;
  if (!worldId) {
    throw new Error(
      `could not create a 5e world: ${JSON.stringify(world.errors ?? world)}`,
    );
  }
  const actor = await graphql<GqlResult<{ createActor: { id: string } }>>(
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
        label: "Vex the Quiet",
        isNpc: false,
        gameSystemId: "dnd5e",
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  if (!actorId) {
    throw new Error(
      `could not create a 5e actor: ${JSON.stringify(actor.errors ?? actor)}`,
    );
  }
  return { worldId, actorId };
}

function region(page: Page, id: string): Locator {
  return page.locator(
    `[data-testid="dnd5e-actor-sheet"] [data-region="${id}"]`,
  );
}

async function boxOf(locator: Locator) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("region is not rendered");
  return box;
}

async function gridColumnCount(page: Page): Promise<number> {
  return page
    .getByTestId("dnd5e-actor-sheet")
    .evaluate(
      (element) =>
        getComputedStyle(element).gridTemplateColumns.split(" ").length,
    );
}

/** Type a number into a commit-on-blur field and leave it. */
async function setNumber(page: Page, testId: string, value: number) {
  const input = page.getByTestId(testId);
  await input.fill(String(value));
  await input.press("Enter");
}

async function systemData(page: Page, actorId: string) {
  const result = await graphql<
    GqlResult<{
      actorSystemData: {
        abilityData: Record<string, unknown> | null;
        resourceData: Record<string, unknown> | null;
        proficiencyData: Record<string, unknown> | null;
        traitData: Record<string, unknown> | null;
        spellData: Record<string, unknown> | null;
      } | null;
    }>
  >(page, ACTOR_SYSTEM_DATA, { actorId });
  return result.data?.actorSystemData ?? null;
}

test("the 5e sheet lays out, derives, persists, and its proficiencies reach the roll", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createFifthEditionCharacter(page);

  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  const sheet = page.getByTestId("dnd5e-actor-sheet");
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(sheet).toHaveAttribute("data-editable", "true");
  // A character has a level: no challenge rating, no creature type.
  await expect(page.getByTestId("dnd5e-challenge-select")).toHaveCount(0);
  await expect(sheet.locator("#dnd5e-creature-type")).toHaveCount(0);

  // 1. The regions, in reading order, each a named section with a heading.
  const ids = await sheet
    .locator(":scope > section")
    .evaluateAll((sections) =>
      sections.map((section) => section.getAttribute("data-region")),
    );
  expect(ids).toEqual(REGION_ORDER);
  for (const id of REGION_ORDER) {
    await expect(
      region(page, id).getByRole("heading", { level: 2 }),
    ).toBeVisible();
  }
  expect(await gridColumnCount(page)).toBe(3);
  const identity = await boxOf(region(page, "identity"));
  const abilities = await boxOf(region(page, "abilities"));
  expect(abilities.y).toBeCloseTo(identity.y, 0);
  expect(abilities.x).toBeGreaterThan(identity.x + identity.width - 1);
  expect(abilities.width).toBeGreaterThan(identity.width * 1.5);
  // The host draws the portrait at the head of column A, beside the sheet in
  // column B (spec 088 contracts/layouts.md); the sheet draws no picture of
  // its own.
  const imagery = await boxOf(page.getByTestId("actor-imagery-panel"));
  expect(imagery.y).toBeLessThanOrEqual(identity.y + 1);
  expect(imagery.x + imagery.width).toBeLessThanOrEqual(identity.x + 1);
  await expect(sheet.locator('input[type="file"], img')).toHaveCount(0);
  await expectNoAxeViolations(page, '[data-testid="dnd5e-actor-sheet"]');

  // 2. Scores show modifiers; level sets the proficiency bonus.
  await setNumber(page, "dnd5e-score-dexterity-input", 16);
  await expect(page.getByTestId("dnd5e-mod-dexterity")).toHaveText("+3", {
    timeout: 10_000,
  });
  await expect(page.getByTestId("dnd5e-initiative")).toHaveText("+3");
  await setNumber(page, "dnd5e-score-strength-input", 8);
  await expect(page.getByTestId("dnd5e-mod-strength")).toHaveText("-1", {
    timeout: 10_000,
  });

  await page.getByTestId("dnd5e-class-select").selectOption("Rogue");
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+3");
  await setNumber(page, "dnd5e-level-input", 5);
  await expect(page.getByTestId("dnd5e-proficiency-bonus")).toHaveText("+3", {
    timeout: 10_000,
  });

  // 3. Tick Stealth: the sheet shows DEX +3 plus proficiency +3 ...
  await page.getByTestId("dnd5e-skill-stealth-proficient").check();
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+6", {
    timeout: 10_000,
  });
  await expect(page.getByTestId("dnd5e-skill-acrobatics-bonus")).toHaveText(
    "+3",
  );
  // ... and the stored shape is the list the rules read.
  const stored = await systemData(page, actorId);
  expect(stored?.proficiencyData?.skill_proficiencies).toEqual(["stealth"]);
  expect(stored?.traitData).toMatchObject({ class: "Rogue", level: 5 });
  expect(stored?.abilityData).toMatchObject({ dexterity: 16, strength: 8 });

  // ... so the server's roll carries it. The total less the die is the
  // modifier, and the only place +6 can come from is the ruleset reading
  // what the sheet wrote.
  const rolled = await graphql<GqlResult<{ rollCheck: Resolution }>>(
    page,
    ROLL_CHECK,
    { worldId, actorId, checkId: "stealth" },
  );
  expect(rolled.errors, "the owner may roll their own Stealth").toBeFalsy();
  const stealth = rolled.data!.rollCheck;
  expect(stealth.dice).toHaveLength(1);
  expect(stealth.dice[0].numericSides).toBe(20);
  expect(
    stealth.resultValue - stealth.dice[0].finalValue,
    "Stealth must be DEX +3 plus proficiency +3 at level 5",
  ).toBe(6);

  // Expertise waits on proficiency, then doubles it: +3 and +3 twice. The
  // roll is asked for while the box is ticked, and it is unticked again so
  // the rest of this test reads the plain proficient +6.
  await expect(
    page.getByTestId("dnd5e-skill-acrobatics-expertise"),
  ).toBeDisabled();
  await page.getByTestId("dnd5e-skill-stealth-expertise").check();
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+9", {
    timeout: 10_000,
  });
  const withExpertise = await systemData(page, actorId);
  expect(withExpertise?.proficiencyData?.skill_expertise).toEqual(["stealth"]);
  const expert = await graphql<GqlResult<{ rollCheck: Resolution }>>(
    page,
    ROLL_CHECK,
    { worldId, actorId, checkId: "stealth" },
  );
  expect(expert.errors).toBeFalsy();
  expect(
    expert.data!.rollCheck.resultValue -
      expert.data!.rollCheck.dice[0].finalValue,
    "expert Stealth must be DEX +3 plus twice proficiency +3 at level 5",
  ).toBe(9);
  await page.getByTestId("dnd5e-skill-stealth-expertise").uncheck();
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+6", {
    timeout: 10_000,
  });

  // A save proficiency goes the same way.
  await page.getByTestId("dnd5e-save-dexterity").check();
  await expect(page.getByTestId("dnd5e-save-dexterity-bonus")).toHaveText(
    "+6",
    { timeout: 10_000 },
  );
  const saved = await graphql<GqlResult<{ rollCheck: Resolution }>>(
    page,
    ROLL_CHECK,
    { worldId, actorId, checkId: "saveDexterity" },
  );
  expect(saved.errors).toBeFalsy();
  expect(
    saved.data!.rollCheck.resultValue -
      saved.data!.rollCheck.dice[0].finalValue,
  ).toBe(6);

  // 4. Hit points, a casting ability, and notes.
  await setNumber(page, "dnd5e-max-hp-input", 38);
  await setNumber(page, "dnd5e-current-hp-input", 21);
  await setNumber(page, "dnd5e-temp-hp-input", 5);
  await expect(page.getByTestId("dnd5e-current-hp-input")).toHaveValue("21", {
    timeout: 10_000,
  });
  await expect(
    region(page, "combat").getByRole("meter", { name: /Hit points/ }),
  ).toHaveAttribute("aria-valuenow", "21");

  await page
    .getByTestId("dnd5e-spellcasting-ability")
    .selectOption("intelligence");
  // INT 10 is +0; level 5 is +3: DC 11, attack +3.
  await expect(page.getByTestId("dnd5e-spell-save-dc")).toHaveText("11", {
    timeout: 10_000,
  });
  await expect(page.getByTestId("dnd5e-spell-attack")).toHaveText("+3");
  // Level 5 full caster: four 1st-level slots; spend one.
  await expect(page.getByTestId("dnd5e-spell-slot-1")).toContainText("4 / 4");
  await page.getByTestId("dnd5e-spell-slot-1-spend").click();
  await expect(page.getByTestId("dnd5e-spell-slot-1")).toContainText("3 / 4", {
    timeout: 10_000,
  });

  await page
    .getByTestId("dnd5e-notes-input")
    .fill("Wanted in Waterdeep under another name.");
  await page.getByTestId("dnd5e-notes-save").click();
  await expect(page.getByTestId("dnd5e-notes-save")).toBeDisabled({
    timeout: 10_000,
  });

  // Everything survives a reload.
  await page.reload();
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("dnd5e-score-dexterity-input")).toHaveValue(
    "16",
  );
  await expect(page.getByTestId("dnd5e-level-input")).toHaveValue("5");
  await expect(page.getByTestId("dnd5e-class-select")).toHaveValue("Rogue");
  await expect(
    page.getByTestId("dnd5e-skill-stealth-proficient"),
  ).toBeChecked();
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+6");
  await expect(page.getByTestId("dnd5e-current-hp-input")).toHaveValue("21");
  await expect(page.getByTestId("dnd5e-max-hp-input")).toHaveValue("38");
  await expect(page.getByTestId("dnd5e-temp-hp-input")).toHaveValue("5");
  await expect(page.getByTestId("dnd5e-spell-slot-1")).toContainText("3 / 4");
  await expect(page.getByTestId("dnd5e-notes-input")).toHaveValue(
    "Wanted in Waterdeep under another name.",
  );
  const after = await systemData(page, actorId);
  expect(after?.resourceData).toMatchObject({
    max_hp: 38,
    current_hp: 21,
    temporary_hp: 5,
  });
  expect(after?.spellData).toMatchObject({
    spellcasting_ability: "intelligence",
    spell_save_dc: 11,
    spell_attack_bonus: 3,
  });

  // Narrow: one column, in the same order, no sideways scroll.
  await page.setViewportSize({ width: 375, height: 812 });
  await page.reload();
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  expect(await gridColumnCount(page)).toBe(1);
  let previousBottom = -Infinity;
  let firstX: number | null = null;
  for (const id of REGION_ORDER) {
    const box = await boxOf(region(page, id));
    expect(box.y, `${id} is below the region before it`).toBeGreaterThanOrEqual(
      previousBottom - 1,
    );
    firstX ??= box.x;
    expect(box.x).toBeCloseTo(firstX, 0);
    previousBottom = box.y + box.height;
  }
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - window.innerWidth,
  );
  // Named in the failure, because "32" alone does not say what is too wide.
  const tooWide = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("body *")]
      .filter((el) => el.getBoundingClientRect().right > window.innerWidth + 1)
      .slice(0, 12)
      .map(
        (el) =>
          `${el.tagName.toLowerCase()}[${el.dataset.testid ?? el.id}] ${Math.round(
            el.getBoundingClientRect().right,
          )}px .${String(el.className).slice(0, 60)}`,
      ),
  );
  expect(
    overflow,
    `the page must not scroll sideways at 375px; past the edge: ${tooWide.join(" | ")}`,
  ).toBeLessThanOrEqual(0);
  await expectNoAxeViolations(page, '[data-testid="dnd5e-actor-sheet"]');

  // 5. Read-only: the same regions, the same facts, nothing to change.
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`/world/${worldId}/actor/${actorId}/view`);
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(sheet).toHaveAttribute("data-editable", "false");
  await expect(sheet.locator("input, textarea, select, button")).toHaveCount(0);
  await expect(page.getByTestId("dnd5e-mod-dexterity")).toHaveText("+3");
  await expect(page.getByTestId("dnd5e-skill-stealth-bonus")).toHaveText("+6");
  await expect(page.getByTestId("dnd5e-current-hp")).toHaveText("21");
  await expect(region(page, "notes")).toContainText(
    "Wanted in Waterdeep under another name.",
  );
  await expectNoAxeViolations(page, '[data-testid="dnd5e-actor-sheet"]');
});

test("a player who claimed the character edits its sheet, and the edits persist", async ({
  page,
  browser,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createFifthEditionCharacter(page);
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

  // A second account at the table: the server refuses a claim from the Game
  // Master, and the point here is what a *player* is allowed to do.
  const player = await inviteAndJoinAsPlayer(browser, page, worldId, "e2e5ep");
  const claim = await graphql<GqlResult<{ claimActor: { actorId: string } }>>(
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

  await player.goto(`/world/${worldId}/actor/${actorId}/edit`);
  const sheet = player.getByTestId("dnd5e-actor-sheet");
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(sheet).toHaveAttribute("data-editable", "true");

  // A score past a character's own ceiling: a belt of giant strength is 21
  // and up, and the sheet used to stop at 20.
  await setNumber(player, "dnd5e-score-strength-input", 21);
  await expect(player.getByTestId("dnd5e-mod-strength")).toHaveText("+5", {
    timeout: 15_000,
  });
  await setNumber(player, "dnd5e-score-wisdom-input", 14);
  await expect(player.getByTestId("dnd5e-mod-wisdom")).toHaveText("+2", {
    timeout: 15_000,
  });
  await setNumber(player, "dnd5e-max-hp-input", 27);
  await setNumber(player, "dnd5e-current-hp-input", 19);
  await expect(player.getByTestId("dnd5e-current-hp-input")).toHaveValue("19", {
    timeout: 15_000,
  });
  await player.getByTestId("dnd5e-skill-perception-proficient").check();
  await expect(player.getByTestId("dnd5e-skill-perception-bonus")).toHaveText(
    "+4",
    { timeout: 15_000 },
  );
  await setNumber(player, "dnd5e-speed-fly-input", 30);

  // The server's copy, asked for by the Game Master: what the player typed
  // is what the table has.
  await expect
    .poll(async () => (await systemData(page, actorId))?.traitData?.speed_fly, {
      timeout: 15_000,
    })
    .toBe(30);
  const stored = await systemData(page, actorId);
  expect(stored?.abilityData).toMatchObject({ strength: 21, wisdom: 14 });
  expect(stored?.resourceData).toMatchObject({ max_hp: 27, current_hp: 19 });
  expect(stored?.proficiencyData?.skill_proficiencies).toContain("perception");

  await player.reload();
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(player.getByTestId("dnd5e-score-strength-input")).toHaveValue(
    "21",
  );
  await expect(player.getByTestId("dnd5e-score-wisdom-input")).toHaveValue(
    "14",
  );
  await expect(player.getByTestId("dnd5e-max-hp-input")).toHaveValue("27");
  await expect(player.getByTestId("dnd5e-current-hp-input")).toHaveValue("19");
  await expect(
    player.getByTestId("dnd5e-skill-perception-proficient"),
  ).toBeChecked();
  await expect(player.getByTestId("dnd5e-skill-perception-bonus")).toHaveText(
    "+4",
  );
  await expect(player.getByTestId("dnd5e-speed-fly-input")).toHaveValue("30");
  // Applying a stat block is the Game Master's; a player is not offered it.
  await expect(player.getByTestId("actor-stat-block")).toHaveCount(0);
  await player.context().close();
});

test("a Game Master applies a stat block to an existing NPC, then edits it by hand", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const { worldId } = await createFifthEditionCharacter(page);
  const made = await graphql<GqlResult<{ createActor: { id: string } }>>(
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
        label: "The Thing Under the Bridge",
        isNpc: true,
        gameSystemId: "dnd5e",
      },
    },
  );
  const npcId = made.data?.createActor?.id;
  if (!npcId) {
    throw new Error(`could not create an NPC: ${JSON.stringify(made)}`);
  }

  await page.goto(`/world/${worldId}/actor/${npcId}/edit`);
  await expect(page.getByTestId("dnd5e-actor-sheet")).toBeVisible({
    timeout: 15_000,
  });
  const panel = page.getByTestId("actor-stat-block");
  await expect(panel).toBeVisible({ timeout: 15_000 });

  // A blank sheet has nothing to lose, so nothing is asked.
  await panel.getByTestId("actor-stat-block-select").selectOption("troll");
  await panel.getByTestId("actor-stat-block-apply").click();
  await expect(panel.getByTestId("actor-stat-block-status")).toContainText(
    "Troll applied",
    { timeout: 30_000 },
  );
  await expect(panel.getByTestId("actor-stat-block-problems")).toHaveCount(0);
  const troll = await systemData(page, npcId);
  expect(troll?.resourceData).toMatchObject({ max_hp: 94, current_hp: 94 });
  expect(troll?.abilityData).toMatchObject({ armor_class: 15, strength: 18 });
  expect(troll?.traitData).toMatchObject({ challenge: "5", size: "large" });
  // Challenge 5 is a +3 bonus, read off the rating: a troll has no level.
  await expect(page.getByTestId("dnd5e-proficiency-bonus")).toHaveText("+3", {
    timeout: 15_000,
  });
  await expect(page.getByTestId("dnd5e-max-hp-input")).toHaveValue("94");
  await expect(page.getByTestId("actor-abilities-panel")).toContainText(
    "Rend (Troll)",
  );

  // A second block would overwrite the first, so this time it asks; backing
  // out changes nothing.
  await panel.getByTestId("actor-stat-block-select").selectOption("ogre");
  await panel.getByTestId("actor-stat-block-apply").click();
  await expect(panel.getByTestId("actor-stat-block-confirm")).toBeVisible({
    timeout: 15_000,
  });
  await panel.getByTestId("actor-stat-block-cancel").click();
  await expect(panel.getByTestId("actor-stat-block-confirm")).toHaveCount(0);
  expect((await systemData(page, npcId))?.resourceData?.max_hp).toBe(94);

  await panel.getByTestId("actor-stat-block-apply").click();
  await panel.getByTestId("actor-stat-block-replace").click();
  await expect(panel.getByTestId("actor-stat-block-status")).toContainText(
    "Ogre applied",
    { timeout: 30_000 },
  );
  const ogre = await systemData(page, npcId);
  expect(ogre?.resourceData).toMatchObject({ max_hp: 68, current_hp: 68 });
  expect(ogre?.abilityData).toMatchObject({ armor_class: 11 });
  expect(ogre?.traitData?.challenge).toBe("2");

  // A tougher ogre than the book's: the numbers are the Game Master's now.
  await expect(page.getByTestId("dnd5e-max-hp-input")).toHaveValue("68", {
    timeout: 15_000,
  });
  await setNumber(page, "dnd5e-max-hp-input", 80);
  await expect
    .poll(async () => (await systemData(page, npcId))?.resourceData?.max_hp, {
      timeout: 15_000,
    })
    .toBe(80);
  // Still a creature after a hand edit: no level crept into the slot.
  await page.getByTestId("dnd5e-challenge-select").selectOption("3");
  await expect
    .poll(async () => (await systemData(page, npcId))?.traitData?.challenge, {
      timeout: 15_000,
    })
    .toBe("3");
  expect((await systemData(page, npcId))?.traitData).not.toHaveProperty(
    "level",
  );
});
