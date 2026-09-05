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

/**
 * User Story 2's promise, checked where it is actually made: **no failure of
 * the remote may affect in-app lore.**
 *
 * These do not simulate a broken remote — they assert the stronger and simpler
 * thing that makes simulating one unnecessary. Nothing in Stories 1 and 2
 * writes to a lore table, so lore behaves identically whether a repository is
 * connected, broken, or absent. The e2e that would "break the host" would be
 * testing a mock; this tests the property.
 *
 * The failure-mode table itself (unreachable host, revoked grant, force-pushed
 * branch, deleted repository) is exercised where those things are real:
 * `lore_sync::git_roundtrip_tests` drives the actual git binary against a local
 * bare repository, and its divergence test has a second clone rewrite the
 * branch mid-pass and asserts the push is refused with the other commit intact.
 */
test.describe("Spec 034 User Story 2: a world is unharmed by its mirror", () => {
  /**
   * SC-006: zero instances of in-app lore being altered, hidden or lost across
   * every failure mode. The structural version of that claim.
   */
  test("lore can be created and read with the mirror surface live", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloreharm"));
    const worldId = await createWorld(page, `E2E Lore Unharmed ${uniqueSuffix()}`);

    // Visit the settings surface first, so the connection machinery is loaded
    // and answering rather than never having been asked.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    // Then do the thing FR-028 protects: read and write lore.
    const title = `Unharmed ${uniqueSuffix()}`;
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Lore" }).click();
    await page.getByTestId("new-lore-entry-title-input").fill(title);
    await page.getByTestId("add-lore-entry-button").click();

    const row = page
      .getByTestId("lore-catalog-table")
      .locator("tr", { hasText: title });
    await expect(row).toBeVisible({ timeout: 15_000 });
    await row.getByRole("link", { name: "View" }).click();
    await page.waitForURL(/\/world\/[^/]+\/lore\/[^/]+\/view$/, {
      timeout: 15_000,
    });

    const slug = /\/lore\/([^/]+)\/view$/.exec(new URL(page.url()).pathname)?.[1];
    if (!slug) throw new Error("no slug");

    // Reading the entry back is the half FR-028 is about: lore behaves exactly
    // as it does in a world with no connection. Editing is covered thoroughly
    // by `lore-wiki.spec.ts`; duplicating its editor interaction here would
    // couple this spec to that surface's markup for no extra assurance.
    // The breadcrumb rather than the rendered markdown: a freshly created
    // entry has an empty body, so there is no markdown block to find, and
    // asserting one would fail for a reason that has nothing to do with this
    // spec.
    await page.goto(`/world/${worldId}/lore/${slug}/view`);
    await expect(page.getByTestId("lore-breadcrumb")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("lore-breadcrumb")).toContainText(title);
  });
});

test.describe("Spec 034: an instance with no repository integration", () => {
  /**
   * FR-036b. The failure this rules out is a Game Master clicking "connect",
   * being sent to a repository host, granting access, and coming back to an
   * error — because the instance never had an application registered.
   */
  test("shows exactly one of the two states, and never a broken flow", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2elore"));
    const worldId = await createWorld(page, `E2E Lore Repo ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);

    const card = page.getByTestId("lore-repository-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    // Which branch renders depends on whether *this instance* has an
    // application registered, and the suite must not assume either. An earlier
    // version asserted the unconfigured branch unconditionally and started
    // failing the moment a real application was configured — the test was
    // right, its assumption was not.
    const unconfigured = page.getByTestId("lore-sync-unconfigured");
    const isUnconfigured = (await unconfigured.count()) > 0;

    if (isUnconfigured) {
      // FR-036b. Nothing connectable exists at all — not a disabled button,
      // which a Game Master would sit and wait for.
      await expect(unconfigured).toContainText(/no repository integration/i);
      await expect(page.getByTestId("lore-sync-connect")).toHaveCount(0);
      await expect(page.getByTestId("lore-sync-notice")).toHaveCount(0);
      await expect(page.getByTestId("lore-sync-acknowledge")).toHaveCount(0);
    } else {
      // Configured: a connect affordance exists, and the pre-synchronisation
      // notice has NOT been skipped — FR-038's gate is not something a
      // configured instance gets to bypass.
      await expect(page.getByTestId("lore-sync-connect")).toHaveCount(1);
      await expect(page.getByTestId("lore-sync-acknowledge")).toHaveCount(0);
    }

    // True in both states, and the reason the branch matters: a Game Master is
    // never shown a half-built flow.
    const connectable = await page.getByTestId("lore-sync-connect").count();
    const explained = await unconfigured.count();
    expect(
      connectable + explained,
      "the card showed neither a way forward nor a reason there is none",
    ).toBeGreaterThan(0);
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
