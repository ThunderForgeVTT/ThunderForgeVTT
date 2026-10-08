import type { Browser } from "@playwright/test";

import { expect, type Page } from "./test";
import {
  clickPlay,
  graphql,
  inviteAndJoinAsPlayer,
  openDockTab,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./helpers";

/**
 * Spec 081: what a page's board and chat say about rolls.
 *
 * "Animated" is read from `__engineProbe.dicePlayed`, the dice each board
 * handed to the engine. It is recorded only after the engine took the
 * command, so it is the board's own account, not the panel's.
 */

/** Waits until the board's engine probe is installed. */
export async function waitForEngineProbe(page: Page): Promise<void> {
  await expect
    .poll(
      () =>
        page.evaluate(
          () =>
            typeof (window as unknown as { __engineProbe?: unknown })
              .__engineProbe === "object",
        ),
      { timeout: 60_000 },
    )
    .toBe(true);
}

/** Every roll this board has animated, each as its dice' final values. */
export function dicePlayed(page: Page): Promise<number[][]> {
  return page.evaluate(() =>
    (
      window as unknown as { __engineProbe: { dicePlayed: () => number[][] } }
    ).__engineProbe.dicePlayed(),
  );
}

/** One die of a landed throw, as the engine drew it (spec 083). */
export interface LandedDie {
  /** A number, or `"F"` for Fate and `"C"` for a coin. */
  sides: number | "F" | "C";
  face: number;
  kept: boolean;
  succeeded: boolean | null;
  rerolled: number[];
  clamped: number | null;
  explosionOf: number | null;
  /** Screen pixels from the throw's anchor, rounded. */
  restingPlace: [number, number];
}

/** A throw the engine landed or skipped (contracts/engine-dice.md). */
export interface LandedThrow {
  rollId: string;
  skipped: boolean;
  reducedMotion: boolean;
  readout: string | null;
  chip: string | null;
  dice: LandedDie[];
}

export interface DiceTimings {
  tumbleMs: number;
  stepMs: number;
  holdMs: number;
  fadeMs: number;
  reducedMs: number;
}

/**
 * Spec 083: every throw this board's engine landed or skipped, oldest first,
 * from the engine's own log (`dice_landed()`). Unlike `dicePlayed`, it is
 * written when the dice come to rest, so it says what the board drew.
 */
export function diceLanded(page: Page): Promise<LandedThrow[]> {
  return page.evaluate(() =>
    (
      window as unknown as {
        __engineProbe: { diceLanded: () => LandedThrow[] };
      }
    ).__engineProbe.diceLanded(),
  );
}

/** How many dice entities the engine has alive: 0 once every throw faded. */
export function diceEntities(page: Page): Promise<number> {
  return page.evaluate(() =>
    (
      window as unknown as { __engineProbe: { diceEntities: () => number } }
    ).__engineProbe.diceEntities(),
  );
}

/** The engine's own throw timings, the ones the roll panel waits on. */
export async function diceTimings(page: Page): Promise<DiceTimings> {
  const timings = await page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/diceThrow.ts"
    )) as typeof import("../../src/engine/bevy/diceThrow");
    return mod.engineDiceTimings();
  });
  if (!timings) throw new Error("the engine reports no dice timings");
  return timings;
}

/** Waits for the board to land (or skip) `count` throws past `before`. */
export async function nextLanded(
  page: Page,
  before: number,
  count = 1,
  timeout = 30_000,
): Promise<LandedThrow[]> {
  await expect
    .poll(async () => (await diceLanded(page)).length, { timeout })
    .toBeGreaterThanOrEqual(before + count);
  return (await diceLanded(page)).slice(before, before + count);
}

/** A roll as the server recorded it, read as `page`'s viewer. */
export interface ServerRoll {
  id: string;
  createdAt: string;
  bindings: { placeholder: string; value: number }[];
  resolution: {
    formula: string;
    resultKind: "TOTAL" | "SUCCESS_COUNT";
    resultValue: number;
    dice: {
      sidesKind: string;
      numericSides: number | null;
      rolls: number[];
      steps: ("REROLL" | "EXPLODE")[];
      kept: boolean;
      finalValue: number;
    }[];
  };
}

const SERVER_ROLL_FIELDS = `
  ... on WorldRoll {
    id
    createdAt
    bindings { placeholder value }
    resolution {
      formula
      resultKind
      resultValue
      dice { sidesKind numericSides rolls steps kept finalValue }
    }
  }
`;

/** One roll, read with `worldRoll`. */
export async function serverRoll(
  page: Page,
  worldId: string,
  rollId: string,
): Promise<ServerRoll> {
  const result = await graphql<{
    data?: { worldRoll: ServerRoll | null };
    errors?: unknown;
  }>(
    page,
    `query ($worldId: UUID!, $rollId: UUID!) {
      worldRoll(worldId: $worldId, rollId: $rollId) { ${SERVER_ROLL_FIELDS} }
    }`,
    { worldId, rollId },
  );
  const roll = result.data?.worldRoll;
  if (!roll?.id) {
    throw new Error(`no roll ${rollId}: ${JSON.stringify(result.errors)}`);
  }
  return roll;
}

/** The world's rolls, oldest first. */
export async function serverRolls(
  page: Page,
  worldId: string,
): Promise<ServerRoll[]> {
  const result = await graphql<{ data?: { worldRolls: ServerRoll[] } }>(
    page,
    `query ($worldId: UUID!) {
      worldRolls(worldId: $worldId, limit: 100) { ${SERVER_ROLL_FIELDS} }
    }`,
    { worldId },
  );
  return [...(result.data?.worldRolls ?? [])].reverse();
}

/**
 * Rolls `formula` in the open through the real mutation, the one the roller
 * calls, without the panel: for several rolls faster than a person clicks.
 */
export async function rollOnServer(
  page: Page,
  worldId: string,
  formula: string,
): Promise<void> {
  const result = await graphql<{ data?: unknown; errors?: unknown }>(
    page,
    `
      mutation ($input: RollDiceInput!) {
        rollDice(input: $input) {
          resultValue
        }
      }
    `,
    { input: { worldId, formula, visibility: "EVERYONE" } },
  );
  if (!result.data) {
    throw new Error(`${formula} was refused: ${JSON.stringify(result.errors)}`);
  }
}

/** Rolls `formula` from the page's dice roller, and waits for its result. */
export async function rollInRoller(page: Page, formula: string): Promise<void> {
  const panel = page.getByTestId("dice-roller-panel");
  await panel.getByTestId("dice-formula-input").fill(formula);
  await panel.getByTestId("dice-roll-button").click();
  await expect(panel.getByTestId("dice-roll-result")).toBeVisible({
    timeout: 15_000,
  });
}

/**
 * Moves the camera the way the application does, through the bound world
 * store's command bridge.
 */
export async function setCamera(
  page: Page,
  camera: { x: number; y: number; zoom?: number },
): Promise<void> {
  const sent = await page.evaluate(
    async (cmd) => {
      const bevy = (await import(
        /* @vite-ignore */ "/src/engine/bevy/index.ts"
      )) as typeof import("../../src/engine/bevy/index");
      const store = bevy.getBoundWorldStore();
      if (!store) return false;
      store.dispatch(cmd as never);
      return true;
    },
    { type: "set_camera", ...camera },
  );
  expect(sent, "the engine's world store must be bound").toBe(true);
}

export async function openChat(page: Page): Promise<void> {
  await openDockTab(page, "chat");
  await expect(page.getByTestId("chat-panel")).toBeVisible({ timeout: 15_000 });
}

/** The board is up, the socket is live and the probe is installed. */
export async function waitForPlayView(page: Page): Promise<void> {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("live-sync-reconnecting-indicator")).toBeHidden(
    { timeout: 30_000 },
  );
  await waitForEngineProbe(page);
}

export interface RollTable {
  worldId: string;
  gm: Page;
  /** Two players, each in their own browser context. */
  players: [Page, Page];
  close: () => Promise<void>;
}

/**
 * A GM and two players at one world's play view, chat open, no dice played
 * yet. Settles for the subscriptions the play view opens as it renders —
 * the socket being live is not every subscription being open (the same
 * measured settle as chat-panel.spec.ts).
 */
export async function seatRollTable(
  gm: Page,
  browser: Browser,
): Promise<RollTable> {
  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(
    gm,
    `E2E Rolls ${suffix}`,
    "e2erollgm",
  );
  const a = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2erollera");
  const b = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2erollerb");
  const close = async () => {
    await a.context().close();
    await b.context().close();
  };
  try {
    await clickPlay(gm);
    for (const page of [a, b]) await page.goto(`/world/${worldId}/play`);
    for (const page of [gm, a, b]) {
      await waitForPlayView(page);
      await openChat(page);
      expect(await dicePlayed(page)).toEqual([]);
    }
    await b.waitForTimeout(6_000);
  } catch (error) {
    await close();
    throw error;
  }
  return { worldId, gm, players: [a, b], close };
}

/**
 * Rolls `formula` from the page's dice roller for `visibility`, and answers
 * the total the roller was shown.
 */
export async function rollFromRoller(
  page: Page,
  formula: string,
  visibility: "EVERYONE" | "GM_EYES" | "GM_ONLY" = "EVERYONE",
): Promise<number> {
  const panel = page.getByTestId("dice-roller-panel");
  await panel.getByTestId("roll-visibility-picker").selectOption(visibility);
  await panel.getByTestId("dice-formula-input").fill(formula);
  await panel.getByTestId("dice-roll-button").click();
  const result = panel.getByTestId("dice-roll-result");
  await expect(result).toBeVisible({ timeout: 15_000 });
  const shown = await result.innerText();
  const escaped = formula.replace(/[+]/g, "\\+");
  const total = Number(new RegExp(`${escaped}:\\s*(-?\\d+)`).exec(shown)?.[1]);
  expect(Number.isFinite(total), `a total in "${shown}"`).toBe(true);
  return total;
}

/**
 * Everything the server sends a page from now on: every WebSocket frame and
 * every GraphQL response body, as text.
 */
export function captureTraffic(page: Page): () => string[] {
  const seen: string[] = [];
  page.on("response", (response) => {
    if (!response.url().includes("graphql")) return;
    void response
      .text()
      .then((body) => seen.push(body))
      .catch(() => undefined);
  });
  page.on("websocket", (socket) => {
    socket.on("framereceived", (frame) => seen.push(String(frame.payload)));
  });
  return () => [...seen];
}
