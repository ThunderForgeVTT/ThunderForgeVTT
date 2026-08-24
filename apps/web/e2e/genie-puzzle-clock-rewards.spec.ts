import { test, expect, type Page } from "@playwright/test";
import { freshCredentials, graphql, register, registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 020 User Story 3: Puzzle Clock segments can each carry a
 * configured reward. Covers the per-segment "production run" case (the
 * blacksmithing example — one dagger per advance, attributed to the
 * triggering actor via the real UI), the single end-of-quest payout case
 * (whole-party split, verified LIVE on a real second account's own
 * client), and FR-006a's fallback (an unattributed advance against a
 * triggering_actor reward still grants, split across the party, rather
 * than failing or crediting no one — verified directly against the
 * server, since it's a plain-click/no-actor-selected case that doesn't
 * need its own dedicated UI path to prove).
 *
 * `ActorInventoryPanel` has no live-sync subscription today (a real,
 * separately-scoped gap, same class already documented in
 * genie-npc-and-items.spec.ts), so the item-reward assertion below is
 * server-side truth, not a live DOM check — only the resource-reward
 * case (which SessionResourceTrade IS live-synced for) is asserted live.
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

/**
 * Creating a clock updates `session.puzzleClocks`, which re-renders
 * `SessionClocks` — under load this can momentarily detach/replace the
 * "Create" button between fill and click (found live: the second clock
 * creation in this file flaked on exactly this). Retrying the whole
 * fill+click sequence, not just the click, sidesteps a stale-locator
 * `.fill()` racing the same re-render.
 */
async function createPuzzleClockViaUi(page: Page, label: string, segmentsMax: number): Promise<void> {
  await expect(async () => {
    await page.locator("#new-clock-label").fill(label);
    await page.locator("#new-clock-segments").fill(String(segmentsMax));
    await page.getByRole("button", { name: "Create" }).click({ timeout: 5_000 });
  }).toPass({ timeout: 20_000 });
  await expect(page.locator("li", { hasText: label })).toBeVisible({ timeout: 10_000 });
}

test.describe("Spec 020 User Story 3: Puzzle Clock segment rewards", () => {
  test("per-segment item rewards attribute to the triggering actor, a whole-party resource reward lands live on a real second client, and an unattributed advance falls back to the whole party", async ({
    browser,
  }) => {
    test.setTimeout(120_000);

    const gmContext = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Genie Clock Rewards ${Date.now()}`, "e2eclockgm");

    const smith = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Smith", isNpc: false, gameSystemId: "genie" } },
    );
    const smithId = smith.data.createActor.id;

    const playerActor = await graphql<{ data: { createActor: { id: string } } }>(
      gmPage,
      `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: "Party Member", isNpc: false, gameSystemId: "genie" } },
    );
    const playerActorId = playerActor.data.createActor.id;
    await graphql(
      gmPage,
      `mutation($actorId: UUID!, $available: Boolean!) { setActorAvailability(actorId: $actorId, available: $available) { id } }`,
      { actorId: playerActorId, available: true },
    );

    const dagger = await graphql<{ data: { createItem: { id: string } } }>(
      gmPage,
      `mutation($input: CreateItemInput!) { createItem(input: $input) { id } }`,
      { input: { worldId, name: "Dagger" } },
    );
    const daggerId = dagger.data.createItem.id;

    const inviteCode = await extractInviteCode(gmPage);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eclockplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });
    await playerPage.getByTestId("available-actor-row").getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    await gmPage.goto(`/world/${worldId}/staging`);
    await expect(gmPage.getByTestId("genie-session-panel-wrapper")).toBeVisible({ timeout: 15_000 });
    const startButton = gmPage.getByTestId("start-genie-session-button");
    if (await startButton.isVisible().catch(() => false)) {
      await startButton.click();
    }
    await expect(gmPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });

    // Create every clock this test will use UP FRONT: spec 018's win rule
    // (all_puzzle_clocks_resolved) fires as soon as every *existing*
    // Puzzle Clock is resolved — resolving "Forge Daggers" alone would
    // otherwise end the session before the other three clocks even exist
    // (taking genieSession(worldId) to null for the still-watching player
    // page, and rejecting any later advancePuzzleClock with "This Genie
    // session has already concluded").
    await createPuzzleClockViaUi(gmPage, "Forge Daggers", 3);
    await createPuzzleClockViaUi(gmPage, "Recover the Sealed Lamp", 2);

    const sessionForSetup = await graphql<{ data: { genieSession: { id: string } } }>(
      gmPage,
      `query($worldId: UUID!) { genieSession(worldId: $worldId) { id } }`,
      { worldId },
    );
    const sessionId = sessionForSetup.data.genieSession.id;

    const fallbackClock = await graphql<{ data: { createPuzzleClock: { id: string } } }>(
      gmPage,
      `mutation($sessionId: UUID!, $label: String!, $segmentsMax: Int!) {
        createPuzzleClock(sessionId: $sessionId, label: $label, segmentsMax: $segmentsMax) { id }
      }`,
      { sessionId, label: "Untended Forge", segmentsMax: 2 },
    );
    const fallbackClockId = fallbackClock.data.createPuzzleClock.id;
    await graphql(
      gmPage,
      `mutation($clockId: UUID!, $triggerSegment: Int!, $rewardResourceType: String, $rewardResourceAmount: Int, $recipientMode: GenieRewardRecipientMode!) {
        configurePuzzleClockReward(clockId: $clockId, triggerSegment: $triggerSegment, rewardResourceType: $rewardResourceType, rewardResourceAmount: $rewardResourceAmount, recipientMode: $recipientMode) { id }
      }`,
      {
        clockId: fallbackClockId,
        triggerSegment: 1,
        rewardResourceType: "essence",
        rewardResourceAmount: 2,
        recipientMode: "TRIGGERING_ACTOR",
      },
    );

    const plainClock = await graphql<{ data: { createPuzzleClock: { id: string } } }>(
      gmPage,
      `mutation($sessionId: UUID!, $label: String!, $segmentsMax: Int!) {
        createPuzzleClock(sessionId: $sessionId, label: $label, segmentsMax: $segmentsMax) { id }
      }`,
      { sessionId, label: "Plain Clock", segmentsMax: 2 },
    );
    const plainClockId = plainClock.data.createPuzzleClock.id;

    // A 3-segment "Forge Daggers" clock with a per-segment item reward on
    // every segment, recipient TRIGGERING_ACTOR — the blacksmithing case.
    await gmPage.getByTestId("reward-clock-select").selectOption({ label: "Forge Daggers" });
    await gmPage.getByTestId("reward-recipient-mode-select").selectOption("TRIGGERING_ACTOR");
    await gmPage
      .locator('[data-testid="genie-puzzle-clock-rewards-panel"] label', { hasText: "Item" })
      .locator("input")
      .check();
    for (const segment of [1, 2, 3]) {
      await expect(async () => {
        await gmPage.getByTestId("reward-trigger-segment-input").fill(String(segment));
        await gmPage.getByTestId("reward-item-select").selectOption({ label: "Dagger" });
        await gmPage.getByTestId("reward-item-quantity-input").fill("1");
        await gmPage.getByTestId("reward-configure-button").click({ timeout: 5_000 });
      }).toPass({ timeout: 15_000 });
      await gmPage.waitForTimeout(300);
    }

    // Advance the Forge Daggers clock 3 times, one segment at a time,
    // each time attributing the smith via the "advance with actor
    // attribution" control (FR-006a).
    for (let i = 0; i < 3; i++) {
      await expect(async () => {
        await gmPage.getByTestId("advance-with-actor-clock-select").selectOption({ label: "Forge Daggers" });
        await gmPage.getByTestId("advance-with-actor-select").selectOption({ label: "Smith" });
        await gmPage.getByTestId("advance-with-actor-delta-input").fill("1");
        await gmPage.getByTestId("advance-with-actor-button").click({ timeout: 5_000 });
      }).toPass({ timeout: 15_000 });
      await gmPage.waitForTimeout(500);
    }

    // Server-side truth: the smith received exactly 3 daggers, one per
    // advance, not a lump sum at segment 3.
    const smithInventory = await graphql<{
      data: { actorInventory: { itemId: string | null; quantity: number }[] };
    }>(gmPage, `query($actorId: UUID!) { actorInventory(actorId: $actorId) { itemId quantity } }`, {
      actorId: smithId,
    });
    expect(smithInventory.data.actorInventory.find((e) => e.itemId === daggerId)?.quantity ?? 0).toBe(3);

    // Second clock: a single end-of-quest reward at the final segment,
    // WHOLE_PARTY, 2 Favor — verified LIVE on the real second account's
    // own client (no reload).
    await playerPage.goto(`/world/${worldId}/staging`);
    await expect(playerPage.getByTestId("genie-session-panel")).toBeVisible({ timeout: 15_000 });
    const playerFavorCell = playerPage.getByTestId("session-resource-trade").locator("li", { hasText: "Favor" });
    await expect(playerFavorCell).toContainText("0");

    await expect(async () => {
      await gmPage.getByTestId("reward-clock-select").selectOption({ label: "Recover the Sealed Lamp" });
      await gmPage.getByTestId("reward-trigger-segment-input").fill("2");
      await gmPage.getByTestId("reward-recipient-mode-select").selectOption("WHOLE_PARTY");
      await gmPage
        .locator('[data-testid="genie-puzzle-clock-rewards-panel"] label', { hasText: "Resource" })
        .locator("input")
        .check();
      await gmPage.getByTestId("reward-resource-type-select").selectOption({ label: "Favor" });
      await gmPage.getByTestId("reward-resource-amount-input").fill("2");
      await gmPage.getByTestId("reward-configure-button").click({ timeout: 5_000 });
    }).toPass({ timeout: 15_000 });
    await gmPage.waitForTimeout(300);

    // Advance to the final segment via SessionClocks' own plain (no
    // actor) "Advance" button — proving a WHOLE_PARTY reward needs no
    // actor attribution at all.
    const recoverClockRow = gmPage.locator("li", { hasText: "Recover the Sealed Lamp" });
    await expect(async () => {
      await recoverClockRow.getByRole("button", { name: "Advance" }).click({ timeout: 5_000 });
    }).toPass({ timeout: 15_000 });
    await gmPage.waitForTimeout(300);
    await expect(async () => {
      await recoverClockRow.getByRole("button", { name: "Advance" }).click({ timeout: 5_000 });
    }).toPass({ timeout: 15_000 });

    // Live update on the recipient's own client — the sum across both
    // party members equals the full configured 2 Favor (research.md R4's
    // even-split-with-remainder rule guarantees nothing is lost).
    await expect
      .poll(
        async () => {
          const text = await playerFavorCell.innerText();
          return Number.parseInt(text.replace(/\D/g, ""), 10) || 0;
        },
        { timeout: 10_000 },
      )
      .toBeGreaterThanOrEqual(1);

    const smithFavor = await graphql<{
      data: { genieResourceHoldings: { resourceType: string; quantity: number }[] };
    }>(
      gmPage,
      `query($sessionId: UUID!, $actorId: UUID!) { genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) { resourceType quantity } }`,
      { sessionId, actorId: smithId },
    );
    const playerFavor = await graphql<{
      data: { genieResourceHoldings: { resourceType: string; quantity: number }[] };
    }>(
      gmPage,
      `query($sessionId: UUID!, $actorId: UUID!) { genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) { resourceType quantity } }`,
      { sessionId, actorId: playerActorId },
    );
    const smithFavorQty = smithFavor.data.genieResourceHoldings.find((h) => h.resourceType === "favor")?.quantity ?? 0;
    const playerFavorQty =
      playerFavor.data.genieResourceHoldings.find((h) => h.resourceType === "favor")?.quantity ?? 0;
    expect(smithFavorQty + playerFavorQty).toBe(2);

    // FR-006a fallback: a triggering_actor reward hit via a plain
    // advancePuzzleClock call with no actorId still grants — split across
    // the party — rather than failing or crediting no one. Clock and
    // reward were pre-created above (see the win-condition note).
    const fallbackResult = await graphql<{
      data?: { advancePuzzleClock: { id: string } };
      errors?: { message: string }[];
    }>(
      gmPage,
      `mutation($clockId: UUID!, $delta: Int!) { advancePuzzleClock(clockId: $clockId, delta: $delta) { id } }`,
      { clockId: fallbackClockId, delta: 1 },
    );
    expect(fallbackResult.errors ?? []).toHaveLength(0);

    const smithEssence = await graphql<{
      data: { genieResourceHoldings: { resourceType: string; quantity: number }[] };
    }>(
      gmPage,
      `query($sessionId: UUID!, $actorId: UUID!) { genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) { resourceType quantity } }`,
      { sessionId, actorId: smithId },
    );
    const playerEssence = await graphql<{
      data: { genieResourceHoldings: { resourceType: string; quantity: number }[] };
    }>(
      gmPage,
      `query($sessionId: UUID!, $actorId: UUID!) { genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) { resourceType quantity } }`,
      { sessionId, actorId: playerActorId },
    );
    const smithGained = smithEssence.data.genieResourceHoldings.find((h) => h.resourceType === "essence")?.quantity ?? 0;
    const playerGained =
      playerEssence.data.genieResourceHoldings.find((h) => h.resourceType === "essence")?.quantity ?? 0;
    expect(smithGained + playerGained).toBe(2);

    // Zero-configured-reward clock behaves exactly as spec 018/019 today.
    // Pre-created above; resolving it last is fine even though it also
    // completes the session (every clock is now resolved) — nothing
    // further in this test depends on the session staying active.
    const plainResolve = await graphql<{ data: { advancePuzzleClock: { resolvedAt: string | null } } }>(
      gmPage,
      `mutation($clockId: UUID!, $delta: Int!) { advancePuzzleClock(clockId: $clockId, delta: $delta) { resolvedAt } }`,
      { clockId: plainClockId, delta: 2 },
    );
    expect(plainResolve.data.advancePuzzleClock.resolvedAt).toBeTruthy();

    await gmContext.close();
    await playerContext.close();
  });
});
