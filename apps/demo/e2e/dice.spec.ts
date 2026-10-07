import { expect, test, type Page } from "@playwright/test";
import { data, enterPlay, openDemo, WORLD_ID } from "./support";

/**
 * Spec 074 / spec 014: the demo rolls with the server's dice. Both places a
 * Game Master rolls from — the dice panel and a character in the dock — reach
 * the same `rollDice`, and each roll lands in the history the server keeps.
 */

let page: Page;
let outside: string[];

const HISTORY = `query ($worldId: UUID!) {
  worldRollRecords(worldId: $worldId, limit: 500) {
    resolution { formula resultValue dice { finalValue kept } }
  }
}`;

type Record = {
  resolution: {
    formula: string;
    resultValue: number;
    dice: { finalValue: number; kept: boolean }[];
  };
};

async function history(): Promise<Record[]> {
  return (
    await data<{ worldRollRecords: Record[] }>(page, HISTORY, {
      worldId: WORLD_ID,
    })
  ).worldRollRecords;
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openDemo(browser, baseURL));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

test("the dice panel rolls a formula, and the roll is in the history", async () => {
  await enterPlay(page);
  const before = (await history()).length;
  await page.getByTestId("dice-formula-input").fill("4d6kh3 + 2");
  await page.getByTestId("dice-roll-button").click();
  await expect(page.getByTestId("dice-roll-result")).toBeVisible({
    timeout: 15_000,
  });
  const records = await history();
  expect(records).toHaveLength(before + 1);
  const { resolution } = records[0];
  expect(resolution.formula).toBe("4d6kh3 + 2");
  expect(resolution.dice).toHaveLength(4);
  const kept = resolution.dice.filter((die) => die.kept);
  expect(kept).toHaveLength(3);
  expect(resolution.resultValue).toBe(
    kept.reduce((sum, die) => sum + die.finalValue, 0) + 2,
  );
  await expect(page.getByTestId("dice-roll-result")).toContainText(
    String(resolution.resultValue),
  );
});

test("a formula the server would refuse is refused, and rolls nothing", async () => {
  const before = (await history()).length;
  await page.getByTestId("dice-formula-input").fill("1d20 +");
  await page.getByTestId("dice-roll-button").click();
  await expect(page.getByTestId("dice-roll-error")).toBeVisible();
  expect(await history()).toHaveLength(before);
});

test("a character in the dock rolls from its sheet", async () => {
  const { worldActors } = await data<{
    worldActors: { id: string; label: string }[];
  }>(
    page,
    "query ($worldId: UUID!) { worldActors(worldId: $worldId) { id label } }",
    { worldId: WORLD_ID },
  );
  const fighter = worldActors.find((a) => a.label === "Brannoc Stoneward");
  if (!fighter) throw new Error("the fighter is in the cast");
  const before = (await history()).length;
  // The sheet opens in the dock for whoever is playing the character, which
  // in the demo is the player (the Game Master's View is a new tab).
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await page.getByTestId("world-dock-tab-actors").click();
  await page.getByTestId(`actor-view-${fighter.id}`).click();
  await expect(page.getByTestId("in-pane-sheet-body")).toBeVisible({
    timeout: 15_000,
  });
  // Strength 16, written as the sheet's stat roll: `1d20+16`.
  await page.getByTestId("in-pane-roll-stat-strength").click();
  await expect(page.getByTestId("in-pane-roll-result")).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("in-pane-roll-error")).toHaveCount(0);
  // The history is the Game Master's to read, as on the server.
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
  const records = await history();
  expect(records).toHaveLength(before + 1);
  expect(records[0].resolution.formula).toBe("1d20+16");
  const d20 = records[0].resolution.dice[0].finalValue;
  expect(d20).toBeGreaterThanOrEqual(1);
  expect(d20).toBeLessThanOrEqual(20);
  expect(records[0].resolution.resultValue).toBe(d20 + 16);
});
