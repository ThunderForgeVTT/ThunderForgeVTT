import { test, expect, type Page } from "@playwright/test";

/**
 * Spec 034: mirroring a world's lore into a repository its owner controls.
 *
 * # What is covered here, and what is not
 *
 * The unconfigured path (FR-036b) is covered because it is **the state every
 * self-hosted instance starts in** and the easiest to leave broken: nobody
 * developing this feature ever sees it, because their instance is configured.
 * A Game Master must never be shown a flow that cannot complete, and the only
 * way to know that holds is to look at an instance with no application
 * registered — which is exactly what the e2e stack is.
 *
 * The *connected* paths are not covered here, deliberately. Reaching them
 * needs an application registered with a real repository host and a grant
 * completed in that host's UI, which no automated suite on this machine can
 * do without inventing a fake host — and a fake host would prove the fake
 * host works. Those claims are covered where they can be proven for real:
 * `lore_sync::git_roundtrip_tests` drives the actual `git` binary against a
 * local bare repository and asserts the mirror, the rename history, and the
 * divergence refusal.
 *
 * That split is stated rather than left implicit, because "there is no e2e for
 * the happy path" should be a decision someone can disagree with, not an
 * omission they discover.
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

test.describe("Spec 034: an instance with no repository integration", () => {
  /**
   * FR-036b. The failure this rules out is a Game Master clicking "connect",
   * being sent to a repository host, granting access, and coming back to an
   * error — because the instance never had an application registered.
   */
  test("offers a Game Master nothing to connect, and says why", async ({ page }) => {
    await register(page, freshCredentials("e2elore"));
    const worldId = await createWorld(page, `E2E Lore Repo ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);

    const card = page.getByTestId("lore-repository-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    // The unconfigured branch renders, and it explains itself.
    const unconfigured = page.getByTestId("lore-sync-unconfigured");
    await expect(unconfigured).toBeVisible();
    await expect(unconfigured).toContainText(/no repository integration/i);

    // Nothing connectable exists at all — not a disabled button, which a
    // Game Master would sit and wait for.
    await expect(page.getByTestId("lore-sync-connect")).toHaveCount(0);
    await expect(page.getByTestId("lore-sync-notice")).toHaveCount(0);
    await expect(page.getByTestId("lore-sync-acknowledge")).toHaveCount(0);
  });

  /**
   * FR-035 and FR-004c, checked where it actually matters — in what the
   * browser receives. The server type has no credential field to return, and
   * this is the assertion that the shape reaching a client stays that way.
   *
   * Reading the response rather than the rendered page on purpose: a value can
   * be absent from the screen and present in the payload, and it is the
   * payload an attacker reads.
   */
  test("never sends a credential or a host identifier to the browser", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloresec"));
    const worldId = await createWorld(page, `E2E Lore Secrets ${uniqueSuffix()}`);

    const payloads: string[] = [];
    page.on("response", async (response) => {
      if (!response.url().includes("/api/graphql")) return;
      try {
        payloads.push(await response.text());
      } catch {
        // A response body that cannot be read is not evidence of a leak.
      }
    });

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    const all = payloads.join("\n");
    expect(all.length, "no GraphQL response was captured").toBeGreaterThan(0);
    for (const forbidden of [
      "installationRef",
      "installation_ref",
      "hostKind",
      "host_kind",
      "privateKey",
      "PRIVATE KEY",
    ]) {
      expect(all, `${forbidden} reached the browser`).not.toContain(forbidden);
    }
  });

  /**
   * FR-002. Every mutation behind this card is owner-level, so a card offered
   * to someone who cannot use it is an invitation to a permission error.
   */
  test("is not offered to a world member who is not its owner", async ({
    browser,
  }) => {
    const ownerContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const ownerPage = await ownerContext.newPage();
    await register(ownerPage, freshCredentials("e2eloreowner"));
    const worldId = await createWorld(
      ownerPage,
      `E2E Lore Member ${uniqueSuffix()}`,
    );

    await ownerPage.goto(`/world/${worldId}`);
    await ownerPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = ownerPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const memberContext = await browser.newContext();
    const memberPage = await memberContext.newPage();
    await register(memberPage, freshCredentials("e2eloremember"));
    await memberPage.goto(`/join/${inviteCode}`);
    await memberPage.getByRole("button", { name: "Join Campaign" }).click();
    await memberPage.waitForURL(
      (url) => url.pathname.startsWith(`/world/${worldId}`),
      { timeout: 15_000 },
    );

    await memberPage.goto(`/world/${worldId}/settings/system`);
    await expect(memberPage.getByTestId("active-system-card")).toBeVisible({
      timeout: 15_000,
    });
    await expect(memberPage.getByTestId("lore-repository-card")).toHaveCount(0);

    await ownerContext.close();
    await memberContext.close();
  });
});
