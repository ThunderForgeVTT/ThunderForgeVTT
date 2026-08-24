import { test, expect, type Page } from "@playwright/test";

/**
 * specs/016-system-pack-legal-compliance: the persistent world System
 * Settings surface (`/world/:id/settings/system`) where a GM assigns a
 * game system and everyone can see its legal/attribution notice. Backend
 * legal-manifest enforcement already has resolver-level coverage
 * (systems::manifest_legal_enforcement_tests); this is the missing
 * browser-level check that a GM can actually pick a system through the
 * real UI, see the legal notice before confirming, have it persist, and
 * that a non-GM sees the same info with no picker.
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

async function createWorld(page: Page, worldName: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`Could not extract world id from URL: ${page.url()}`);
  return match[1];
}

test.describe("Spec 016: GM assigns a game system and its legal notice is persistently visible", () => {
  test("GM sees Genie already assigned, unimplemented systems marked TBD and disabled, and re-confirming Genie's legal notice persists across a revisit", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esystem"));
    const worldId = await createWorld(page, `E2E System Settings ${uniqueSuffix()}`);

    // Spec 021: world creation now sends a game-system picker default
    // (Genie), so a freshly-created world already has one assigned rather
    // than showing "no system assigned yet".
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toContainText("Genie");

    await page.getByTestId("system-picker").click();

    // Spec 021: Genie is the only system with a real, data-connected actor
    // sheet — every other bundled pack is marked "(TBD)" and disabled in
    // the picker rather than silently accepting a non-functional choice.
    const dnd5eOption = page.getByRole("option", { name: "5E System Core" });
    await expect(dnd5eOption).toContainText("(TBD)");
    await expect(dnd5eOption).toHaveAttribute("aria-disabled", "true");

    // Re-picking the already-active system still exercises the "review the
    // legal notice before confirming" flow (FR-004's "point of selection"
    // review step), without needing a second implemented system.
    await page.getByRole("option", { name: "Genie", exact: true }).click();
    await expect(page.getByTestId("pending-system-confirmation")).toBeVisible({ timeout: 10_000 });
    const pendingLegalText = await page.getByTestId("pending-system-confirmation").innerText();
    expect(pendingLegalText.length).toBeGreaterThan(0);

    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByText("System assigned.")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("active-system-card")).toContainText("Genie");

    // Persists across a fresh navigation, not just optimistic local state.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toContainText("Genie", { timeout: 10_000 });
    await expect(page.getByTestId("active-system-card")).not.toContainText(/no system assigned yet/i);
  });

  test("a non-GM member sees the active system and its legal notice, but no picker", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    await register(gmPage, freshCredentials("e2esystemgm"));
    // Spec 021: world creation already assigns Genie by default, so there's
    // no separate "GM assigns a system" step needed before checking that a
    // non-GM member sees the same info with no picker.
    const worldId = await createWorld(gmPage, `E2E System Settings Viewer ${uniqueSuffix()}`);

    await gmPage.goto(`/world/${worldId}`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = gmPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2esystemplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL((url) => url.pathname.startsWith(`/world/${worldId}`), {
      timeout: 15_000,
    });

    await playerPage.goto(`/world/${worldId}/settings/system`);
    await expect(playerPage.getByTestId("active-system-card")).toContainText("Genie", {
      timeout: 10_000,
    });
    // No GM-only picker for a non-GM member.
    await expect(playerPage.getByTestId("system-picker-card")).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });
});
