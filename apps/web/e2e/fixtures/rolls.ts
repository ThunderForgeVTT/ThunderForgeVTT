import type { Browser } from "@playwright/test";

import { expect, type Page } from "./test";
import {
  clickPlay,
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
