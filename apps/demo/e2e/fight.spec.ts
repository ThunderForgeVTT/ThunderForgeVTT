import { expect, test, type Page } from "@playwright/test";

/**
 * Spec 079: a fight in the browser, as the demo ships it. The Game Master
 * starts a fight on the Grassy Path Ambush, the fighter swings at a goblin,
 * the order comes round again, a goblin goes down, a reload keeps all of it,
 * and a player is told no monster's hit points.
 *
 * Every rule is the combat crate's, compiled to wasm; nothing here has a
 * server to ask. The dice are seeded (T046) so the run is the same each time,
 * and each step's answer is timed from the click to the page showing it
 * (T050: under half a second).
 */

const WORLD_ID = "d0000000-0000-4000-0002-000000000001";
const AMBUSH = "d0000000-0000-4000-0003-000000000001";
const DICE_SEED_KEY = "thunderforge-demo:dice-seed";
/** SC of spec 079: a step's result is on the page within half a second. */
const STEP_BUDGET_MS = 500;

type Combatant = {
  id: string;
  label: string;
  tokenId: string | null;
  actorId: string | null;
  initiative: number;
  active: boolean;
};

let page: Page;
const timings: Record<string, number> = {};

async function ask<T>(query: string, variables: object = {}) {
  const answer = (await page.evaluate(
    async ([query, variables]) => {
      const response = await fetch("/api/graphql", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ query, variables }),
      });
      return response.json();
    },
    [query, variables] as const,
  )) as { data?: T; errors?: { message: string }[] };
  if (!answer.data) throw new Error(JSON.stringify(answer.errors));
  return answer.data;
}

async function combatants(): Promise<Combatant[]> {
  const { activeCombat } = await ask<{
    activeCombat: { combatants: Combatant[] } | null;
  }>(
    `query ($w: UUID!) { activeCombat(worldId: $w) {
      combatants { id label tokenId actorId initiative active } } }`,
    { w: WORLD_ID },
  );
  return activeCombat?.combatants ?? [];
}

/**
 * Do `act`, then wait for `selector` to show text matching `pattern`,
 * checked every 16 ms in the page; returns how long that took.
 */
async function timed(
  step: string,
  act: () => Promise<unknown>,
  selector: string,
  pattern: RegExp,
): Promise<number> {
  const started = await page.evaluate(() => performance.now());
  await act();
  await page.waitForFunction(
    ([selector, source, flags]) => {
      const re = new RegExp(source, flags);
      return [...document.querySelectorAll(selector)].some((el) =>
        re.test(el.textContent ?? ""),
      );
    },
    [selector, pattern.source, pattern.flags] as const,
    { polling: 16, timeout: 10_000 },
  );
  const took = (await page.evaluate(() => performance.now())) - started;
  timings[step] = Math.round(took);
  return took;
}

const row = (label: string) =>
  page.getByTestId("combatant-row").filter({ hasText: label });

async function openCombat() {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  if ((await page.getByTestId("combat-panel").count()) === 0) {
    await page.getByTestId("world-dock-tab-combat").click();
  }
  await expect(page.getByTestId("combat-panel")).toBeVisible();
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser }) => {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  page = await context.newPage();
  await page.addInitScript(([key]) => window.localStorage.setItem(key, "79"), [
    DICE_SEED_KEY,
  ] as const);
});

test.afterAll(async () => {
  // eslint-disable-next-line no-console
  console.log(`spec 079 step timings (ms): ${JSON.stringify(timings)}`);
  await page.context().close();
});

test("the Game Master starts a fight, and the order is rolled and shown", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  await openCombat();
  expect(
    await timed(
      "start",
      () => page.getByTestId("start-combat-button").click(),
      '[data-testid="combat-round-counter"]',
      /Round 1/,
    ),
  ).toBeLessThan(STEP_BUDGET_MS);

  for (const label of ["Brannoc Stoneward", "Goblin Warrior", "Dire Wolf"]) {
    const before = await page.getByTestId("combatant-row").count();
    await page.getByTestId("combat-add-actor-select").selectOption({ label });
    const took = await timed(
      `add ${label}`,
      () => page.getByTestId("combat-add-button").click(),
      '[data-testid="combatant-list"]',
      new RegExp(label === "Goblin Warrior" ? "Goblin 1" : label),
    );
    expect(took).toBeLessThan(STEP_BUDGET_MS);
    await expect(page.getByTestId("combatant-row")).toHaveCount(before + 1);
  }

  // The order on the page is the rolled one, highest first.
  const shown = await page
    .getByTestId("combatant-row")
    .locator('input[aria-label^="Initiative for"]')
    .evaluateAll((inputs) =>
      inputs.map((input) => Number((input as HTMLInputElement).value)),
    );
  expect(shown).toHaveLength(3);
  expect(shown).toEqual([...shown].sort((a, b) => b - a));
  expect((await combatants()).map((c) => c.initiative)).toEqual(shown);
});

test("Brannoc attacks a goblin, and the log says hit or miss against its defence", async () => {
  // Round the order to Brannoc's turn: his attack is his action.
  for (let i = 0; i < 3; i += 1) {
    if ((await row("Brannoc").getAttribute("data-active-turn")) === "true") {
      break;
    }
    await page.getByTestId("advance-turn-button").click();
  }
  await expect(row("Brannoc")).toHaveAttribute("data-active-turn", "true");

  const seats = await combatants();
  const brannoc = seats.find((c) => c.label === "Brannoc Stoneward")!;
  const goblin = seats.find((c) => c.label === "Goblin 1")!;
  const { actorAbilities } = await ask<{
    actorAbilities: { abilityId: string; abilityName: string }[];
  }>(
    "query ($a: UUID!) { actorAbilities(actorId: $a) { abilityId abilityName } }",
    { a: brannoc.actorId },
  );
  const sword = actorAbilities.find((a) => a.abilityName === "Longsword")!;
  const hp = async () => {
    const { tokenStatus } = await ask<{
      tokenStatus: {
        tokenId: string;
        resources: { entries: { current: number }[] | null }[];
      }[];
    }>(
      `query ($s: UUID!) { tokenStatus(sceneId: $s) {
        tokenId resources { entries { current } } } }`,
      { s: AMBUSH },
    );
    const status = tokenStatus.find((t) => t.tokenId === goblin.tokenId);
    return status!.resources[0].entries![0].current;
  };
  const before = await hp();

  let attack: {
    outcome: string;
    defence: number;
    toHit: { resultValue: number };
    damage: { resultValue: number } | null;
    offer: { id: string; amount: number; status: string } | null;
  } = undefined as never;
  const took = await timed(
    "attack",
    async () => {
      const made = await ask<{ makeAttack: (typeof attack)[] }>(
        `mutation ($i: AttackInput!) { makeAttack(input: $i) {
          outcome defence toHit { resultValue } damage { resultValue }
          offer { id amount status } } }`,
        {
          i: {
            attackerTokenId: brannoc.tokenId,
            abilityId: sword.abilityId,
            targetTokenId: goblin.tokenId,
          },
        },
      );
      attack = made.makeAttack[0];
    },
    '[data-testid="attack-log-entry"]',
    /Goblin 1/,
  );
  expect(took).toBeLessThan(STEP_BUDGET_MS);
  expect(attack.defence).toBe(15);
  expect(attack.outcome).toBe(attack.toHit.resultValue >= 15 ? "HIT" : "MISS");
  await expect(
    page
      .getByTestId("attack-log-entry")
      .first()
      .getByTestId("attack-log-outcome"),
  ).toContainText(attack.outcome === "HIT" ? /hit/i : /miss/i);

  if (attack.outcome === "HIT") {
    const offer = attack.offer!;
    if (offer.status === "PENDING") {
      // The Game Master takes the damage from the offer on the table.
      await page.getByTestId("offer-take").first().click();
    }
    await expect.poll(hp).toBe(Math.max(0, before - offer.amount));
  } else {
    expect(attack.damage).toBeNull();
    expect(await hp()).toBe(before);
  }
});

test("the order comes round into round 2", async () => {
  const counter = page.getByTestId("combat-round-counter");
  for (let i = 0; i < 4; i += 1) {
    if (/Round 2/.test((await counter.textContent()) ?? "")) break;
    const from = await counter.textContent();
    const active = await page
      .locator('[data-testid="combatant-row"][data-active-turn="true"]')
      .textContent();
    await page.getByTestId("advance-turn-button").click();
    await expect
      .poll(async () => [
        await counter.textContent(),
        await page
          .locator('[data-testid="combatant-row"][data-active-turn="true"]')
          .textContent(),
      ])
      .not.toEqual([from, active]);
  }
  await expect(page.getByTestId("combat-round-counter")).toHaveText(/Round 2/);
  // And one more turn, timed: the active row moves within the budget.
  const active = async () =>
    page
      .locator('[data-testid="combatant-row"][data-active-turn="true"]')
      .textContent();
  const from = await active();
  const started = await page.evaluate(() => performance.now());
  await page.getByTestId("advance-turn-button").click();
  await page.waitForFunction(
    (from) =>
      document.querySelector(
        '[data-testid="combatant-row"][data-active-turn="true"]',
      )?.textContent !== from,
    from,
    { polling: 16, timeout: 10_000 },
  );
  timings["advance"] = Math.round(
    (await page.evaluate(() => performance.now())) - started,
  );
  expect(timings["advance"]).toBeLessThan(STEP_BUDGET_MS);
});

test("a goblin at 0 hit points shows as down", async () => {
  const goblin = row("Goblin 1");
  await goblin.getByTestId("combatant-hp-amount").fill("100");
  expect(
    await timed(
      "down",
      () => goblin.getByTestId("combatant-damage-button").click(),
      '[data-testid="combatant-row"] [data-testid="combatant-out"]',
      /Out: 0 hit points/,
    ),
  ).toBeLessThan(STEP_BUDGET_MS);
  await expect(goblin).toHaveAttribute("data-downed-by", "HIT_POINTS");
});

test("a reload keeps the fight where it was", async () => {
  await page.reload();
  await openCombat();
  await expect(page.getByTestId("combat-round-counter")).toHaveText(/Round 2/);
  await expect(page.getByTestId("combatant-row")).toHaveCount(3);
  await expect(row("Goblin 1").getByTestId("combatant-out")).toHaveText(
    "Out: 0 hit points",
  );
  await expect(page.getByTestId("attack-log-entry").first()).toContainText(
    "Goblin 1",
  );
});

test("a player sees the fight but no monster's hit points or hidden name", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await openCombat();
  await expect(page.getByTestId("combatant-row")).toHaveCount(3);
  await expect(page.getByTestId("combatant-list")).toContainText(
    "Brannoc Stoneward",
  );
  await expect(page.getByTestId("combatant-list")).toContainText("Unknown");
  await expect(page.getByTestId("combatant-list")).not.toContainText(
    "Goblin 1",
  );
  await expect(page.getByTestId("combatant-list")).not.toContainText(
    "Dire Wolf",
  );
  // No hit-point controls, and no monster's figure on the wire.
  await expect(page.getByTestId("combatant-hit-points")).toHaveCount(0);
  await expect(page.getByTestId("advance-turn-button")).toHaveCount(0);
  const { tokenStatus } = await ask<{
    tokenStatus: {
      tokenId: string;
      resources: { disclosure: string; entries: unknown }[];
    }[];
  }>(
    `query ($s: UUID!) { tokenStatus(sceneId: $s) {
      tokenId resources { disclosure entries { current max } } } }`,
    { s: AMBUSH },
  );
  const seats = await combatants();
  const monsters = seats
    .filter((c) => c.label !== "Brannoc Stoneward")
    .map((c) => c.tokenId);
  for (const status of tokenStatus) {
    if (!monsters.includes(status.tokenId)) continue;
    for (const resource of status.resources) {
      expect(resource.disclosure).not.toBe("visible");
      expect(resource.entries).toBeNull();
    }
  }
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
});
