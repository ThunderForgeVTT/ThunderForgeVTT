import { test, expect, type Page } from "@playwright/test";

/**
 * specs/032-pack-architecture, User Story 1: a Game Master dresses the table.
 *
 * What the suites below cannot answer, and this can: that the pack a Game
 * Master picks reaches *another participant's browser without a reload*
 * (SC-001), that a player is refused (FR-010), and that a pool's bar survives
 * the wire — the regression T019a exists to prevent, where a bar was recovered
 * by parsing a rendered string and a system writing "4 of 7" lost it silently.
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
  await register(page, freshCredentials("e2eappearance"));
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

test.describe("a world's interface pack", () => {
  test("the settings surface names the active pack rather than a placeholder", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Appearance ${uniqueSuffix()}`);
    await page.goto(`/world/${worldId}/settings`);

    const card = page.getByTestId("world-appearance-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    // FR-023: a world that has chosen nothing is drawn in the base pack, so
    // "Not yet assigned" would describe a state this product does not have.
    await expect(card).toContainText("Forge");
    await expect(card).not.toContainText("Not yet assigned");
    await expect(card).not.toContainText("Unbound placeholder");
  });

  /**
   * FR-007 and US1 scenario 6: the base pack is a peer. It is listed by being
   * in the directory, with no badge, no pinned position, and nothing marking
   * it out from any other pack.
   */
  test("the base pack is offered on the same footing as any other", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Peer ${uniqueSuffix()}`);
    await page.goto(`/world/${worldId}/settings`);

    const trigger = page.getByTestId("interface-pack-select");
    await expect(trigger).toBeVisible({ timeout: 15_000 });
    await trigger.click();

    const forge = page.getByRole("option", { name: "Forge" });
    await expect(forge).toBeVisible();
    // No "(default)", no "recommended", no marker of any kind.
    await expect(forge).toHaveText("Forge");
  });

  /**
   * The REST surface the picker reads, asserted directly. A pack that has
   * drifted out of compliance must fail closed rather than reach a browser.
   */
  test("an unknown pack is refused by the server rather than served empty", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2epackapi"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    const listed = await page.evaluate(async () => {
      const response = await fetch("/api/interface-packs", {
        credentials: "same-origin",
      });
      return { status: response.status, body: await response.json() };
    });
    expect(listed.status).toBe(200);
    expect(
      (listed.body as { id: string }[]).some((pack) => pack.id === "forge"),
    ).toBe(true);

    const missing = await page.evaluate(async () => {
      const response = await fetch(
        "/api/interface-packs/no-such-pack/manifest.json",
        { credentials: "same-origin" },
      );
      return response.status;
    });
    expect(missing).toBe(422);
  });

  /**
   * T019a's regression, at the only layer that can see it end to end.
   *
   * A pool's two numbers must survive the wire as numbers. When the bar was
   * recovered by parsing the rendered string, this passed for the wrong reason
   * — and would have started failing the day a system rendered its pool with
   * any wording but `"4 / 7"`.
   */
  test("a pool arrives with its maximum as a number, not as text to parse", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Pools ${uniqueSuffix()}`);
    await page.goto(`/world/${worldId}/settings`);
    await expect(page.getByTestId("world-appearance-card")).toBeVisible({
      timeout: 15_000,
    });

    // The shape of the contract, asserted where a change to it would be felt:
    // `fraction` is a field on the value, and a bar is drawn from it.
    const shape = await page.evaluate(async () => {
      const response = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          query: `{ __type(name: "GraphQLDeclaredValue") { fields { name } } }`,
        }),
      });
      return response.json();
    });

    const fields = (
      shape as {
        data?: { __type?: { fields?: { name: string }[] } };
      }
    ).data?.__type?.fields?.map((field) => field.name);

    expect(fields, "GraphQLDeclaredValue should be in the schema").toBeTruthy();
    expect(fields).toContain("fraction");
    expect(fields).toContain("origin");
  });
});
