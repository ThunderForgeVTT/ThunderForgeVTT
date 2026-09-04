import { test, expect, type Page } from "@playwright/test";

/**
 * `032/T108`: a pack declares the panels it contributes.
 *
 * Four pages used to compare a world's game system id against one system's
 * and mount that system's component if it matched. They now look a slot up in
 * a registry `import.meta.glob` builds from
 * `packs/systems/<id>/web/src/panels/<slot>.tsx`, and know nothing about who
 * filled it.
 *
 * Two of the four slots already have e2e that fail loudly if the mount
 * breaks: `genie-session-loop` and four other specs open the staging panel by
 * its test id, and `genie-npc-shop` buys from the NPC shop. The settings slot
 * had none — the Session Resource carryover card had never been exercised in
 * a browser, before or after this move — so it is covered here rather than by
 * restating what the Genie specs already prove.
 *
 * The third test is the absence, which is what the four retired violations
 * could never have been asked in reverse. Under the old code these surfaces
 * were decided by a comparison against one id, so "a different system" and "a
 * system that contributes nothing" were the same branch. They are now
 * different questions, and both still have to answer correctly.
 *
 * **Not covered here: the clocks dock's empty state.** `ClocksPanel` inverted
 * from "is this world *not* that system?" to "did any pack fill the clocks
 * slot?", and the honest browser test of that would mount the play dock,
 * which loads the engine — a large cost for one paragraph of text. It is
 * covered instead by `systemPanels.test.ts`, which asserts the lookup the
 * component now makes, and left named here rather than quietly skipped.
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

test.describe("Spec 032 T108: a pack's panels reach the pages that host them", () => {
  /**
   * The `world-settings` slot, end to end through a real mutation.
   *
   * Toggling and re-reading after a fresh navigation is the whole point: the
   * panel no longer returns a `WorldRecord` for the page to hold, it tells
   * the page to re-read. A card that flipped its own checkbox and never
   * persisted would pass a weaker check.
   */
  test("a GM sees the system's settings panel, and its toggle persists", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eslotgm"));
    const worldId = await createWorld(page, `E2E Panel Slots ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);

    const card = page.getByTestId("genie-resource-carryover-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    const toggle = page.getByTestId("genie-resource-carryover-toggle");
    const before = await toggle.isChecked();
    await toggle.click();
    await expect(toggle).toBeChecked({ checked: !before });

    // Re-read, not re-render. The panel's mutation selects `id` alone and
    // signals `onWorldChanged`; if that signal were dropped the checkbox
    // would still look right here and be wrong after a navigation.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("genie-resource-carryover-toggle")).toBeChecked(
      { checked: !before, timeout: 15_000 },
    );
  });

  /**
   * The same slot, for someone who is not a GM.
   *
   * The page mounts the panel for every member and passes `isGm`; the panel
   * decides. That is deliberate — a pack may well have something to show a
   * player — so the assertion is that *this* panel draws nothing for one,
   * rather than that the page refused to mount it.
   */
  test("a non-GM member is shown none of it", async ({ browser }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    await register(gmPage, freshCredentials("e2eslotowner"));
    const worldId = await createWorld(gmPage, `E2E Panel Slots Viewer ${uniqueSuffix()}`);

    await gmPage.goto(`/world/${worldId}`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = gmPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eslotplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(
      (url) => url.pathname.startsWith(`/world/${worldId}`),
      { timeout: 15_000 },
    );

    await playerPage.goto(`/world/${worldId}/settings/system`);
    await expect(playerPage.getByTestId("active-system-card")).toContainText(
      "Genie",
      { timeout: 10_000 },
    );
    await expect(
      playerPage.getByTestId("genie-resource-carryover-card"),
    ).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });

  /**
   * A world whose system contributes no panels: the staging page ends after
   * the invite link, and the settings page shows no contributed card.
   *
   * This is the assertion the four `KNOWN` violations could never have
   * passed in reverse — under the old code these surfaces were decided by a
   * comparison against one id, so "some other system" and "a system with no
   * panels" were the same branch. They are now different questions with the
   * same answer, and the answer has to still be right.
   */
  test("a world on a system that ships no panels is drawn without them", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eslotnone"));
    const worldId = await createWorld(page, `E2E Panel Slots Bare ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toBeVisible({
      timeout: 15_000,
    });

    await page.getByTestId("system-picker").click();
    await page.getByRole("option", { name: "5E System Core" }).click();
    await expect(page.getByTestId("pending-system-confirmation")).toBeVisible({
      timeout: 10_000,
    });
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByText("System assigned.")).toBeVisible({
      timeout: 15_000,
    });

    // No pack fills `world-settings` for this system, so nothing is drawn
    // there — and nothing throws trying.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toBeVisible({
      timeout: 15_000,
    });
    await expect(
      page.getByTestId("genie-resource-carryover-card"),
    ).toHaveCount(0);

    // Nor `world-staging`. The staging page still renders everything of its
    // own; it simply ends after the invite link.
    await page.goto(`/world/${worldId}/staging`);
    await expect(page.getByTestId("world-staging-page")).toBeVisible({
      timeout: 15_000,
    });
    await expect(
      page.getByTestId("genie-session-panel-wrapper"),
    ).toHaveCount(0);
  });
});
