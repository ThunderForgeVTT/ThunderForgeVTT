import { test, expect, type Page } from "@playwright/test";
import { createNpcViaCompendium } from "./fixtures/content";

/**
 * specs/010-world-staging-actors (US3): the DM-only "ownership block" —
 * assigning Viewer/Editor/Owner to any world member, including multiple
 * simultaneous Owners, and the non-DM-cannot-touch-the-block rule.
 *
 * Scope note: this file verifies the ownership-block UI and the
 * resulting edit-permission gating (both DOM-testable). It does not drive
 * an actual canvas token drag to prove live-play token control end to
 * end — that authorization path is covered by the server-side
 * `cargo test`s in `mutations_tokens.rs`/`auth::actor_permissions`
 * (research.md §5); adding full canvas-drag e2e coverage for this one
 * additional authorization branch was judged disproportionate to the
 * marginal coverage it would add over the existing token-drag e2e
 * coverage in `token-authoring.spec.ts`.
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

async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2eownership"));
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

async function currentUserId(page: Page): Promise<string> {
  const cookies = await page.context().cookies();
  const csrfToken = cookies.find((c) => c.name === "csrf_token")?.value;
  const response = await page.request.post("/api/graphql", {
    headers: csrfToken ? { "x-csrf-token": csrfToken } : {},
    data: { query: "query { me { id } }" },
  });
  const payload = (await response.json()) as { data?: { me?: { id?: string } } };
  const id = payload.data?.me?.id;
  if (!id) throw new Error("Could not resolve current user id via /api/graphql");
  return id;
}

async function createNpcAndOpenEdit(page: Page, worldId: string, npcName: string): Promise<string> {
  // Spec 011: NPC creation moved from staging to /compendium.
  await page.goto(`/world/${worldId}/compendium`);
  // Spec 031 FR-035 moved NPC authoring onto its own page, so the
  // fixture creates it and hands back the id — no row to find, no View
  // link to follow.
  const seededActorId = await createNpcViaCompendium(page, worldId, npcName);
  await page.goto(`/world/${worldId}/actor/${seededActorId}/view`);
  await page.waitForURL(new RegExp(`/world/${worldId}/actor/[^/]+/view$`), { timeout: 15_000 });
  await page.getByRole("button", { name: "Edit" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}/actor/([^/]+)/edit$`), { timeout: 15_000 });
  const match = new RegExp(`/world/${worldId}/actor/([^/]+)/edit$`).exec(
    new URL(page.url()).pathname,
  );
  if (!match) throw new Error(`Could not extract actor id from URL: ${page.url()}`);
  return match[1];
}

test.describe("US3: DM assigns ownership; grantee gains edit rights", () => {
  test("DM grants a player Owner, and that player can then edit the actor", async ({
    page,
    browser,
  }) => {
    const worldName = `E2E Ownership ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2eownerplyr"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await playerPage.locator("#world-name").fill(`E2E Player Own ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
      const playerId = await currentUserId(playerPage);

      await playerPage.goto(`/join/${inviteCode}`);
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}(/actor-select)?$`), { timeout: 15_000 });

      await page.goto(`/world/${worldId}/staging`);
      const actorId = await createNpcAndOpenEdit(page, worldId, `Bo Jangles ${uniqueSuffix()}`);

      // DM assigns the player Owner via the ownership block. The player
      // row depends on `useWorldMembers`'s fetch picking up the
      // just-completed join, which can lag behind the GraphQL mutation
      // that caused it — reload once if the row hasn't appeared yet,
      // rather than failing outright on fetch timing.
      await expect(page.getByTestId("actor-ownership-block")).toBeVisible({ timeout: 10_000 });
      const ownershipRow = page.getByTestId(`ownership-row-${playerId}`);
      try {
        await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
      } catch {
        await page.reload();
        await expect(page.getByTestId("actor-ownership-block")).toBeVisible({ timeout: 10_000 });
        await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
      }
      await page.getByTestId(`ownership-select-${playerId}`).selectOption("OWNER");
      await expect(page.getByTestId(`ownership-select-${playerId}`)).toHaveValue("OWNER");

      // The player can now reach the actor's /edit route and save.
      await playerPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
      await expect(playerPage).toHaveURL(new RegExp(`/actor/${actorId}/edit$`));
      await playerPage.locator("#actor-label").fill("Renamed By Owner Player");
      await playerPage.getByRole("button", { name: "Save" }).click();
      await expect(playerPage.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

      // But the player (non-DM) cannot see/change the ownership block,
      // even though they now hold Owner on this very actor.
      await expect(playerPage.getByTestId("actor-ownership-block")).toHaveCount(0);
    } finally {
      await playerContext.close();
    }
  });

  test("multiple simultaneous Owners are both accepted by the ownership block", async ({
    page,
    browser,
  }) => {
    const worldName = `E2E Multi Owner ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);
    const contexts = await Promise.all([browser.newContext(), browser.newContext()]);
    const [playerAPage, playerBPage] = await Promise.all(contexts.map((c) => c.newPage()));
    try {
      const playerIds: string[] = [];
      for (const [index, playerPage] of [playerAPage, playerBPage].entries()) {
        await register(playerPage, freshCredentials(`e2emultiowner${index}`));
        await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
        await playerPage.locator("#world-name").fill(`E2E Multi Own ${index} ${uniqueSuffix()}`);
        await playerPage.getByRole("button", { name: /create world/i }).click();
        await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
        playerIds.push(await currentUserId(playerPage));

        await playerPage.goto(`/join/${inviteCode}`);
        await playerPage.getByRole("button", { name: "Join Campaign" }).click();
        await playerPage.waitForURL(new RegExp(`/world/${worldId}(/actor-select)?$`), { timeout: 15_000 });
      }

      await page.goto(`/world/${worldId}/staging`);
      await createNpcAndOpenEdit(page, worldId, `Shared NPC ${uniqueSuffix()}`);

      await expect(page.getByTestId("actor-ownership-block")).toBeVisible({ timeout: 10_000 });
      for (const playerId of playerIds) {
        const ownershipRow = page.getByTestId(`ownership-row-${playerId}`);
        try {
          await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
        } catch {
          await page.reload();
          await expect(page.getByTestId("actor-ownership-block")).toBeVisible({ timeout: 10_000 });
          await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
        }
        await page.getByTestId(`ownership-select-${playerId}`).selectOption("OWNER");
        await expect(page.getByTestId(`ownership-select-${playerId}`)).toHaveValue("OWNER");
      }
    } finally {
      await Promise.all(contexts.map((c) => c.close()));
    }
  });
});
