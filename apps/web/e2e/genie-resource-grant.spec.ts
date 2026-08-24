import { test, expect, type Page } from "@playwright/test";
import { freshCredentials, graphql, register, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 020 User Story 1: a GM grants a Session Resource or an item
 * directly to a player's character — the bootstrapping fix that unblocks
 * spec 019's peer-to-peer trading (previously "fully built and fully
 * unusable" per spec 020's Problem statement, since a holding could only
 * ever move via a trade or a Puzzle Clock spend, both of which require a
 * holding to already exist).
 *
 * Uses two genuinely distinct accounts (not the same login in two
 * contexts), mirroring `genie-resource-trade.spec.ts`'s established
 * real-invite-flow pattern.
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

test.describe("Spec 020 User Story 1: GM grants Session Resources and items", () => {
  test("a resource grant appears live on the recipient's own client without reload, and an item grant lands in their inventory", async ({
    browser,
  }) => {
    test.setTimeout(120_000);

    const gmContext = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Genie Grant ${Date.now()}`, "e2egrantgm");

    // A PC, available for the incoming player to claim.
    const playerActor = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Grant Recipient", isNpc: false, gameSystemId: "genie" } },
    );
    const playerActorId = playerActor.data.createActor.id;
    await graphql(
      gmPage,
      `mutation($actorId: UUID!, $available: Boolean!) { setActorAvailability(actorId: $actorId, available: $available) { id } }`,
      { actorId: playerActorId, available: true },
    );

    // A world item for the item-grant half of this test (FR-002).
    const item = await graphql<{ data: { createItem: { id: string } } }>(
      gmPage,
      `mutation($input: CreateItemInput!) { createItem(input: $input) { id } }`,
      { input: { worldId, name: "Bag of Holding" } },
    );
    const itemId = item.data.createItem.id;

    const inviteCode = await extractInviteCode(gmPage);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2egrantplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });
    await playerPage.getByTestId("available-actor-row").getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // GM starts the session from the staging page.
    await gmPage.goto(`/world/${worldId}/staging`);
    await expect(gmPage.getByTestId("genie-session-panel-wrapper")).toBeVisible({ timeout: 15_000 });
    const startButton = gmPage.getByTestId("start-genie-session-button");
    if (await startButton.isVisible().catch(() => false)) {
      await startButton.click();
    }
    await expect(gmPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });

    // Player is already watching the staging page (open BEFORE the grant)
    // so the assertion below genuinely exercises live cross-client sync
    // (FR-007's "resource_grant" world_events kind), not a fetch-on-mount.
    await playerPage.goto(`/world/${worldId}/staging`);
    await expect(playerPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });
    const playerEssenceCell = playerPage
      .getByTestId("session-resource-trade")
      .locator("li", { hasText: "Essence" });
    await expect(playerEssenceCell).toContainText("0");

    // Scenario 1: GM grants 3 Essence via the Grant Resource panel.
    await expect(gmPage.getByTestId("genie-grant-resource-panel")).toBeVisible();
    await gmPage.getByTestId("grant-resource-actor-select").selectOption({ label: "Grant Recipient" });
    // Resource type select defaults to "Insight" — switch to Essence.
    await gmPage
      .locator('[data-testid="genie-grant-resource-panel"] select[aria-label="Resource type to grant"]')
      .selectOption({ label: "Essence" });
    await gmPage
      .locator('[data-testid="genie-grant-resource-panel"] input[aria-label="Amount to grant"]')
      .fill("3");
    await gmPage.getByTestId("grant-resource-button").click();

    // Live update on the recipient's own client, no reload (Scenario 1).
    await expect(playerEssenceCell).toContainText("3", { timeout: 10_000 });

    // Scenario 3: granting fails cleanly when no active session exists
    // (verified directly against the server, independent of this UI,
    // which only exposes the Grant control once a session is active).
    const noSessionGrant = await graphql<{ errors?: { message: string }[] }>(
      gmPage,
      `mutation($sessionId: UUID!, $actorId: UUID!, $resourceType: String!, $amount: Int!) {
        grantSessionResource(sessionId: $sessionId, actorId: $actorId, resourceType: $resourceType, amount: $amount) { quantity }
      }`,
      { sessionId: "00000000-0000-0000-0000-000000000000", actorId: playerActorId, resourceType: "insight", amount: 1 },
    );
    expect(noSessionGrant.errors?.length ?? 0).toBeGreaterThan(0);

    // Scenario 4: a non-GM caller cannot grant, even to their own actor.
    const sessionQuery = await graphql<{ data: { genieSession: { id: string } | null } }>(
      playerPage,
      `query($worldId: UUID!) { genieSession(worldId: $worldId) { id } }`,
      { worldId },
    );
    const sessionId = sessionQuery.data.genieSession!.id;
    const playerAttemptsGrant = await graphql<{ errors?: { message: string }[] }>(
      playerPage,
      `mutation($sessionId: UUID!, $actorId: UUID!, $resourceType: String!, $amount: Int!) {
        grantSessionResource(sessionId: $sessionId, actorId: $actorId, resourceType: $resourceType, amount: $amount) { quantity }
      }`,
      { sessionId, actorId: playerActorId, resourceType: "insight", amount: 1 },
    );
    expect(playerAttemptsGrant.errors?.length ?? 0).toBeGreaterThan(0);

    // Scenario 2: GM grants an item via the existing ActorInventoryPanel
    // "Add" affordance (FR-002 — no new mutation, reused as-is).
    await gmPage.goto(`/world/${worldId}/actor/${playerActorId}/view`);
    await expect(gmPage.getByTestId("actor-inventory-panel")).toBeVisible({ timeout: 10_000 });
    await gmPage.getByTestId("inventory-add-item-select").selectOption({ label: "Bag of Holding" });
    await gmPage.getByTestId("inventory-add-quantity-input").fill("1");
    await gmPage.getByTestId("inventory-add-button").click();
    const entryRow = gmPage.locator('[data-testid^="inventory-entry-"]').first();
    await expect(entryRow).toBeVisible({ timeout: 10_000 });
    await expect(entryRow).toContainText("Bag of Holding");

    // Confirm server-side truth for the recipient (ActorInventoryPanel
    // has no live-sync subscription today — this is a real fetch, not a
    // reload of stale client state).
    const recipientInventory = await graphql<{
      data: { actorInventory: { itemId: string | null; itemName: string }[] };
    }>(
      playerPage,
      `query($actorId: UUID!) { actorInventory(actorId: $actorId) { itemId itemName } }`,
      { actorId: playerActorId },
    );
    expect(recipientInventory.data.actorInventory.some((e) => e.itemId === itemId)).toBe(true);

    await gmContext.close();
    await playerContext.close();
  });
});
