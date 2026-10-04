import type { Page } from "@playwright/test";

import {
  graphql,
  registerAndCreateWorld,
  setWorldSystem,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * specs/062-roll-for-shoes-extras: the optional rules the game leaves to the
 * table, each one off until a world turns it on.
 *
 * The claim that the core game did not change is **not** made here — it is made
 * by `system-roll-for-shoes.spec.ts` continuing to pass unmodified, and
 * exhaustively by `game.test.ts` comparing every possible roll against spec
 * 061's own comparison. What this file proves is the other half: that a world
 * which does turn something on gets it, on screen, having reached the database.
 *
 * As in spec 061, there is no dice seed. Every assertion is built either from
 * something true of *every* roll, or from the faces this particular roll
 * actually showed, read back off the page.
 */

const STARTING_SKILL_ID = "starting-skill";

/** All five settings, stated every time — the mutation is a whole-row write. */
interface Extras {
  difficultyMode: "free" | "rolled" | "target";
  tieSucceeds: boolean;
  statusesEnabled: boolean;
  skillSlotsEnabled: boolean;
  startingSkills: { name: string; level: number }[];
}

const NOTHING_ON: Extras = {
  difficultyMode: "free",
  tieSucceeds: false,
  statusesEnabled: false,
  skillSlotsEnabled: false,
  startingSkills: [],
};

async function createCharacter(page: Page): Promise<{
  worldId: string;
  actorId: string;
}> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E RfS Extras ${Date.now()}`,
    "e2erfsx",
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

/**
 * Turn settings on as the Game Master.
 *
 * Through the pack's own root field, which is the only way these are written —
 * there is no column on `worlds` to reach behind it (ADR-063).
 */
async function setExtras(
  page: Page,
  worldId: string,
  extras: Partial<Extras>,
): Promise<void> {
  const written = await graphql<{
    data?: {
      updateRollForShoesWorldSettings?: { difficultyMode: string };
    };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: UpdateRollForShoesWorldSettingsInput!) {
        updateRollForShoesWorldSettings(input: $input) {
          difficultyMode
          tieSucceeds
          statusesEnabled
          skillSlotsEnabled
          startingSkills {
            name
            level
          }
        }
      }
    `,
    { input: { worldId, ...NOTHING_ON, ...extras } },
  );
  expect(
    written.data?.updateRollForShoesWorldSettings,
    `settings refused: ${JSON.stringify(written.errors ?? written)}`,
  ).toBeTruthy();
}

/** The faces currently drawn under a given test-id prefix, as numbers. */
async function facesUnder(page: Page, prefix: string): Promise<number[]> {
  const texts = await page
    .locator(`[data-testid^="${prefix}"]`)
    .allInnerTexts();
  return texts.map((text) => Number(text.trim()));
}

test("the Game Master rolls for the opposition, and those dice are never the character's", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { difficultyMode: "rolled" });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });

  // The bands only exist because this world asked for them. A world left
  // alone never sees them, which is what spec 061's spec keeps proving.
  //
  // This is the Game Master's sheet, so the bands are the table's: choosing
  // one sets what every player at the table has to beat. The per-roll picker
  // a player relays a band through is not drawn here as well — one control
  // per meaning.
  await expect(page.getByTestId("rfs-table-difficulty")).toBeVisible({
    timeout: 15_000,
  });
  for (const band of ["easy", "moderate", "hard", "veryHard"]) {
    await expect(page.getByTestId(`rfs-table-band-${band}`)).toBeVisible({
      timeout: 15_000,
    });
  }
  await expect(page.getByTestId("rfs-difficulty-mode")).toHaveCount(0);

  // Very Hard is four dice — one per point of difficulty, exactly as a skill
  // rolls one per level. They are rolled by the server, once, when the band
  // is chosen.
  await page.getByTestId("rfs-table-band-veryHard").click();
  const gmDice = page.locator('[data-testid^="rfs-table-gm-die-"]');
  await expect(gmDice).toHaveCount(4, { timeout: 15_000 });

  const gmFaces = await facesUnder(page, "rfs-table-gm-die-");
  for (const face of gmFaces) {
    expect(Number.isInteger(face)).toBe(true);
    expect(face).toBeGreaterThanOrEqual(1);
    expect(face).toBeLessThanOrEqual(6);
  }
  const gmTotal = gmFaces.reduce((running, face) => running + face, 0);
  // The dice arrive at the one number the table rolls against; they do not
  // become a second opposition. And while it stands there is no field on the
  // sheet to type a different one into.
  await expect(page.getByTestId("rfs-table-target")).toHaveText(
    String(gmTotal),
  );
  await expect(page.getByTestId("rfs-opposition")).toHaveCount(0);

  // FR-014, the reason the two sets of dice are drawn apart: the character
  // rolls one die for a level-1 skill, and it stays one however many the Game
  // Master rolled.
  await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
  await expect(page.getByTestId("rfs-total")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('[data-testid^="rfs-die-"]')).toHaveCount(1);
  await expect(gmDice).toHaveCount(4);

  const characterFaces = await facesUnder(page, "rfs-die-");
  expect(Number((await page.getByTestId("rfs-total").innerText()).trim())).toBe(
    characterFaces.reduce((running, face) => running + face, 0),
  );

  // One d6 cannot beat four, whatever any of the five dice showed: the
  // Game Master's minimum is 4 and the character's maximum is 6 — so this is
  // only certain when the Game Master rolled above 6, and otherwise the page
  // is read rather than predicted.
  const expected = characterFaces[0]! > gmTotal ? /success/i : /fail/i;
  await expect(page.getByTestId("rfs-result")).toContainText(expected);

  await expect(page.getByTestId("rfs-error")).toHaveCount(0);
});

test("a fixed number per band, with the typed number still there", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { difficultyMode: "target" });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("rfs-table-band-easy")).toBeVisible({
    timeout: 15_000,
  });

  // The four numbers the game names. Nothing is rolled for them. Each is set
  // for the table, because this is the Game Master choosing.
  for (const [band, target] of [
    ["easy", "3"],
    ["moderate", "6"],
    ["hard", "9"],
    ["veryHard", "12"],
  ] as const) {
    await page.getByTestId(`rfs-table-band-${band}`).click();
    await expect(page.getByTestId("rfs-table-target")).toHaveText(target, {
      timeout: 15_000,
    });
  }
  await expect(page.locator('[data-testid^="rfs-table-gm-die-"]')).toHaveCount(
    0,
  );

  // FR-011: a Game Master who wants to name a number can still name one, in
  // any mode, and the named one is what the roll is judged against.
  await page.getByTestId("rfs-table-number").fill("99");
  await page.getByTestId("rfs-table-set").click();
  await expect(page.getByTestId("rfs-table-target")).toHaveText("99", {
    timeout: 15_000,
  });
  await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
  await expect(page.getByTestId("rfs-total")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);

  await expect(page.getByTestId("rfs-error")).toHaveCount(0);
});

test("with the tie rule on, matching the opposition is good enough and pays nothing", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { tieSucceeds: true });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });

  // Nothing about the *sheet* changes — the tie rule changes what a verdict
  // means, not what is on screen (T030). So there is no new control to find.
  await expect(page.getByTestId("rfs-difficulty-mode")).toHaveCount(0);

  // A single d6 against 6 is the only roll that can tie, and it ties on a six.
  // Without a dice seed the tie has to be waited for rather than arranged; each
  // roll is independent, so sixty attempts miss it about once in a million
  // runs. Every attempt asserts the rule, so the loop is not merely a search:
  // it is sixty judgements, of which at least one is the tie.
  await page.getByTestId("rfs-opposition").fill("6");

  let sawTie = false;
  let xp = 0;
  for (let attempt = 1; attempt <= 60 && !sawTie; attempt += 1) {
    await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();

    // Which branch this roll took is read from the ledger, not from the dice.
    // A failure pays 1 XP and that increment is the one unambiguous signal
    // that *this* roll — rather than the one still on screen from the last
    // iteration — has landed. A tie pays nothing, so the absence of the
    // increment is precisely the case being hunted.
    let paid = true;
    try {
      await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp + 1), {
        timeout: 8_000,
      });
    } catch {
      paid = false;
    }

    if (paid) {
      xp += 1;
      await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);
      const [face] = await facesUnder(page, "rfs-die-");
      expect(
        face,
        "a roll that paid must have come in under the opposition",
      ).toBeLessThan(6);
      continue;
    }

    // The tie: the one d6 showed a six, matched the opposition, and under this
    // world's rule that is good enough. In spec 061's world the identical roll
    // is a failure worth 1 XP.
    sawTie = true;
    await expect(page.getByTestId("rfs-result")).toContainText(/success/i);
    await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp));
    expect(await facesUnder(page, "rfs-die-")).toEqual([6]);
    await expect(page.getByTestId("rfs-error")).toHaveCount(0);
  }

  expect(
    sawTie,
    "sixty single-d6 rolls without a six — the dice engine is not random",
  ).toBe(true);
});

test("a status moves the total and leaves the dice alone", async ({ page }) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { statusesEnabled: true });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-statuses")).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("rfs-statuses-none")).toBeVisible();

  // A label the table wrote, worth what the table said. The system ships no
  // list of these and judges none of them (FR-020).
  await page.getByTestId("rfs-status-name").fill("Buried to the neck");
  await page.getByTestId("rfs-status-modifier").fill("-100");
  await page.getByTestId("rfs-status-add").click();

  // It reached the database: the row is drawn from what came back, not from
  // what was typed.
  await expect(
    page.locator('[data-testid^="rfs-status-"]').first(),
  ).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("rfs-statuses-total")).toContainText("100");
  await expect(page.getByTestId("rfs-error")).toHaveCount(0);

  await page.getByTestId("rfs-opposition").fill("1");
  await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
  await expect(page.getByTestId("rfs-total")).toBeVisible({ timeout: 15_000 });

  // The die still reads as it fell — the modifier went to the total, not to
  // the dice, and not to how many were rolled.
  const faces = await facesUnder(page, "rfs-die-");
  expect(faces).toHaveLength(1);
  expect(faces[0]).toBeGreaterThanOrEqual(1);
  expect(faces[0]).toBeLessThanOrEqual(6);

  await expect(page.getByTestId("rfs-modifier")).toContainText("100");
  expect(Number((await page.getByTestId("rfs-total").innerText()).trim())).toBe(
    faces[0]! - 100,
  );

  // A −100 status cannot produce a positive total, so this beat nothing —
  // true of every roll, whatever the die showed.
  await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);
});

test("a status can neither create nor destroy an advancement", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { statusesEnabled: true });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-statuses")).toBeVisible({
    timeout: 15_000,
  });

  await page.getByTestId("rfs-status-name").fill("Doomed");
  await page.getByTestId("rfs-status-modifier").fill("-100");
  await page.getByTestId("rfs-status-add").click();
  await expect(page.getByTestId("rfs-statuses-total")).toContainText("100", {
    timeout: 15_000,
  });

  // Something to beat, and something impossible to beat: with a −100 status
  // every roll fails, so every roll pays exactly 1 XP. That is not incidental
  // — it is the completion signal. Nothing else on this page distinguishes
  // *this* roll's dice from the ones still on screen from the last one, and
  // without that distinction the test can read a stale six, find the prompt it
  // expects, and then have it replaced by the next roll mid-click.
  //
  // Judging the roll costs the claim nothing: a roll can fail and earn a new
  // skill in the same breath, which is exactly what a six here does.
  await page.getByTestId("rfs-opposition").fill("99");

  // A level-1 skill rolls one die, so "every die showed a six" is "the die
  // showed a six" — the one case that can be reached without a dice seed.
  // Sixty single-d6 rolls miss a six about once in 1.4 million.
  let sawASix = false;
  let xp = 0;
  for (let attempt = 1; attempt <= 60 && !sawASix; attempt += 1) {
    await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();

    // The ledger is what says this roll has landed. Only once it has moved is
    // anything else on the page this roll's.
    xp += 1;
    await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp), {
      timeout: 15_000,
    });

    const faces = await facesUnder(page, "rfs-die-");
    expect(faces).toHaveLength(1);
    const face = faces[0]!;

    // The total moved by the full −100 every time...
    expect(
      Number((await page.getByTestId("rfs-total").innerText()).trim()),
    ).toBe(face - 100);

    // ...and the advancement still reads the raw die. Both directions are
    // asserted on every attempt: a six must offer it despite the −100
    // (cannot destroy), and anything else must not (cannot create).
    await expect(page.getByTestId("rfs-advancement")).toHaveCount(
      face === 6 ? 1 : 0,
    );

    if (face === 6) {
      sawASix = true;
      await page.getByTestId("rfs-advancement-decline").click();
    }
  }
  expect(
    sawASix,
    "sixty single-d6 rolls without a six — the dice engine is not random",
  ).toBe(true);

  // Taking it off leaves the character where they started.
  const removes = page.locator('[data-testid^="rfs-status-remove-"]');
  await removes.first().click();
  await expect(page.getByTestId("rfs-statuses-none")).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("rfs-error")).toHaveCount(0);
});

/** Write one of an actor's system-data columns directly, to set a scene up. */
async function seedSystemData(
  page: Page,
  actorId: string,
  dataType: "trait_data" | "resource_data",
  data: Record<string, unknown>,
): Promise<void> {
  const written = await graphql<{
    data?: { updateActorSystemData?: { id: string } };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) {
          id
        }
      }
    `,
    { input: { actorId, gameSystemId: "roll_for_shoes", dataType, data } },
  );
  expect(
    written.data?.updateActorSystemData,
    `seed refused: ${JSON.stringify(written.errors ?? written)}`,
  ).toBeTruthy();
}

test("a full level tells the character there is no room, and 4 XP makes some", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const { worldId, actorId } = await createCharacter(page);
  await setExtras(page, worldId, { skillSlotsEnabled: true });

  // A character already holding the four skills level 2 allows. Seeded rather
  // than played out, because rolling four advancements without a dice seed is
  // not a test, it is a wait.
  await seedSystemData(page, actorId, "trait_data", {
    skills: [
      {
        id: STARTING_SKILL_ID,
        name: "Do Anything",
        level: 1,
        parentId: null,
      },
      ...[1, 2, 3, 4].map((n) => ({
        id: `full-${n}`,
        name: `Second Level Skill ${n}`,
        level: 2,
        parentId: STARTING_SKILL_ID,
      })),
    ],
  });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });

  // Nothing can be beaten, so every roll fails and pays 1 XP — which is both
  // how the experience gets banked and how each roll announces it landed.
  await page.getByTestId("rfs-opposition").fill("99");

  // Roll the level-1 skill until it comes up a six. An advancement off it
  // would sit at level 2, and level 2 is full.
  let xp = 0;
  let sawASix = false;
  for (let attempt = 1; attempt <= 60; attempt += 1) {
    await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
    xp += 1;
    await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp), {
      timeout: 15_000,
    });

    const [face] = await facesUnder(page, "rfs-die-");

    // The character is told, never silently denied (FR-030): where spec 061
    // would have offered the skill, this world offers the reason instead.
    await expect(page.getByTestId("rfs-advancement")).toHaveCount(0);
    await expect(page.getByTestId("rfs-advancement-no-room")).toHaveCount(
      face === 6 ? 1 : 0,
    );

    if (face === 6 && xp >= 4) {
      sawASix = true;
      break;
    }
    if (face === 6) {
      // A six too early to afford the slot. Dismiss it and bank more.
      await page.getByTestId("rfs-advancement-decline").click();
    }
  }
  expect(
    sawASix,
    "sixty rolls without a six banked alongside 4 XP — the dice engine is not random",
  ).toBe(true);

  // 4 XP for a level-2 slot: twice the level.
  const before = Number((await page.getByTestId("rfs-xp").innerText()).trim());
  await page.getByTestId("rfs-buy-slot").click();
  await expect(page.getByTestId("rfs-xp")).toHaveText(String(before - 4), {
    timeout: 15_000,
  });

  // The room bought is room: the same advancement is now offered for real.
  await expect(page.getByTestId("rfs-advancement")).toHaveCount(1, {
    timeout: 15_000,
  });
  await expect(page.getByTestId("rfs-advancement-no-room")).toHaveCount(0);

  // The new skill's id is generated, so it is counted rather than named: the
  // character held five skills and now holds six.
  const skillRows = page.locator('[data-testid^="rfs-skill-"]');
  await expect(skillRows).toHaveCount(5);
  await page
    .getByTestId("rfs-advancement-name")
    .fill("Kick A Cellar Door Down");
  await page.getByTestId("rfs-advancement-confirm").click();
  await expect(skillRows).toHaveCount(6, { timeout: 15_000 });
  await expect(page.getByTestId("rfs-error")).toHaveCount(0);
});

/** Another character in the same world. */
async function createActorIn(
  page: Page,
  worldId: string,
  label: string,
): Promise<string> {
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
        label,
        isNpc: false,
        gameSystemId: "roll_for_shoes",
      },
    },
  );
  return actor.data.createActor.id;
}

test("a world starts its own people with its own skills, and nobody already playing moves", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId: veteran } = await createCharacter(page);

  // Somebody who has already played: their skills are stored, which is what
  // makes them a character rather than a blank sheet.
  await seedSystemData(page, veteran, "trait_data", {
    skills: [
      { id: STARTING_SKILL_ID, name: "Do Anything", level: 1, parentId: null },
    ],
  });

  // Now the table changes what new people start with.
  await setExtras(page, worldId, {
    startingSkills: [
      { name: "Scavenge", level: 3 },
      { name: "Run Away", level: 1 },
    ],
  });

  const newcomer = await createActorIn(page, worldId, "Just Arrived");
  await page.goto(`/world/${worldId}/actor/${newcomer}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });

  // Two starting skills, both of them roots, at the levels the world named.
  await expect(page.getByTestId("rfs-skill-starting-skill")).toContainText(
    "Scavenge",
    { timeout: 15_000 },
  );
  await expect(page.getByTestId("rfs-skill-starting-skill-1")).toContainText(
    "Run Away",
  );
  await expect(page.locator('[data-testid^="rfs-skill-"]')).toHaveCount(2);

  // Level 3 is three dice, which is the point of declaring a level at all.
  await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
  await expect(page.getByTestId("rfs-total")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('[data-testid^="rfs-die-"]')).toHaveCount(3);
  await expect(page.getByTestId("rfs-error")).toHaveCount(0);

  // FR-039. The change reached new characters and nobody else.
  await page.goto(`/world/${worldId}/actor/${veteran}/edit`);
  await expect(page.getByTestId("rfs-sheet")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("rfs-skill-starting-skill")).toContainText(
    "Do Anything",
    { timeout: 15_000 },
  );
  await expect(page.getByTestId("rfs-skill-starting-skill-1")).toHaveCount(0);
  await expect(page.locator('[data-testid^="rfs-skill-"]')).toHaveCount(1);
});

test("three Extras at once, and each one still means what it meant alone", async ({
  page,
}) => {
  test.setTimeout(300_000);
  const { worldId, actorId } = await createCharacter(page);

  // Every other test in this file turns on exactly one setting, which proves
  // each rule but says nothing about what they do to one another. FR-002 says
  // enabling one must never silently change another, and the only way to find
  // out is to enable several and check each is still the rule it was.
  //
  // Three are on: a banded static target, statuses, and skill slots — and the
  // tie rule, which is what makes the target interesting. Slots are the
  // control: the level-2 cap has nothing to do with a level-1 roll, so if
  // anything about this roll changes because slots are on, that is the bug
  // FR-002 is about.
  await setExtras(page, worldId, {
    difficultyMode: "target",
    tieSucceeds: true,
    statusesEnabled: true,
    skillSlotsEnabled: true,
  });

  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(page.getByTestId("rfs-statuses")).toBeVisible({
    timeout: 15_000,
  });

  await page.getByTestId("rfs-status-name").fill("Shoeless");
  await page.getByTestId("rfs-status-modifier").fill("-2");
  await page.getByTestId("rfs-status-add").click();
  await expect(page.getByTestId("rfs-statuses-total")).toContainText("2", {
    timeout: 15_000,
  });
  await expect(page.getByTestId("rfs-error")).toHaveCount(0);

  // Easy is 3, and nothing is rolled for it — a static target stays static
  // with a status on the character, because the status is the character's
  // side of the comparison and the target is the table's.
  await page.getByTestId("rfs-table-band-easy").click();
  await expect(page.getByTestId("rfs-table-target")).toHaveText("3", {
    timeout: 15_000,
  });
  await expect(page.locator('[data-testid^="rfs-table-gm-die-"]')).toHaveCount(
    0,
  );

  // Unlike every other loop in this file, this one cannot use the XP ledger to
  // tell it that a roll has landed: the outcome being hunted is a *success*,
  // and successes pay nothing, so the ledger stands still exactly when the
  // interesting thing happens.
  //
  // So the page is reloaded before each roll, with the remembered attempt
  // forgotten first. The sheet keeps the last attempt for the browser tab so
  // that closing it loses nothing; clearing that and reloading leaves no
  // result on screen at all — and "no result, then a result" is a signal that
  // cannot be satisfied by the previous roll's dice the way "the total matches
  // the face" can.

  // One d6, less two, against 3. The total lands between −1 and 4, so the
  // three outcomes are all reachable and which one this roll took is dictated
  // by the face: 5 ties, 6 beats it, anything under 5 falls short. The tie is
  // what is being hunted, and every attempt judges the whole combination.
  let sawTie = false;
  let xp = 0;
  for (let attempt = 1; attempt <= 60 && !sawTie; attempt += 1) {
    await page.evaluate(() => sessionStorage.clear());
    await page.reload();
    await expect(page.getByTestId("rfs-sheet")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("rfs-total")).toHaveCount(0);

    // The Game Master set the band for the table, and the table's difficulty
    // is the server's: it is not chosen again, and its still being 3 after a
    // reload is itself worth asserting.
    await expect(page.getByTestId("rfs-table-target")).toHaveText("3", {
      timeout: 15_000,
    });

    await page.getByTestId(`rfs-roll-${STARTING_SKILL_ID}`).click();
    await expect(page.getByTestId("rfs-total")).toBeVisible({
      timeout: 15_000,
    });

    const faces = await facesUnder(page, "rfs-die-");
    expect(faces).toHaveLength(1);
    const face = faces[0]!;
    expect(
      Number((await page.getByTestId("rfs-total").innerText()).trim()),
    ).toBe(face - 2);

    // Statuses × slots: one die, because the pool is the skill's level and a
    // cap on how many skills sit at a level is not a cap on the dice.
    await expect(page.getByTestId("rfs-modifier")).toContainText("2");

    if (face >= 5) {
      // 5 is the tie the tie rule rescues; 6 beats the target outright. Both
      // are successes and neither pays.
      await expect(page.getByTestId("rfs-result")).toContainText(/success/i);
      await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp));
      if (face === 5) {
        sawTie = true;
      }
    } else {
      // Statuses × the tie rule: a status can push a roll under the target,
      // and the tie rule does not rescue what is merely close. The XP still
      // arrives, which is the tie rule staying a rule about *ties*.
      xp += 1;
      await expect(page.getByTestId("rfs-result")).toContainText(/fail/i);
      await expect(page.getByTestId("rfs-xp")).toHaveText(String(xp), {
        timeout: 8_000,
      });
    }

    await expect(page.getByTestId("rfs-error")).toHaveCount(0);
  }

  expect(
    sawTie,
    "sixty d6 without a five — the dice engine is not random",
  ).toBe(true);
});
