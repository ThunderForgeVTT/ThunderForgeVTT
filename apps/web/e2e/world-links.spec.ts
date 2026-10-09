import { test, expect, type Browser, type Page } from "./fixtures/test";
import {
  freshCredentials,
  register,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * specs/088-first-session-feedback (US1): a GM makes world links on the
 * players page, chooses a use limit or none, and every link that admits no
 * one says why.
 *
 * Owner's rules (2026-10-09): no limit by default, 1 to 50 when set, links
 * are revocable, and they admit existing ThunderForge accounts only. A use
 * counts only on a real join.
 */

async function openWorldLinks(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/players`);
  await expect(page.getByTestId("world-links-panel")).toBeVisible({
    timeout: 15_000,
  });
}

/** Generates a link with what is chosen, and returns the newest one's code. */
async function generateLink(page: Page): Promise<string> {
  const before = await page.getByTestId("invite-link-row").count();
  await page.getByRole("button", { name: "Generate Join Link" }).click();
  await expect(page.getByTestId("invite-link-row")).toHaveCount(before + 1, {
    timeout: 15_000,
  });
  const url = await page
    .getByTestId("invite-link-row")
    .first()
    .getByLabel("Invite link")
    .inputValue();
  const code = url.split("/join/")[1];
  expect(code, `could not parse a code out of ${url}`).toBeTruthy();
  return code;
}

async function newAccount(browser: Browser, prefix: string): Promise<Page> {
  const context = await browser.newContext();
  const page = await context.newPage();
  await register(page, freshCredentials(prefix));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  return page;
}

async function joinWith(page: Page, code: string, worldId: string) {
  await page.goto(`/join/${code}`);
  await page.getByRole("button", { name: "Join Campaign" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}`), { timeout: 15_000 });
}

test.describe("world links (spec 088)", () => {
  // Several fresh accounts and a world each: slow, not hung.
  test.describe.configure({ timeout: 150_000 });

  test("a default link has no limit, a join shows without a reload, and a player sees no links", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gm = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(
      gm,
      `Links Default ${uniqueSuffix()}`,
      "e2elinkgm",
    );
    await openWorldLinks(gm, worldId);
    await expect(gm.getByTestId("world-link-limit")).toHaveValue("none");
    const code = await generateLink(gm);

    const row = gm.getByTestId("invite-link-row").first();
    await expect(row.getByTestId("invite-link-uses")).toHaveText(
      "No joins yet",
    );
    await expect(row.getByTestId("invite-link-expiry")).toHaveText(/^Expires /);

    // Signed out, the link asks for an existing account and names no world.
    const strangerContext = await browser.newContext();
    const stranger = await strangerContext.newPage();
    await stranger.goto(`/join/${code}`);
    await stranger.waitForURL(/\/login\?returnTo=/, { timeout: 15_000 });
    await expect(stranger.getByTestId("world-link-sign-in-notice")).toBeVisible(
      { timeout: 15_000 },
    );
    await expect(
      stranger.getByRole("link", { name: /create a local account/i }),
    ).toHaveCount(0);
    await expect(stranger.getByText(/Links Default/)).toHaveCount(0);
    await strangerContext.close();

    // A player joins while the GM watches the list: no reload.
    const player = await newAccount(browser, "e2elinkpl");
    await joinWith(player, code, worldId);
    await expect(row.getByTestId("invite-link-uses")).toHaveText("1 join", {
      timeout: 15_000,
    });

    // The player sees the players page, but no links on it.
    await player.goto(`/world/${worldId}/players`);
    await expect(player.getByTestId("world-nav-overview")).toBeVisible({
      timeout: 15_000,
    });
    await expect(player.getByTestId("world-links-panel")).toHaveCount(0);
    await expect(
      player.getByRole("button", { name: "Generate Join Link" }),
    ).toHaveCount(0);

    await player.context().close();
    await gmContext.close();
  });

  test("a one-use link survives being opened, is spent by one join, and a revoked link says so", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gm = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(
      gm,
      `Links One Use ${uniqueSuffix()}`,
      "e2elinkgm",
    );
    await openWorldLinks(gm, worldId);
    await gm.getByTestId("world-link-limit").selectOption("1");
    const oneUse = await generateLink(gm);
    await expect(
      gm.getByTestId("invite-link-row").first().getByTestId("invite-link-uses"),
    ).toHaveText("No joins yet · 1 of 1 uses left");

    // Opened and refreshed, as a preview or a second look would: no use.
    const player = await newAccount(browser, "e2elinkpl");
    await player.goto(`/join/${oneUse}`);
    await expect(
      player.getByRole("button", { name: "Join Campaign" }),
    ).toBeVisible({ timeout: 15_000 });
    await player.reload();
    await expect(
      player.getByRole("button", { name: "Join Campaign" }),
    ).toBeVisible({ timeout: 15_000 });
    await openWorldLinks(gm, worldId);
    await expect(
      gm.getByTestId("invite-link-row").first().getByTestId("invite-link-uses"),
    ).toHaveText("No joins yet · 1 of 1 uses left");

    // One join spends it.
    await player.getByRole("button", { name: "Join Campaign" }).click();
    await player.waitForURL(new RegExp(`/world/${worldId}`), {
      timeout: 15_000,
    });

    // A second account reads that it has been used, with no Join.
    const latecomer = await newAccount(browser, "e2elinklate");
    await latecomer.goto(`/join/${oneUse}`);
    await expect(
      latecomer.getByRole("heading", { name: /this link has been used/i }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      latecomer.getByRole("button", { name: "Join Campaign" }),
    ).toHaveCount(0);

    // A revoked link reads as withdrawn.
    await gm.getByTestId("world-link-limit").selectOption("none");
    const revoked = await generateLink(gm);
    const row = gm.getByTestId("invite-link-row").first();
    await row.getByTestId("invite-link-revoke").click();
    await row.getByTestId("invite-link-revoke-confirm").click();
    await expect(gm.getByTestId("world-links-past")).toBeVisible({
      timeout: 15_000,
    });

    await latecomer.goto(`/join/${revoked}`);
    await expect(
      latecomer.getByRole("heading", { name: /this link was withdrawn/i }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      latecomer.getByRole("button", { name: "Join Campaign" }),
    ).toHaveCount(0);

    await latecomer.context().close();
    await player.context().close();
    await gmContext.close();
  });
});
