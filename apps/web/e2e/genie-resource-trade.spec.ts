import { test, expect, type Page } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 019: Session Resource trading, wired via the new
 * `genieTradeProposals(actorId)` query (previously the backend only had
 * point mutations — propose/accept — with no way for a player to
 * discover a trade proposed to them). Uses two genuinely distinct
 * accounts (not the same login in two contexts), mirroring
 * `invite-membership.spec.ts`'s established real-invite-flow pattern —
 * the only way a second party actually gets access to a world today.
 */

async function registerAndCreateWorldOnDashboard(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2etradegm"));
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`Could not extract world id from URL: ${page.url()}`);
  const worldId = match[1];
  await page.goto(`/world/${worldId}`);
  await expect(page).toHaveURL(new RegExp(`/world/${worldId}$`));
  return worldId;
}

async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

test.describe("Spec 019: Session Resource trading between two real players", () => {
  test("a GM proposes a trade and the recipient sees it as an incoming proposal", async ({ browser }) => {
    test.setTimeout(120_000);

    const gmContext = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorldOnDashboard(gmPage, `E2E Genie Trade ${Date.now()}`);

    // GM's own PC.
    const gmActor = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "GM Trader", isNpc: false, gameSystemId: "genie" } },
    );
    const gmActorId = gmActor.data.createActor.id;

    // A second PC, available for the incoming player to claim.
    const playerActor = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Player Trader", isNpc: false, gameSystemId: "genie" } },
    );
    const playerActorId = playerActor.data.createActor.id;
    await graphql(
      gmPage,
      `mutation($actorId: UUID!, $available: Boolean!) { setActorAvailability(actorId: $actorId, available: $available) { id } }`,
      { actorId: playerActorId, available: true },
    );

    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(gmPage);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2etradeplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });
    await playerPage.getByTestId("available-actor-row").getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // Start the session and propose from the GM staging page.
    await gmPage.goto(`/world/${worldId}/staging`);
    const gmPanel = gmPage.getByTestId("genie-session-panel-wrapper");
    await expect(gmPanel).toBeVisible({ timeout: 15_000 });
    const startButton = gmPage.getByTestId("start-genie-session-button");
    if (await startButton.isVisible().catch(() => false)) {
      await startButton.click();
    }
    await expect(gmPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });

    const tradeForm = gmPage.locator("text=Propose a Trade").locator("..");
    await expect(tradeForm).toBeVisible({ timeout: 10_000 });
    await tradeForm.locator('input[type="number"]').first().fill("2");
    await gmPage.getByRole("button", { name: "Propose Trade" }).click();
    await gmPage.waitForTimeout(1_000);

    // Confirm via the real query the mutation used (server-side truth).
    const proposals = await graphql<{
      data: { genieTradeProposals: { fromActorId: string; fromQuantity: number }[] };
    }>(
      gmPage,
      `query($actorId: UUID!) { genieTradeProposals(actorId: $actorId) { fromActorId fromQuantity } }`,
      { actorId: playerActorId },
    );
    expect(proposals.data.genieTradeProposals.some((p) => p.fromActorId === gmActorId && p.fromQuantity === 2)).toBe(
      true,
    );

    // And confirm the real UI, as the recipient, renders it.
    await playerPage.goto(`/world/${worldId}/staging`);
    await expect(playerPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });
    await expect(playerPage.getByText("Incoming Trade Proposals")).toBeVisible({ timeout: 10_000 });
    await expect(playerPage.getByText(/GM Trader offers 2/)).toBeVisible();

    await gmContext.close();
    await playerContext.close();
  });
});
