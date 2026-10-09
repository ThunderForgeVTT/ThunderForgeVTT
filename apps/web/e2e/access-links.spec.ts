import { test, expect, type Page } from "./fixtures/test";

/**
 * specs/027-unified-access-links (US1, US3, US4): a GM can kill a leaked
 * invite link.
 *
 * The control that matters is Scenario 1's ordering — join with the code
 * *first*, so that when the old code is refused after rotation we know it was
 * rotation that did it, and not that the link never worked.
 *
 * No Bevy canvas surface here, so this escapes the documented "headless
 * Chromium can't render the canvas" limitation that blocks canvas specs.
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
}

async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2elink"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

/**
 * Opens the world's links. Spec 088 (FR-001) moved them from the world
 * dashboard to the players page, shown only to those who run the world.
 */
async function openWorldLinks(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/players`);
  await expect(page.getByTestId("world-links-panel")).toBeVisible({
    timeout: 15_000,
  });
}

/** Reads the code out of the first link row's readonly URL field. */
async function firstLinkCode(page: Page): Promise<string> {
  const url = await page
    .getByTestId("invite-link-row")
    .first()
    .getByLabel("Invite link")
    .inputValue();
  const code = url.split("/join/")[1];
  expect(code, `could not parse a code out of ${url}`).toBeTruthy();
  return code;
}

async function generateLink(page: Page): Promise<string> {
  await page.getByRole("button", { name: /generate join link/i }).click();
  await expect(page.getByTestId("invite-link-row").first()).toBeVisible({
    timeout: 15_000,
  });
  return firstLinkCode(page);
}

test.describe("world access links", () => {
  // Each of these registers two or three fresh accounts and creates a world,
  // every one a full page load against the dev server. That legitimately
  // exceeds Playwright's 30s default — the tests are slow, not hung — and
  // running into it produced "locator.fill: Test ended" failures that looked
  // like product bugs.
  test.describe.configure({ timeout: 120_000 });

  test("rotating a link kills the old code and issues a working one", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(
      gmPage,
      `Leak Test ${uniqueSuffix()}`,
    );
    await openWorldLinks(gmPage, worldId);
    const originalCode = await generateLink(gmPage);

    // Control: the code works before rotation. Without this the refusal
    // below would prove nothing.
    const firstJoiner = await browser.newContext();
    const firstPage = await firstJoiner.newPage();
    await register(firstPage, freshCredentials("e2ejoin1"));
    await firstPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await firstPage.goto(`/join/${originalCode}`);
    await firstPage.getByRole("button", { name: /join campaign/i }).click();
    await firstPage.waitForURL(new RegExp(`/world/${worldId}`), {
      timeout: 15_000,
    });
    await firstJoiner.close();

    // The GM notices the leak and refreshes the link.
    await openWorldLinks(gmPage, worldId);
    await gmPage.getByTestId("invite-link-refresh").first().click();
    await expect
      .poll(async () => firstLinkCode(gmPage), { timeout: 15_000 })
      .not.toBe(originalCode);
    const replacementCode = await firstLinkCode(gmPage);

    // The retired code is refused on its very next use.
    const secondJoiner = await browser.newContext();
    const secondPage = await secondJoiner.newPage();
    await register(secondPage, freshCredentials("e2ejoin2"));
    await secondPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await secondPage.goto(`/join/${originalCode}`);
    // Spec 088 (FR-008): a refreshed link reads as withdrawn.
    await expect(
      secondPage.getByRole("heading", { name: /this link was withdrawn/i }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      secondPage.getByRole("button", { name: /join campaign/i }),
    ).toHaveCount(0);

    // The replacement works.
    await secondPage.goto(`/join/${replacementCode}`);
    await secondPage.getByRole("button", { name: /join campaign/i }).click();
    await secondPage.waitForURL(new RegExp(`/world/${worldId}`), {
      timeout: 15_000,
    });
    await secondJoiner.close();

    await gmContext.close();
  });

  test("revoking a link retires it and the panel says so", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(
      gmPage,
      `Revoke Test ${uniqueSuffix()}`,
    );
    await openWorldLinks(gmPage, worldId);
    const code = await generateLink(gmPage);

    const row = gmPage.getByTestId("invite-link-row").first();
    await expect(row.getByTestId("invite-link-state")).toHaveText(/active/i);

    // Revoke asks first — it cannot be undone.
    await row.getByTestId("invite-link-revoke").click();
    await row.getByTestId("invite-link-revoke-confirm").click();

    // FR-010 (027), FR-003 (088): a revoked link stays listed, under past
    // links, so a GM can see what they retired.
    const past = gmPage.getByTestId("world-links-past");
    await expect(past).toBeVisible({ timeout: 15_000 });
    await past.locator("summary").click();
    const retired = past.getByTestId("invite-link-row");
    await expect(retired).toHaveCount(1);
    await expect(retired.getByTestId("invite-link-state")).toHaveText(
      /revoked/i,
    );
    await expect(gmPage.getByTestId("invite-link-list")).toHaveCount(0);

    // And the code is dead.
    const joiner = await browser.newContext();
    const joinerPage = await joiner.newPage();
    await register(joinerPage, freshCredentials("e2ejoin3"));
    await joinerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await joinerPage.goto(`/join/${code}`);
    await expect(
      joinerPage.getByRole("heading", { name: /this link was withdrawn/i }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      joinerPage.getByRole("button", { name: /join campaign/i }),
    ).toHaveCount(0);
    await joiner.close();

    await gmContext.close();
  });

  test("a revoked code and a never-issued one each say why, and neither names the world", async ({
    browser,
  }) => {
    // Spec 088 (FR-008) replaces 027's FR-011: a signed-in visitor reads why
    // a link admits no one. The world behind a dead link stays unnamed.
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(
      gmPage,
      `Uniform Test ${uniqueSuffix()}`,
    );
    await openWorldLinks(gmPage, worldId);
    const code = await generateLink(gmPage);

    const row = gmPage.getByTestId("invite-link-row").first();
    await row.getByTestId("invite-link-revoke").click();
    await row.getByTestId("invite-link-revoke-confirm").click();
    await expect(gmPage.getByTestId("world-links-past")).toBeVisible({
      timeout: 15_000,
    });

    const visitor = await browser.newContext();
    const visitorPage = await visitor.newPage();
    await register(visitorPage, freshCredentials("e2ejoin4"));
    await visitorPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    const messageFor = async (
      candidate: string,
      heading: RegExp,
    ): Promise<string> => {
      await visitorPage.goto(`/join/${candidate}`);
      await expect(
        visitorPage.getByRole("heading", { name: heading }),
      ).toBeVisible({ timeout: 15_000 });
      await expect(
        visitorPage.getByRole("button", { name: /join campaign/i }),
      ).toHaveCount(0);
      return (await visitorPage.locator("main").innerText()).trim();
    };

    const revokedMessage = await messageFor(code, /this link was withdrawn/i);
    const unknownMessage = await messageFor(
      "ZZZZZZZZZZZZZZZZZZZZZZZZZZ",
      /no world behind this link/i,
    );

    expect(revokedMessage).not.toContain("Uniform Test");
    expect(unknownMessage).not.toContain("Uniform Test");

    await visitor.close();
    await gmContext.close();
  });
});
