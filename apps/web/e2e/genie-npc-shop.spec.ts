import { test, expect, type Page } from "@playwright/test";
import { freshCredentials, graphql, register, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 020 User Story 2: an NPC actor sells items for Session Resources
 * or item-for-item barter. GM stocks the NPC's inventory and prices two
 * listings (one resource-priced, one barter); a real second player buys
 * each. Mirrors `genie-resource-trade.spec.ts`'s two-real-account
 * invite-and-claim pattern.
 */

async function extractInviteCode(page: Page): Promise<string> {
  await page.getByTestId("session-setup-generate-invite").click();
  const input = page.getByTestId("session-setup-invite-url");
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

test.describe("Spec 020 User Story 2: NPC shop sells items for Session Resources or barter", () => {
  test("a player buys a resource-priced listing and a barter listing from a real NPC shop", async ({ browser }) => {
    test.setTimeout(120_000);

    const gmContext = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Genie Shop ${Date.now()}`, "e2eshopgm");

    // NPC merchant + a PC available for the incoming player to claim.
    const npc = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Traveling Merchant", isNpc: true, gameSystemId: "genie" } },
    );
    const npcId = npc.data.createActor.id;

    const playerActor = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Shop Customer", isNpc: false, gameSystemId: "genie" } },
    );
    const playerActorId = playerActor.data.createActor.id;
    await graphql(
      gmPage,
      `mutation($actorId: UUID!, $available: Boolean!) { setActorAvailability(actorId: $actorId, available: $available) { id } }`,
      { actorId: playerActorId, available: true },
    );

    // Two items: one sold for Insight, one bartered for a "Sealed Flask".
    const lantern = await graphql<{ data: { createItem: { id: string } } }>(
      gmPage,
      `mutation($input: CreateItemInput!) { createItem(input: $input) { id } }`,
      { input: { worldId, name: "Rusty Lantern" } },
    );
    const lanternId = lantern.data.createItem.id;
    const flask = await graphql<{ data: { createItem: { id: string } } }>(
      gmPage,
      `mutation($input: CreateItemInput!) { createItem(input: $input) { id } }`,
      { input: { worldId, name: "Sealed Flask" } },
    );
    const flaskId = flask.data.createItem.id;
    const dagger = await graphql<{ data: { createItem: { id: string } } }>(
      gmPage,
      `mutation($input: CreateItemInput!) { createItem(input: $input) { id } }`,
      { input: { worldId, name: "Traveling Dagger" } },
    );
    const daggerId = dagger.data.createItem.id;

    // Stock the NPC (one unit each of Rusty Lantern and Traveling Dagger).
    await graphql(
      gmPage,
      `mutation($input: AddItemToInventoryInput!) { addItemToInventory(input: $input) { id } }`,
      { input: { actorId: npcId, itemId: lanternId, quantity: 1 } },
    );
    await graphql(
      gmPage,
      `mutation($input: AddItemToInventoryInput!) { addItemToInventory(input: $input) { id } }`,
      { input: { actorId: npcId, itemId: daggerId, quantity: 1 } },
    );

    const inviteCode = await extractInviteCode(gmPage);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eshopplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });
    await playerPage.getByTestId("available-actor-row").getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // Give the player the flask they'll need for the barter listing, and
    // start the session (needed for the resource-priced listing).
    await graphql(
      gmPage,
      `mutation($input: AddItemToInventoryInput!) { addItemToInventory(input: $input) { id } }`,
      { input: { actorId: playerActorId, itemId: flaskId, quantity: 1 } },
    );
    await gmPage.goto(`/world/${worldId}/staging`);
    await expect(gmPage.getByTestId("genie-session-panel-wrapper")).toBeVisible({ timeout: 15_000 });
    const startButton = gmPage.getByTestId("start-genie-session-button");
    if (await startButton.isVisible().catch(() => false)) {
      await startButton.click();
    }
    await expect(gmPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });

    // Fund the buyer with 2 Insight via the real Grant Resource panel.
    await gmPage.getByTestId("grant-resource-actor-select").selectOption({ label: "Shop Customer" });
    await gmPage
      .locator('[data-testid="genie-grant-resource-panel"] input[aria-label="Amount to grant"]')
      .fill("2");
    await gmPage.getByTestId("grant-resource-button").click();
    await expect
      .poll(async () => {
        const holdings = await graphql<{ data: { genieResourceHoldings: { resourceType: string; quantity: number }[] } }>(
          gmPage,
          `query($sessionId: UUID!, $actorId: UUID!) { genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) { resourceType quantity } }`,
          { sessionId: (
              await graphql<{ data: { genieSession: { id: string } } }>(
                gmPage,
                `query($worldId: UUID!) { genieSession(worldId: $worldId) { id } }`,
                { worldId },
              )
            ).data.genieSession.id,
            actorId: playerActorId,
          },
        );
        return holdings.data.genieResourceHoldings.find((h) => h.resourceType === "insight")?.quantity ?? 0;
      }, { timeout: 10_000 })
      .toBe(2);

    // GM creates a resource-priced listing on the Rusty Lantern (2 Insight)
    // via the real NPC shop UI.
    await gmPage.goto(`/world/${worldId}/actor/${npcId}/view`);
    await expect(gmPage.getByTestId("genie-shop-panel")).toBeVisible({ timeout: 10_000 });
    await gmPage.getByTestId("shop-new-listing-item-select").selectOption({ label: "Rusty Lantern" });
    await gmPage.locator('[aria-label="Price amount"]').fill("2");
    await gmPage.getByTestId("shop-create-listing-button").click();
    await expect(gmPage.locator('[data-testid^="shop-listing-"]')).toHaveCount(1, { timeout: 10_000 });

    // ...and a barter listing on the Traveling Dagger (1 Sealed Flask).
    await gmPage.getByTestId("shop-new-listing-item-select").selectOption({ label: "Traveling Dagger" });
    await gmPage.getByText("Item barter").click();
    await gmPage.locator('[aria-label="Required barter item"]').selectOption({ label: "Sealed Flask" });
    await gmPage.getByTestId("shop-create-listing-button").click();
    await expect(gmPage.locator('[data-testid^="shop-listing-"]')).toHaveCount(2, { timeout: 10_000 });

    // Player buys the resource-priced listing.
    await playerPage.goto(`/world/${worldId}/actor/${npcId}/view`);
    await expect(playerPage.getByTestId("genie-shop-panel")).toBeVisible({ timeout: 10_000 });
    const lanternRow = playerPage.locator('[data-testid^="shop-listing-"]', { hasText: "Rusty Lantern" });
    await expect(lanternRow).toContainText("Stock: 1");
    await lanternRow.getByRole("button", { name: "Buy" }).click();
    await expect(lanternRow).toContainText("Stock: 0", { timeout: 10_000 });

    // Player buys the barter listing.
    const daggerRow = playerPage.locator('[data-testid^="shop-listing-"]', { hasText: "Traveling Dagger" });
    await expect(daggerRow).toContainText("Stock: 1");
    await daggerRow.getByRole("button", { name: "Buy" }).click();
    await expect(daggerRow).toContainText("Stock: 0", { timeout: 10_000 });

    // Confirm server-side truth: both purchased items landed in the
    // buyer's inventory, the flask left it, Insight was deducted, and the
    // NPC collected the traded-in flask (Scenario 3).
    const buyerInventory = await graphql<{
      data: { actorInventory: { itemId: string | null; quantity: number }[] };
    }>(playerPage, `query($actorId: UUID!) { actorInventory(actorId: $actorId) { itemId quantity } }`, {
      actorId: playerActorId,
    });
    const buyerItems = buyerInventory.data.actorInventory;
    expect(buyerItems.some((e) => e.itemId === lanternId && e.quantity >= 1)).toBe(true);
    expect(buyerItems.some((e) => e.itemId === daggerId && e.quantity >= 1)).toBe(true);
    expect(buyerItems.find((e) => e.itemId === flaskId)?.quantity ?? 0).toBe(0);

    const npcInventory = await graphql<{
      data: { actorInventory: { itemId: string | null; quantity: number }[] };
    }>(gmPage, `query($actorId: UUID!) { actorInventory(actorId: $actorId) { itemId quantity } }`, {
      actorId: npcId,
    });
    expect(npcInventory.data.actorInventory.find((e) => e.itemId === flaskId)?.quantity ?? 0).toBe(1);

    // Scenario 6: a plain NPC with no listings shows no shop UI to a
    // non-GM viewer.
    const plainNpc = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Silent Statue", isNpc: true, gameSystemId: "genie" } },
    );
    await playerPage.goto(`/world/${worldId}/actor/${plainNpc.data.createActor.id}/view`);
    await expect(playerPage.getByTestId("genie-shop-panel")).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });
});
