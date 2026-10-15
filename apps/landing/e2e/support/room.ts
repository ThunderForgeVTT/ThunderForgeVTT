import { expect, type Page } from "@playwright/test";
import { HERO_ID, toWorld, type MapFile, type Point } from "@thunderforge/encounter";
import { readFileSync } from "node:fs";
import { roomScript } from "../../src/room/script.ts";
import { BOOT_TIMEOUT, engine } from "./engine.ts";

/** Spec 091: the room, read from the DOM and from the engine. */

export const script = roomScript(
  JSON.parse(readFileSync(new URL("../../public/maps/demo.json", import.meta.url), "utf8")) as MapFile,
);
const grid = script.map.gridSize;

/** The page's seeds, from a counter: the same rolls on every run. */
export const FIXED_SEEDS = () => {
  let n = 1;
  crypto.getRandomValues = <T extends ArrayBufferView | null>(array: T): T => {
    const a = array as unknown as Uint32Array;
    for (let i = 0; i < a.length; i += 1) a[i] = (n++ * 2654435761) >>> 0;
    return array;
  };
};

export const room = (page: Page) => page.locator("#room");
export const roomFrame = (page: Page) => page.locator("#room-table");
export const said = (page: Page) => room(page).locator(".room-said");
export const square = (page: Page) =>
  room(page).locator(".room-layout").getAttribute("data-square");
export const pad = (page: Page, dir: string) =>
  room(page).getByRole("group", { name: "Move one square" }).getByRole("button", { name: dir });
export const goal = (page: Page, id: string) => room(page).locator(`[data-goal="${id}"]`);

/** Every text the live region took, in order, from its mutations. */
export async function recordSaid(page: Page) {
  await page.evaluate(() => {
    const el = document.querySelector("#room .room-said")!;
    const w = window as unknown as { saidLog: string[] };
    w.saidLog = [];
    new MutationObserver(() => {
      const t = (el.textContent ?? "").replace(/ $/, "");
      if (t) w.saidLog.push(t);
    }).observe(el, { childList: true, characterData: true, subtree: true });
  });
}
export const saidLog = (page: Page) =>
  page.evaluate(() => (window as unknown as { saidLog: string[] }).saidLog);

/** Press a pad button and hear exactly one line back. */
export async function stepAndHear(page: Page, dir: string, words: RegExp) {
  await pressAndHear(page, pad(page, dir), words);
}
export async function pressAndHear(
  page: Page,
  button: ReturnType<Page["locator"]>,
  words: RegExp,
) {
  const before = (await saidLog(page)).length;
  await button.click();
  await expect.poll(async () => (await saidLog(page)).slice(before)).toHaveLength(1);
  expect((await saidLog(page)).at(-1)).toMatch(words);
}

/**
 * Bring the room on screen and wait for the engine to run in its frame. It
 * takes the table by itself; under reduced motion the visitor still asks.
 */
export async function bootRoom(page: Page) {
  // The table and its controls on one screen: a click scrolls to its
  // button, and a frame scrolled out of view pauses the engine (FR-017).
  const size = page.viewportSize();
  if (size && size.height < 1400) await page.setViewportSize({ width: size.width, height: 1400 });
  await roomFrame(page).evaluate((el) => el.scrollIntoView({ block: "start" }));
  const play = roomFrame(page).getByRole("button", { name: "Play the room" });
  if (await page.evaluate(() => matchMedia("(prefers-reduced-motion: reduce)").matches)) {
    await play.click();
  }
  await expect(roomFrame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
}

interface Footprint {
  tokenId: string;
  x: number;
  y: number;
}

/** Where the engine has the hero, in squares. */
export async function engineHero(page: Page): Promise<Point | undefined> {
  const all = await engine<Footprint[]>(page, "token_footprints");
  const h = all.find((t) => t.tokenId === HERO_ID);
  return h && { x: h.x / grid, y: h.y / grid };
}

/** Whether the viewer sees a square, by the engine's own sight probe. */
export async function seen(page: Page, at: Point) {
  const w = toWorld(at, grid);
  return page.evaluate(
    async ([x, y]) => {
      const m = await (await fetch("/demo/engine.json")).json();
      const e = await import(`/demo/${m.js}`);
      return (JSON.parse(e.sight_probe(x, y)) as { seen: boolean }).seen;
    },
    [w.x, w.y] as const,
  );
}

/** Plays all four goals with the page's controls alone. */
export async function playThrough(page: Page) {
  await room(page).scrollIntoViewIfNeeded();
  // The room renders when it nears the screen.
  await expect(said(page)).toBeAttached();
  await recordSaid(page);
  expect(await square(page)).toBe("-3,1.5");

  await expect(goal(page, "door-reached")).toContainText("(not yet)");
  for (const y of ["0.5", "-0.5", "-1.5"]) {
    await stepAndHear(page, "South", /door to the hall/);
    expect(await square(page)).toBe(`-3,${y}`);
  }
  // Within reach of the door is having reached it; the bump teaches the rule.
  await expect(goal(page, "door-reached")).toContainText("(done)");
  await stepAndHear(page, "South", /^The door is shut\. Walls and closed doors refuse a move/);
  expect(await square(page)).toBe("-3,-1.5");

  const door = room(page).getByRole("button", { name: "Open the door to the hall" });
  await expect(door).toBeEnabled();
  const before = (await saidLog(page)).length;
  await door.click();
  await expect.poll(async () => (await saidLog(page)).slice(before)).toHaveLength(1);
  expect((await saidLog(page)).at(-1)).toMatch(/^You open the door to the hall\./);
  await expect(goal(page, "door-opened")).toContainText("(done)");

  await stepAndHear(page, "South", /hall/);
  await stepAndHear(page, "South", /hall/);
  await stepAndHear(page, "East", /^You find the chest\. It is locked: roll 1d20\+2 and meet 12/);
  await expect(goal(page, "chest-found")).toContainText("(done)");

  const roll = room(page).getByRole("button", { name: "Roll 1d20+2" });
  for (let i = 0; i < 30; i += 1) {
    const n = (await saidLog(page)).length;
    await roll.click();
    await expect.poll(async () => (await saidLog(page)).length).toBe(n + 1);
    const line = (await saidLog(page)).at(-1)!;
    if (/The chest opens\.$/.test(line)) break;
    expect(line).toMatch(/^You rolled \d+\. Not enough: you need 12\. Roll again\.$/);
  }
  await expect(goal(page, "chest-opened")).toContainText("(done)");
  await expect(roll).toBeDisabled();
  await expect(room(page).getByRole("link", { name: "Open the demo" })).toBeVisible();
}
