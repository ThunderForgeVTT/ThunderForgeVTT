import { expectNoAxeViolations } from "./fixtures/axe";
import { createNpcViaCompendium } from "./fixtures/content";
import { expect, test } from "./fixtures/test";
import { freshCredentials, register, uniqueSuffix } from "./fixtures/helpers";

/**
 * Spec 023: the Players section — every member browses the roster as
 * characters (US1), and GM/Owner members get role-change/removal controls
 * there instead of on the world dashboard's Campaign Settings panel (US2,
 * FR-011).
 */

async function registerAndCreateWorld(
  page: import("@playwright/test").Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2eplygm"));
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

async function extractInviteCode(
  page: import("@playwright/test").Page,
): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

async function generateInviteCode(
  gmPage: import("@playwright/test").Page,
  worldId: string,
): Promise<string> {
  await gmPage.goto(`/world/${worldId}`);
  await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
  return extractInviteCode(gmPage);
}

async function createPcActor(
  gmPage: import("@playwright/test").Page,
  worldId: string,
  label: string,
): Promise<string> {
  // Through the shared fixture: this spec needs an actor, it is not about how
  // one is made. See `fixtures/content.ts`.
  const actorId = await createNpcViaCompendium(gmPage, worldId, label);

  await gmPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await gmPage.getByLabel(/this is a player character/i).check();
  await gmPage.getByRole("button", { name: "Save" }).click();
  await expect(gmPage.getByText(/^saved\.?$/i)).toBeVisible({
    timeout: 10_000,
  });

  return actorId;
}

async function markAvailable(
  gmPage: import("@playwright/test").Page,
  worldId: string,
  actorId: string,
): Promise<void> {
  await gmPage.goto(`/world/${worldId}/actor/${actorId}/view`);
  const checkbox = gmPage
    .getByTestId("actor-claim-block")
    .locator('input[type="checkbox"]');
  await checkbox.click();
  await expect(checkbox).toBeChecked({ timeout: 10_000 });
}

test("US1: every member sees the roster paired with claimed characters, and Overview no longer shows a roster", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  const worldId = await registerAndCreateWorld(
    gmPage,
    `E2E Players Roster ${uniqueSuffix()}`,
  );

  const actorLabel = `Claimable ${uniqueSuffix()}`;
  const actorId = await createPcActor(gmPage, worldId, actorLabel);
  await markAvailable(gmPage, worldId, actorId);

  // The roster lives on its own section now, not on the overview. Asserted by
  // the nav entry that leads there: the old overview list's testid exists
  // nowhere, so asserting its absence passed against a blank page
  // (docs/test-audit-2026-09-02.md).
  await gmPage.goto(`/world/${worldId}/staging`);
  await expect(gmPage.getByTestId("world-nav-players")).toBeVisible();

  // First member joins and claims the character.
  const inviteA = await generateInviteCode(gmPage, worldId);
  const contextA = await browser.newContext();
  const pageA = await contextA.newPage();
  await register(pageA, freshCredentials("e2eplyclaim"));
  await pageA.goto(`/join/${inviteA}`);
  await pageA.getByRole("button", { name: "Join Campaign" }).click();
  await pageA.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
    timeout: 15_000,
  });
  await pageA
    .getByTestId("available-actor-row")
    .first()
    .getByRole("button", { name: "Select" })
    .click();
  await pageA.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

  // Second member joins without claiming (no characters left available).
  const inviteB = await generateInviteCode(gmPage, worldId);
  const contextB = await browser.newContext();
  const pageB = await contextB.newPage();
  await register(pageB, freshCredentials("e2eplynoclaim"));
  await pageB.goto(`/join/${inviteB}`);
  await pageB.getByRole("button", { name: "Join Campaign" }).click();

  // The Players section, opened from the sidebar, shows all three members
  // correctly paired/marked — verified from a non-GM member's own view.
  await pageA.goto(`/world/${worldId}/players`);
  await expect(pageA.getByTestId("players-list")).toBeVisible({
    timeout: 10_000,
  });
  // Cards, not rows: the Players section became a searchable card grid in
  // spec 031 (FR-033) because a bare table showed neither who a player is
  // playing nor any way to set it. The card's testid is the stable handle.
  const rows = pageA
    .getByTestId("players-list")
    .locator('[data-testid^="player-card-"]');
  await expect(rows).toHaveCount(3);
  await expect(pageA.getByRole("link", { name: actorLabel })).toBeVisible();
  // Both the GM/Owner (synthesized into the roster — they have no real
  // world_members row of their own) and the non-claiming second member
  // show this label.
  // The card says "No character" where the table said "No character claimed".
  // Only the second player's card, now: the Game Master's card says they play
  // every character, and "None set" for a character of their own.
  await expect(pageA.getByText("No character", { exact: true })).toHaveCount(1);
  const gmCard = pageA.locator('[data-runs-the-table="true"]');
  await expect(gmCard).toHaveCount(1);
  await expect(gmCard).toContainText("Owner / Game Master");
  await expect(gmCard).toContainText("Playing all characters.");

  await gmContext.close();
  await contextA.close();
  await contextB.close();
});

test("US2: GM changes a role and removes a member from the Players section; non-GM sees no controls; Campaign Settings panel is trimmed", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  const worldId = await registerAndCreateWorld(
    gmPage,
    `E2E Players Moderation ${uniqueSuffix()}`,
  );

  const inviteA = await generateInviteCode(gmPage, worldId);
  const contextA = await browser.newContext();
  const pageA = await contextA.newPage();
  await register(pageA, freshCredentials("e2eplymodA"));
  await pageA.goto(`/join/${inviteA}`);
  await pageA.getByRole("button", { name: "Join Campaign" }).click();
  await pageA.waitForURL(
    (url) => url.pathname.startsWith(`/world/${worldId}`),
    { timeout: 15_000 },
  );

  const inviteB = await generateInviteCode(gmPage, worldId);
  const contextB = await browser.newContext();
  const pageB = await contextB.newPage();
  await register(pageB, freshCredentials("e2eplymodB"));
  await pageB.goto(`/join/${inviteB}`);
  await pageB.getByRole("button", { name: "Join Campaign" }).click();
  await pageB.waitForURL(
    (url) => url.pathname.startsWith(`/world/${worldId}`),
    { timeout: 15_000 },
  );

  // GM promotes member A to GM, and removes member B.
  await gmPage.goto(`/world/${worldId}/players`);
  await expect(gmPage.getByTestId("players-list")).toBeVisible({
    timeout: 10_000,
  });
  const rows = gmPage
    .getByTestId("players-list")
    .locator('[data-testid^="player-card-"]');
  await expect(rows).toHaveCount(3);

  // Capture stable per-row test ids up front — after the role change
  // below, the "Player"-role text filter would otherwise re-match a
  // *different* row (the one not yet promoted), not the row just
  // changed, since Locators are re-evaluated lazily at assertion time.
  const playerRoleSelects = gmPage
    .getByTestId("players-list")
    .locator('select[data-testid^="player-role-select-"]');
  await expect(playerRoleSelects).toHaveCount(2);
  const rowATestId = await playerRoleSelects.nth(0).getAttribute("data-testid");
  const rowBTestId = await playerRoleSelects.nth(1).getAttribute("data-testid");
  if (!rowATestId || !rowBTestId)
    throw new Error("Could not capture player row test ids");

  const roleSelect = gmPage.getByTestId(rowATestId);
  await roleSelect.selectOption("GM");
  await expect(roleSelect).toHaveValue("GM", { timeout: 10_000 });

  const removeButtonTestId = rowBTestId.replace(
    "player-role-select-",
    "player-remove-",
  );
  gmPage.once("dialog", (dialog) => void dialog.accept());
  await gmPage.getByTestId(removeButtonTestId).click();
  await expect(rows).toHaveCount(2, { timeout: 10_000 });

  // A non-GM member sees no role-change or removal controls.
  await pageA.reload();
  await pageA.goto(`/world/${worldId}/players`);
  await expect(pageA.getByTestId("players-list")).toBeVisible({
    timeout: 10_000,
  });
  await expect(
    pageA.locator('select[data-testid^="player-role-select-"]'),
  ).toHaveCount(0);
  await expect(pageA.getByRole("button", { name: "Remove" })).toHaveCount(0);

  // The Campaign Settings panel no longer shows a roster or role/remove controls.
  await gmPage.goto(`/world/${worldId}`);
  await expect(gmPage.getByText(/player roster/i)).toHaveCount(0);
  await expect(gmPage.getByRole("button", { name: "Remove" })).toHaveCount(0);

  await gmContext.close();
  await contextA.close();
  await contextB.close();
});

/**
 * What a Game Master's card says, and a Game Master holding a character of
 * their own.
 *
 * Two Game Masters, because the server treats them differently. A member
 * promoted to Game Master has a membership record, and can set and unset a
 * character of their own. A world's creator has none: the server refuses the
 * binding ("That player is not a member of this world"), so their card says
 * that rather than offering a picker that can only fail. When the server keeps
 * a record for the creator, the second half of this test is what changes.
 */
test("a Game Master sets and unsets a character of their own, and still plays every character", async ({
  browser,
}) => {
  test.setTimeout(150_000);
  const ownerContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const ownerPage = await ownerContext.newPage();
  const worldId = await registerAndCreateWorld(
    ownerPage,
    `E2E Players GM Own ${uniqueSuffix()}`,
  );
  const actorLabel = `GM Own ${uniqueSuffix()}`;
  await createPcActor(ownerPage, worldId, actorLabel);

  // A second person joins, and the creator makes them a Game Master.
  const invite = await generateInviteCode(ownerPage, worldId);
  const gmContext = await browser.newContext();
  const gmPage = await gmContext.newPage();
  await register(gmPage, freshCredentials("e2eplyowngm"));
  await gmPage.goto(`/join/${invite}`);
  await gmPage.getByRole("button", { name: "Join Campaign" }).click();
  await gmPage.waitForURL(
    (url) => url.pathname.startsWith(`/world/${worldId}`),
    { timeout: 15_000 },
  );
  await ownerPage.goto(`/world/${worldId}/players`);
  const roleSelect = ownerPage
    .getByTestId("players-list")
    .locator('select[data-testid^="player-role-select-"]');
  await expect(roleSelect).toHaveCount(1, { timeout: 10_000 });
  await roleSelect.selectOption("GM");
  await expect(roleSelect).toHaveValue("GM", { timeout: 10_000 });

  // The creator's own card: what they are, that they play everything, and
  // plainly that a character of their own cannot be set.
  const creatorCard = ownerPage.locator(
    '[data-runs-the-table="true"]:has-text("(you)")',
  );
  await expect(creatorCard).toContainText("Owner / Game Master");
  await expect(creatorCard).toContainText("Playing all characters.");
  await expect(
    creatorCard.locator('[data-testid^="player-character-unavailable-"]'),
  ).toContainText("can't be set for the world's creator yet");
  await expect(
    creatorCard.locator('select[data-testid^="player-character-select-"]'),
  ).toHaveCount(0);

  // The promoted Game Master sets a character of their own, on their own card.
  await gmPage.goto(`/world/${worldId}/players`);
  const ownCard = gmPage.locator(
    '[data-runs-the-table="true"]:has-text("(you)")',
  );
  await expect(ownCard).toContainText("Game Master", { timeout: 10_000 });
  await expect(ownCard).toContainText("Playing all characters.");
  await expect(ownCard).toContainText("None set");
  await expectNoAxeViolations(gmPage, '[data-testid="players-list"]');

  await ownCard
    .locator('select[data-testid^="player-character-select-"]')
    .selectOption({ label: actorLabel });
  await expect(ownCard).toContainText(
    `Playing all characters, with ${actorLabel} as their own.`,
    { timeout: 10_000 },
  );
  await expect(gmPage.getByTestId("players-error")).toHaveCount(0);
  await expect(ownCard.getByRole("link", { name: actorLabel })).toBeVisible();

  // The server's record, not the card's state: the creator sees it too.
  await ownerPage.reload();
  const promotedCard = ownerPage.locator(
    '[data-runs-the-table="true"]:not(:has-text("(you)"))',
  );
  await expect(promotedCard).toContainText(`with ${actorLabel} as their own.`, {
    timeout: 10_000,
  });

  // Unset. They give up their own character and nothing else.
  await ownCard.getByRole("button", { name: "Unset your character" }).click();
  await expect(ownCard).toContainText("None set", { timeout: 10_000 });
  await expect(ownCard).toContainText("Playing all characters.");
  await expect(gmPage.getByTestId("players-error")).toHaveCount(0);
  await gmPage.reload();
  await expect(ownCard).toContainText("None set", { timeout: 10_000 });

  // 375px: the cards fit, and pass axe.
  await gmPage.setViewportSize({ width: 375, height: 812 });
  await gmPage.reload();
  const list = gmPage.getByTestId("players-list");
  await expect(list).toBeVisible({ timeout: 10_000 });
  const listBox = await list.boundingBox();
  expect(listBox, "the roster is on screen").not.toBeNull();
  expect(listBox!.x + listBox!.width).toBeLessThanOrEqual(375);
  await expectNoAxeViolations(gmPage, '[data-testid="players-list"]');

  await ownerContext.close();
  await gmContext.close();
});
