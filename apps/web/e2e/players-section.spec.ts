import { createNpcViaCompendium } from "./fixtures/content";
import { expect, test } from "@playwright/test";
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

  // Overview no longer shows any player roster.
  await gmPage.goto(`/world/${worldId}/staging`);
  await expect(gmPage.getByTestId("staging-player-list")).toHaveCount(0);

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
  await expect(pageA.getByText(actorLabel)).toBeVisible();
  // Both the GM/Owner (synthesized into the roster — they have no real
  // world_members row of their own) and the non-claiming second member
  // show this label.
  // The card says "No character" where the table said "No character claimed".
  await expect(pageA.getByText("No character", { exact: true })).toHaveCount(2);

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
