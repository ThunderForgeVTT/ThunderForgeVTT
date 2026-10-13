import { expect, test, type Page } from "./fixtures/test";
import { closeTrustTable, trustTable } from "./fixtures/world-cache-trust";

/**
 * Peer-adjudicated play's discrepancy display (spec 028 Phase 10a, T108a).
 *
 * The server computes a discrepancy in `reconcileQueuedChanges`, and
 * `RollResult.tsx` renders one. Nothing carries it between them:
 * `api/reconcile.ts` does not select the field, `ReconcileOutcome` has no
 * place for it, and `rollDice` has no such field at all. So the test here
 * puts the comparison on the wire itself and asserts on the display and the
 * role gate, which is the half that exists; the transport is noted rather than
 * pretended.
 */

test.describe("Client world cache — peer-adjudicated play (US7, Phase 10a)", () => {
  test.setTimeout(600_000);

  /**
   * FR-065 / FR-067, at the surface the requirement is actually about: what
   * the Game Master *sees*, and what everybody else does not.
   *
   * The two numbers are put on the wire by the test rather than by the
   * server, and that is a deliberate, documented limit. `rollDice` has no
   * `discrepancy` field and `api/reconcile.ts` does not select the one the
   * reconcile mutation returns, so nothing in the shipped client ever carries
   * a comparison to the component that renders it (see this suite's report).
   * Stubbing the response reaches the half that does exist — `RollResult`,
   * `discrepancyToShow`, and the role gate between them — running inside the
   * real page, with a real GM session and a real player session.
   *
   * The GM half is asserted first so the player half means something: the
   * marker demonstrably renders for exactly the same payload, so its absence
   * on the player's screen is the gate doing its work rather than the feature
   * failing to run.
   */
  test("a discrepancy is rendered distinctly for the Game Master, with both values, and not shown to players (SC-021a, FR-067, T108a)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108d", 1);
    const [player] = table.players;

    const CLAIMED = 41;
    const DETERMINED = 17;

    // Answer the app's own roll with the same result the server gave, plus
    // the comparison the server would attach once the field exists. Only the
    // roll is touched; every other GraphQL call passes through untouched.
    const attachDiscrepancy = async (page: Page) => {
      await page.route("**/api/graphql", async (route) => {
        const body = route.request().postData() ?? "";
        if (!body.includes("RollDice")) {
          await route.fallback();
          return;
        }
        const response = await route.fetch();
        const json = (await response.json()) as {
          data?: { rollDice?: Record<string, unknown> };
        };
        if (json.data?.rollDice) {
          json.data.rollDice.resultValue = CLAIMED;
          json.data.rollDice.discrepancy = {
            claimedValue: CLAIMED,
            determinedValue: DETERMINED,
          };
        }
        await route.fulfill({ response, json });
      });
    };

    await attachDiscrepancy(table.gm);
    await attachDiscrepancy(player);

    const roll = async (page: Page) => {
      await page.goto(`/world/${table.worldId}/play`);
      await expect(page.getByTestId("dice-roller-panel")).toBeVisible({
        timeout: 60_000,
      });
      await page.getByTestId("dice-formula-input").fill("1d20");
      await page.getByTestId("dice-roll-button").click();
      await expect(page.getByTestId("dice-roll-result")).toBeVisible({
        timeout: 30_000,
      });
    };

    await roll(table.gm);
    // Rendered distinctly, and the result itself stands unaltered — the note
    // is a difference observed, never a correction applied (FR-066).
    await expect(table.gm.getByTestId("roll-discrepancy-marker")).toBeVisible({
      timeout: 15_000,
    });
    await expect(table.gm.getByTestId("roll-result-total")).toHaveText(
      String(CLAIMED),
    );

    // Both values inspectable, and nothing offered to do about them: the
    // popover has two numbers and a sentence, and no control that would make
    // the software the arbiter of a question it cannot answer (FR-065a).
    await table.gm.getByTestId("roll-discrepancy-marker").click();
    await expect(table.gm.getByTestId("roll-discrepancy-claimed")).toHaveText(
      String(CLAIMED),
    );
    await expect(
      table.gm.getByTestId("roll-discrepancy-determined"),
    ).toHaveText(String(DETERMINED));
    const details = table.gm.getByTestId("roll-discrepancy-details");
    await expect(details).toContainText("stands as rolled");
    await expect(
      details.getByRole("button"),
      "there is no resolution workflow, deliberately",
    ).toHaveCount(0);

    // And the player, given the identical payload, sees an ordinary roll.
    await roll(player);
    await expect(player.getByTestId("roll-result-total")).toHaveText(
      String(CLAIMED),
    );
    await expect(
      player.getByTestId("roll-discrepancy-marker"),
      "a mark against a player must never be visible to the table (FR-067)",
    ).toHaveCount(0);

    await closeTrustTable(table);
  });
});
