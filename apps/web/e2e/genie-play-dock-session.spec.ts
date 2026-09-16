import { test, expect } from "./fixtures/test";
import { graphql, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Genie's session loop on the play field, where the owner found both of
 * these (2026-09-15) — not on the staging page the older specs drive.
 *
 * 1. **The actor picker offered nothing.** The Clocks & Timers panel's
 *    "grant to" and "attribute to" dropdowns were the *party*: player
 *    characters only. A world whose cast was NPCs got an empty list. They
 *    list actors now, characters first and NPCs after, each group labelled.
 *
 *    The NPC here is left hidden on purpose. The NPC visibility rule landed
 *    the same day and withholds a hidden NPC from a player everywhere; a
 *    Game Master must still see it. If that rule ever starts withholding
 *    from whoever runs the world, this spec is where it shows.
 *
 * 2. **A wish's "asked for" list was empty.** `spendWish` recorded the Wish
 *    Effect and nothing read it back. It is listed under the Wish Pool now.
 */

async function createActor(
  page: Parameters<typeof graphql>[0],
  worldId: string,
  label: string,
  isNpc: boolean,
): Promise<string> {
  const created = await graphql<{ data: { createActor: { id: string } } }>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, label, isNpc, gameSystemId: "genie" } },
  );
  return created.data.createActor.id;
}

test.describe("Genie session loop on the play field", () => {
  test("the actor picker lists characters then NPCs, hidden ones included for the GM, and a spent wish says what was asked for", async ({
    page,
  }) => {
    test.setTimeout(120_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Genie Dock ${Date.now()}`,
      "e2egeniedock",
    );

    // Created out of name order, so the assertion below is about sorting and
    // not about insertion order.
    const zahraId = await createActor(page, worldId, "Zahra the Broker", true);
    await createActor(page, worldId, "Brannic", false);
    await createActor(page, worldId, "Ash", true);
    await createActor(page, worldId, "Aurelia", false);

    await page.goto(`/world/${worldId}/play`);
    await page.getByTestId("world-dock-tab-clocks").click();
    const panel = page.getByTestId("world-dock-panel-clocks");
    await expect(panel.getByTestId("genie-session-panel")).toBeVisible({
      timeout: 15_000,
    });

    // ── 1. the picker ────────────────────────────────────────────────────
    const grantSelect = panel.getByTestId("grant-resource-actor-select");
    await expect(grantSelect.locator("optgroup")).toHaveCount(2, {
      timeout: 10_000,
    });
    await expect(grantSelect.locator("optgroup").nth(0)).toHaveAttribute(
      "label",
      "Characters",
    );
    await expect(grantSelect.locator("optgroup").nth(1)).toHaveAttribute(
      "label",
      "NPCs",
    );
    await expect(
      grantSelect.locator('optgroup[label="Characters"] option'),
    ).toHaveText(["Aurelia", "Brannic"]);
    // Both NPCs are hidden from players (the default). The Game Master
    // still sees them.
    await expect(
      grantSelect.locator('optgroup[label="NPCs"] option'),
    ).toHaveText(["Ash", "Zahra the Broker"]);

    // The attribution picker is the same list. It is only offered once there
    // is a Puzzle Clock to attribute an advance on, so make one.
    await panel.locator("#new-clock-label").fill("Open the vault");
    await panel
      .getByTestId("session-clocks")
      .getByRole("button", { name: "Create" })
      .click();
    await expect(panel.getByTestId("advance-with-actor-select")).toBeVisible({
      timeout: 10_000,
    });
    await expect(
      panel
        .getByTestId("advance-with-actor-select")
        .locator('optgroup[label="NPCs"] option'),
    ).toHaveText(["Ash", "Zahra the Broker"]);

    // And an NPC is a real target, not just a name in a list.
    await grantSelect.selectOption({ label: "Zahra the Broker" });
    await panel
      .locator('select[aria-label="Resource type to grant"]')
      .selectOption({ label: "Favor" });
    await panel.locator('input[aria-label="Amount to grant"]').fill("2");
    await panel.getByTestId("grant-resource-button").click();
    // Asked of the server rather than read off the panel: the grant panel
    // does not show a holding, and a silently refused grant looks identical
    // to a successful one from here.
    const sessionId = (
      await graphql<{ data: { genieSession: { id: string } } }>(
        page,
        `
          query ($worldId: UUID!) {
            genieSession(worldId: $worldId) {
              id
            }
          }
        `,
        { worldId },
      )
    ).data.genieSession.id;
    await expect
      .poll(
        async () =>
          (
            await graphql<{
              data: {
                genieResourceHoldings: {
                  resourceType: string;
                  quantity: number;
                }[];
              };
            }>(
              page,
              `
                query ($sessionId: UUID!, $actorId: UUID) {
                  genieResourceHoldings(
                    sessionId: $sessionId
                    actorId: $actorId
                  ) {
                    resourceType
                    quantity
                  }
                }
              `,
              { sessionId, actorId: zahraId },
            )
          ).data.genieResourceHoldings.find((h) => h.resourceType === "favor")
            ?.quantity ?? 0,
        { timeout: 10_000 },
      )
      .toBe(2);

    // ── 2. the asked-for list ────────────────────────────────────────────
    const wishPool = panel.getByTestId("session-wish-pool");
    const askedFor = wishPool.getByTestId("wish-asked-for");
    await expect(askedFor).toContainText("No wishes spent yet.");

    await wishPool
      .locator("#wish-narrative-effect")
      .fill("The sandstorm turns aside from the caravan.");
    await wishPool.getByRole("button", { name: "Spend a Wish" }).click();
    await expect(wishPool).toContainText("2 / 3 remaining", {
      timeout: 10_000,
    });
    await expect(askedFor.getByTestId("wish-asked-for-entry")).toHaveText(
      [/The sandstorm turns aside from the caravan\./],
      { timeout: 10_000 },
    );

    // It is read from the server, not held in the tab: a reload keeps it.
    await page.reload();
    // The dock may remember its open section across a reload; clicking an
    // open tab would close it.
    await expect(page.getByTestId("world-dock-tab-clocks")).toBeVisible({
      timeout: 15_000,
    });
    if (
      !(await page
        .getByTestId("world-dock-panel-clocks")
        .isVisible()
        .catch(() => false))
    ) {
      await page.getByTestId("world-dock-tab-clocks").click();
    }
    await expect(
      page
        .getByTestId("world-dock-panel-clocks")
        .getByTestId("wish-asked-for-entry"),
    ).toHaveText([/The sandstorm turns aside from the caravan\./], {
      timeout: 15_000,
    });
  });
});
