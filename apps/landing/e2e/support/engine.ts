import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { expect, type Page } from "@playwright/test";
import { initSync, roll } from "@thunderforge/dice";
import { diceCommand, type Resolution } from "../../../../packages/dice-throw/src/index.ts";

/**
 * Spec 090 and 091: reaching the engine the page booted, from a test.
 *
 * The engine is the demo's build, served at `/demo/` by the preview server.
 * Importing the URL the page imported returns the instance that is running.
 */

export const MANIFEST = new URL("../../../../data/demo/engine.json", import.meta.url).pathname;
export const BUILT = existsSync(MANIFEST);
export const BOOT_TIMEOUT = 300_000;

const require = createRequire(import.meta.url);
initSync({ module: readFileSync(require.resolve("@thunderforge/dice/dice_bg.wasm")) });

/** The crate's roll for a fixed seed, as the frame would throw it. */
export function seeded(formula: string, s: number, id: string) {
  const seed = new Uint32Array([s, Math.imul(s, 0x9e3779b1), Math.imul(s, 0x85ebca6b), 1]);
  const detail = JSON.parse(roll(formula, "{}", seed)) as Resolution;
  return { detail, json: JSON.stringify(diceCommand(detail, { rollerName: "Test", id })) };
}

export interface Landed {
  rollId: string;
  skipped: boolean;
  reducedMotion: boolean;
  dice: Array<{ sides: number | string; face: number; kept: boolean }>;
}

/** Call one of the running engine's exports; JSON answers come back parsed. */
export async function engine<T>(page: Page, call: string, arg?: string): Promise<T> {
  return page.evaluate(
    async ([call, arg]) => {
      const m = await (await fetch("/demo/engine.json")).json();
      const e = await import(`/demo/${m.js}`);
      const out = arg === undefined ? e[call]() : e[call](arg);
      return typeof out === "string" && /^[[{]/.test(out)
        ? JSON.parse(out)
        : out;
    },
    [call, arg] as const,
  ) as Promise<T>;
}

export const landed = (page: Page) => engine<Landed[]>(page, "dice_landed");
export const frames = async (page: Page) =>
  (await engine<{ frames: number }>(page, "engine_stats")).frames;

export async function landedOf(page: Page, id: string): Promise<Landed> {
  let found: Landed | undefined;
  await expect
    .poll(async () => (found = (await landed(page)).find((t) => t.rollId === id)), {
      timeout: 30_000,
    })
    .toBeTruthy();
  return found!;
}

export const diceFrame = (page: Page) => page.locator("#dice-table");
export const throwButton = (page: Page) =>
  diceFrame(page).getByRole("button", { name: "Throw it on the table" });

/** Click the dice frame's button and wait for the engine to run. */
export async function bootDiceFrame(page: Page) {
  await diceFrame(page).scrollIntoViewIfNeeded();
  await throwButton(page).click();
  await expect(diceFrame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
}

export const isEngineFile = (url: string) => new URL(url).pathname.startsWith("/demo/");

export const readManifest = () => JSON.parse(readFileSync(MANIFEST, "utf8"));
