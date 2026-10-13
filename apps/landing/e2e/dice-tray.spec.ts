import { expect, test, type Locator, type Page } from "@playwright/test";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * Spec 090 T007: the dice tray throws the real dice (the Rust crate, as
 * wasm) and shows what it threw. Every roll is seeded from crypto, so these
 * tests assert how the dice, the total and the history relate, never what
 * they came up.
 */

interface Preset {
  label: string;
  formula: string;
  /** Flat modifier written into the formula. */
  modifier: number;
  /** How many dice the visitor should see, and how many of them count. */
  thrown: number;
  kept: number;
  /** Lowest and highest face one die can show. */
  faces: [number, number];
}

const PRESETS: Preset[] = [
  { label: "Attack", formula: "1d20+5", modifier: 5, thrown: 1, kept: 1, faces: [1, 20] },
  { label: "Advantage", formula: "2d20kh1+5", modifier: 5, thrown: 2, kept: 1, faces: [1, 20] },
  { label: "Ability score", formula: "4d6kh3", modifier: 0, thrown: 4, kept: 3, faces: [1, 6] },
  { label: "Fireball", formula: "8d6", modifier: 0, thrown: 8, kept: 8, faces: [1, 6] },
  { label: "Fate", formula: "4dF", modifier: 0, thrown: 4, kept: 4, faces: [-1, 1] },
];

/** A die as the screen reader hears it: its face, and whether it was dropped. */
interface Shown {
  value: number;
  dropped: boolean;
}

function readFace(label: string): Shown {
  const dropped = label.endsWith(", dropped");
  const face = dropped ? label.slice(0, -", dropped".length) : label;
  const fate: Record<string, number> = { "+": 1, "−": -1, "·": 0 };
  const value = face in fate ? fate[face] : Number(face);
  expect(Number.isInteger(value), `die face "${face}" is not a number or a Fate face`).toBe(true);
  return { value, dropped };
}

function tray(page: Page) {
  const root = page.locator(".tray");
  return {
    root,
    preset: (p: Preset) =>
      root.getByRole("group", { name: "Example rolls" }).getByRole("button", {
        name: `${p.label} ${p.formula}`,
      }),
    input: root.getByLabel("Or write your own"),
    rollButton: root.getByRole("button", { name: "Roll", exact: true }),
    dice: root.locator('ul[aria-label="Dice rolled"] > li'),
    total: root.locator(".total .total-num"),
    shownFormula: root.locator(".total code"),
    error: root.locator(".tray-error"),
    history: root.locator('ol[aria-label="Earlier rolls"] > li'),
  };
}

type Tray = ReturnType<typeof tray>;

/** What the tray shows once a roll of `formula` has landed. */
async function readRoll(t: Tray, formula: string) {
  await expect(t.error).toHaveCount(0);
  await expect(t.shownFormula).toHaveText(formula);
  const labels = await t.dice.evaluateAll((els) => els.map((el) => el.getAttribute("aria-label") ?? ""));
  const faces = await t.dice.evaluateAll((els) =>
    els.map((el) => el.querySelector("span")?.textContent ?? ""),
  );
  const total = Number((await t.total.textContent())?.trim());
  expect(Number.isInteger(total), "the total is a whole number").toBe(true);
  return { dice: labels.map(readFace), labels, faces, total };
}

function expectConsistent(p: Preset, roll: Awaited<ReturnType<typeof readRoll>>) {
  const { dice, total } = roll;
  expect(dice, `${p.formula} shows every die thrown`).toHaveLength(p.thrown);
  for (const d of dice) {
    expect(d.value).toBeGreaterThanOrEqual(p.faces[0]);
    expect(d.value).toBeLessThanOrEqual(p.faces[1]);
  }
  const kept = dice.filter((d) => !d.dropped);
  const dropped = dice.filter((d) => d.dropped);
  expect(kept, `${p.formula} keeps ${p.kept}`).toHaveLength(p.kept);
  // Keep-highest keeps the best: nothing dropped beats anything kept.
  if (dropped.length) {
    const lowestKept = Math.min(...kept.map((d) => d.value));
    for (const d of dropped) expect(d.value).toBeLessThanOrEqual(lowestKept);
  }
  const sum = kept.reduce((s, d) => s + d.value, 0);
  expect(total, `${p.formula}: kept dice ${kept.map((d) => d.value).join(", ")} + ${p.modifier}`).toBe(
    sum + p.modifier,
  );
  // The face drawn on the die is the one announced.
  roll.labels.forEach((label, i) => {
    expect(label.replace(/, dropped$/, "")).toBe(roll.faces[i]);
  });
}

test("every example roll shows dice that add up to its total", async ({ page }) => {
  const opened = await openLanding(page);
  const t = tray(page);
  for (const p of PRESETS) {
    await t.preset(p).click();
    // The preset becomes the formula in the field, ready to roll again.
    await expect(t.input).toHaveValue(p.formula);
    expectConsistent(p, await readRoll(t, p.formula));
  }
  expectClean(opened);
});

test("a written formula rolls with the Roll button and with Enter", async ({ page }) => {
  const opened = await openLanding(page);
  const t = tray(page);
  const custom: Preset = { label: "", formula: "3d8+2", modifier: 2, thrown: 3, kept: 3, faces: [1, 8] };

  await t.input.fill(custom.formula);
  await t.rollButton.click();
  const first = await readRoll(t, custom.formula);
  expectConsistent(custom, first);
  await expect(t.history).toHaveCount(0);

  const enter: Preset = { label: "", formula: "2d10-1", modifier: -1, thrown: 2, kept: 2, faces: [1, 10] };
  await t.input.fill(enter.formula);
  await t.input.press("Enter");
  expectConsistent(enter, await readRoll(t, enter.formula));
  // The first roll moved into the history: proof Enter rolled, not just redrew.
  await expect(t.history).toHaveText([`${custom.formula} → ${first.total}`]);
  expectClean(opened);
});

test("nonsense is refused and nothing is rolled", async ({ page }) => {
  const opened = await openLanding(page);
  const t = tray(page);
  const attack = PRESETS[0];

  await t.preset(attack).click();
  const before = await readRoll(t, attack.formula);

  await t.input.fill("nonsense");
  await t.rollButton.click();
  await expect(t.error).toHaveText('"nonsense" is not a formula the dice understand.');
  await expect(t.dice).toHaveCount(0);
  await expect(t.total).toHaveCount(0);
  // Nothing new went into the history either.
  await expect(t.history).toHaveCount(0);

  // The next good roll clears the refusal, and the history holds only the
  // roll before it.
  await t.input.fill(attack.formula);
  await t.input.press("Enter");
  await expect(t.error).toHaveCount(0);
  await expect(t.history).toHaveText([`${attack.formula} → ${before.total}`]);
  expectConsistent(attack, await readRoll(t, attack.formula));
  expectClean(opened);
});

test("the history keeps the last three earlier rolls, newest first", async ({ page }) => {
  const opened = await openLanding(page);
  const t = tray(page);
  await expect(t.history).toHaveCount(0);

  const thrown: string[] = [];
  for (const [i, p] of PRESETS.entries()) {
    await t.preset(p).click();
    const roll = await readRoll(t, p.formula);
    // The current roll is on the felt, not in the list; at most three
    // earlier rolls are kept.
    await expect(t.history).toHaveCount(Math.min(i, 3));
    thrown.unshift(`${p.formula} → ${roll.total}`);
    await expect(t.history).toHaveText(thrown.slice(1, 4));
  }
  expectClean(opened);
});
