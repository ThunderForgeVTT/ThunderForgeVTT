import type { Expect, Page } from "@playwright/test";

/**
 * Spec 014 US4's proof, as one function: on a play page whose canvas is up,
 * the dice panel rolls `1d20`, a result shows, and the Game Master finds the
 * roll in `worldRollRecords` afterwards.
 *
 * Shared rather than inlined in `dice-roll.spec.ts` because spec 074 SC-008
 * asks that an existing feature's proof run against the demo too, unchanged:
 * `apps/demo/e2e/dice.spec.ts` calls this same body on the demo's world. The
 * history is asked through the page's own `fetch`, so that against the demo
 * it reaches the in-page backend the way the app does; `expect` is passed in
 * so each suite asserts with its own copy of Playwright.
 */
export async function rollD20IntoHistory(
  page: Page,
  expect: Expect,
  worldId: string,
): Promise<void> {
  const panel = page.getByTestId("dice-roller-panel");
  await expect(panel).toBeVisible({ timeout: 15_000 });
  await page.getByTestId("dice-formula-input").fill("1d20");
  await page.getByTestId("dice-roll-button").click();

  await expect(page.getByTestId("dice-roll-result")).toBeVisible({
    timeout: 10_000,
  });
  const resultText = await page.getByTestId("dice-roll-result").innerText();
  expect(resultText).toMatch(/1d20:\s*-?\d+/);

  // Confirm the DM's roll history shows the matching record via GraphQL
  // (worldRollRecords, DM-only) — a lightweight check against the live
  // session's own cookies rather than adding a full history UI.
  const historyResponse = await page.evaluate(async (wid) => {
    const csrfToken = document.cookie
      .split(";")
      .map((part) => part.trim())
      .find((part) => part.startsWith("csrf_token="))
      ?.slice("csrf_token=".length);
    const res = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
      },
      body: JSON.stringify({
        query: `query($worldId: UUID!) { worldRollRecords(worldId: $worldId, limit: 5) { id resolution { formula resultValue } } }`,
        variables: { worldId: wid },
      }),
    });
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      throw new Error(
        `Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`,
      );
    }
  }, worldId);

  expect(historyResponse.data.worldRollRecords.length).toBeGreaterThan(0);
  expect(historyResponse.data.worldRollRecords[0].resolution.formula).toBe(
    "1d20",
  );
}
