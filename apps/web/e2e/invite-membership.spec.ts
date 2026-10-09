import { test, expect, type Page } from "./fixtures/test";

/**
 * specs/005-live-canvas-sync, User Story 4 (folded in post-planning):
 * verifies the two real, pre-existing bugs found during spec 003's live
 * verification are actually fixed, end-to-end, through the real app —
 * not just at the unit-test level (mutations_invites.rs::tests already
 * covers the underlying `require_world_member` fix directly).
 *
 * Before this fix: `CampaignSettingsPanel.tsx`'s "Generate Join Link"
 * button sent flat GraphQL arguments the resolver didn't accept, and
 * even corrected, the resolver's own inline `world_members` lookup had
 * no fallback for a world's own owner (who has no `world_members` row —
 * `create_world` never inserts one, see `insert_test_world`'s doc
 * comment in `test_support.rs`). Together these meant no session could
 * ever invite a second, distinct account into a world through the real
 * app — confirmed live and `test.skip`-ed in
 * `map-editor-tooling.spec.ts` (spec 003, T006) for exactly this reason.
 *
 * This is, deliberately, the first e2e test in this project to use two
 * genuinely distinct accounts (not the same login reused in a second
 * browser context) — closing the blind spot both the unit-test and e2e
 * gap analyses flagged independently.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

interface Credentials {
  username: string;
  email: string;
  password: string;
}

function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
}

async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

/** Registers a GM, creates a world, then explicitly navigates to the world
 * dashboard (`/world/{id}`), where `CampaignSettingsPanel` lives — needed
 * to generate an invite code. Spec 008: CreateWorldPage's own post-success
 * navigation now goes straight to `/world/{id}/play`, so reaching the
 * dashboard is a deliberate second step here, not the default landing. */
async function registerAndCreateWorldOnDashboard(
  page: Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2egm"));

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });

  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  const worldId = match[1];

  await page.goto(`/world/${worldId}`);
  await expect(page).toHaveURL(new RegExp(`/world/${worldId}$`));
  return worldId;
}

test.describe("A GM invites a genuine second player (US4)", () => {
  test("GM generates a working invite code, and a distinct second account joins the world", async ({
    browser,
  }) => {
    // Clipboard permission avoids handleGenerateInvite's
    // navigator.clipboard.writeText call throwing in a headless context
    // with no clipboard permission granted (canvas-asset-paste.spec.ts's
    // own doc comment notes this exact class of flakiness elsewhere in
    // this codebase and works around it differently; granting the
    // permission is the simpler fix here since this test isn't itself
    // testing clipboard behavior).
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();

    const worldId = await registerAndCreateWorldOnDashboard(
      gmPage,
      `E2E Invite Membership ${uniqueSuffix()}`,
    );

    // Bug 1 regression guard: this click used to fail outright with a
    // GraphQL argument-shape error ("argument input... is required but
    // not provided").
    // Spec 088: links are made on the players page.
    await gmPage.goto(`/world/${worldId}/players`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();

    // Bug 2 regression guard: this used to fail with "User is not a
    // member of this world" even with Bug 1 fixed, since create_world
    // never gives the owner a world_members row.
    const inviteCode = await extractInviteCode(gmPage);
    // No error banner should be showing after a successful generation.
    await expect(
      gmPage.getByText(/failed to generate invite code/i),
    ).toHaveCount(0);

    // A second, genuinely distinct account — not the GM's login reused.
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eplayer"));

    await playerPage.goto(`/join/${inviteCode}`);
    await expect(
      playerPage.getByRole("button", { name: "Join Campaign" }),
    ).toBeVisible({ timeout: 10_000 });
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();

    // Spec 017 (FR-001): a non-GM member who joins with zero
    // GM-designated available characters and player-created-actors off is
    // routed to Actor Selection's "ask your GM" wait state, not straight
    // to the world dashboard — the join itself still succeeded (they're a
    // full member), just gated on picking a character first.
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });
    await expect(playerPage.getByTestId("waiting-for-gm")).toBeVisible();

    await gmContext.close();
    await playerContext.close();
  });

  test("visiting a join link with an invalid invite code shows a clear error, not a crash", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2ebadcode"));
    await page.goto("/join/NOTAREALCODE");

    // Spec 088 (FR-008) replaces spec 027's single message: a signed-in
    // visitor is told why a link admits no one, and is offered no Join.
    await expect(
      page.getByRole("heading", { name: /no world behind this link/i }),
    ).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/check it was copied whole/i)).toBeVisible();
    await expect(
      page.getByRole("button", { name: /return to my campaigns/i }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Join Campaign" }),
    ).toHaveCount(0);
  });

  test("a user who is already a member sees the already-a-member state, not the join flow", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorldOnDashboard(
      gmPage,
      `E2E Already Member ${uniqueSuffix()}`,
    );
    // Spec 088: links are made on the players page.
    await gmPage.goto(`/world/${worldId}/players`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(gmPage);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2ealready"));

    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });

    // Revisit the same invite link as the same, now-a-member, user.
    await playerPage.goto(`/join/${inviteCode}`);
    await expect(
      playerPage.getByText(/you.re already in this world/i),
    ).toBeVisible({
      timeout: 10_000,
    });
    await expect(
      playerPage.getByRole("button", { name: /enter campaign/i }),
    ).toBeVisible();
    // The join flow itself must not be offered again.
    await expect(
      playerPage.getByRole("button", { name: "Join Campaign" }),
    ).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });

  // Owner's rule, 2026-10-09: a use counts only when someone becomes a new
  // member. The GM owns the world without a world_members row, so their own
  // link used to offer Join, and the click burned a use and added a Player
  // row beside the ownership.
  test("the GM opening their own link sees they are already a member, and no use is burned", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorldOnDashboard(
      gmPage,
      `E2E Owner Link ${uniqueSuffix()}`,
    );
    // Spec 088: links are made on the players page.
    await gmPage.goto(`/world/${worldId}/players`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(gmPage);
    // Spec 088: a link made with the defaults has no use limit.
    await expect(gmPage.getByTestId("invite-link-uses").first()).toHaveText(
      "No joins yet",
    );

    await gmPage.goto(`/join/${inviteCode}`);
    await expect(gmPage.getByText(/you.re already in this world/i)).toBeVisible(
      {
        timeout: 10_000,
      },
    );
    await expect(
      gmPage.getByRole("button", { name: /enter campaign/i }),
    ).toBeVisible();
    await expect(
      gmPage.getByRole("button", { name: "Join Campaign" }),
    ).toHaveCount(0);

    // Opening the link twice more, as a refresh or a bot would, still burns
    // nothing.
    await gmPage.reload();
    await expect(gmPage.getByText(/you.re already in this world/i)).toBeVisible(
      {
        timeout: 10_000,
      },
    );

    await gmPage.goto(`/world/${worldId}/players`);
    await expect(gmPage.getByTestId("invite-link-uses").first()).toHaveText(
      "No joins yet",
      { timeout: 10_000 },
    );

    await gmContext.close();
  });

  // A used-up link is covered by world-links.spec.ts (spec 088), which
  // makes a one-use link through the players page.
});
