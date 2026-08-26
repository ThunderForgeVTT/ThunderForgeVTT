import { expect, type Browser, type Page } from "@playwright/test";

/**
 * Shared UI-driven e2e helpers, extracted from the near-identical
 * register/registerAndCreateWorld duplicated across most spec files.
 * New specs should import from here instead of re-implementing these.
 */

export function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

export interface Credentials {
  username: string;
  email: string;
  password: string;
}

export function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

/** Logs in an already-existing user (e.g. the SQL-seeded demo user from
 * src/server/seeds/e2e_demo.sql) by identifier (username or email) +
 * password. */
export async function login(
  page: Page,
  identifier: string,
  password: string,
): Promise<void> {
  await page.goto("/login");
  await page.locator("#login-identifier").fill(identifier);
  await page.locator("#login-password").fill(password);
  await page.getByRole("button", { name: /sign in/i }).click();
}

export async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
  // Wait for the post-registration redirect before returning: a caller
  // that immediately does its own page.goto() (a full navigation, not a
  // SPA route change) can otherwise race/abort the register mutation's
  // still-in-flight fetch/cookie-set, landing on /login instead — found
  // live while writing genie-resource-trade.spec.ts. Existing callers
  // that already wait for a more specific URL right after this
  // (registerAndCreateWorld's own `/worlds/create$` wait, etc.) are
  // unaffected — this resolves immediately once already past /register.
  // `waitUntil: "commit"` (rather than the default "load"): WelcomePage
  // immediately client-side-redirects a zero-world account straight to
  // /worlds/create (see WelcomePage.tsx's zero-worlds redirect), so a
  // freshly registered account can chain two navigations
  // (/register → /welcome → /worlds/create) within milliseconds. Waiting
  // for the "load" lifecycle event of a navigation that gets superseded
  // by the next one before it fires left this hanging until timeout even
  // though the URL predicate was already satisfied — most reproducible
  // under load (many concurrent contexts/requests, as most multi-account
  // specs have). "commit" only waits for navigation to start, which is
  // enough to observe the URL change and is not racy the same way.
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
    waitUntil: "commit",
  });
}

/** Registers a fresh user (prefixed for readability in failure output) and
 * creates a world, leaving no game system explicitly selected so it picks
 * up the server-side "genie" default. Returns the new world's id. */
export async function registerAndCreateWorld(
  page: Page,
  worldName: string,
  credentialPrefix = "e2e",
): Promise<string> {
  await register(page, freshCredentials(credentialPrefix));
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

export async function graphql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  return page.evaluate(
    async ({ query, variables }) => {
      const csrfToken = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
        },
        body: JSON.stringify({ query, variables }),
      });
      const text = await res.text();
      try {
        return JSON.parse(text);
      } catch {
        throw new Error(`Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`);
      }
    },
    { query, variables },
  );
}

/** The Play dock's Settings section (scene switcher, map import, "back to
 * setup") is collapsed by default — open it if it isn't already. */
export async function ensureSidebarOpen(page: Page): Promise<void> {
  const switcher = page.getByTestId("scene-switcher");
  if (await switcher.isVisible().catch(() => false)) {
    return;
  }
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  await page.getByTestId("world-dock-tab-settings").dispatchEvent("click");
  await expect(switcher).toBeVisible({ timeout: 10_000 });
}

export async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

/**
 * Invites a fresh registered user into `worldId` as a Player and returns
 * their logged-in `Page`, in a separate browser context (so GM and
 * player sessions are genuinely independent cookies/tabs, matching how
 * two real people would connect). `gmPage` must already be on a page
 * showing the "Generate Join Link" control (e.g. the world dashboard).
 */
export async function inviteAndJoinAsPlayer(
  browser: Browser,
  gmPage: Page,
  worldId: string,
  credentialPrefix = "e2eplayer",
): Promise<Page> {
  await gmPage.goto(`/world/${worldId}`);
  await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
  const inviteInput = gmPage.locator("input[readonly]").first();
  await expect(inviteInput).toBeVisible({ timeout: 10_000 });
  const inviteUrl = await inviteInput.inputValue();
  const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
  if (!inviteCode) {
    throw new Error("Could not extract invite code");
  }

  const playerContext = await browser.newContext();
  const playerPage = await playerContext.newPage();
  await register(playerPage, freshCredentials(credentialPrefix));
  await playerPage.goto(`/join/${inviteCode}`);
  await playerPage.getByRole("button", { name: "Join Campaign" }).click();
  await playerPage.waitForURL((url) => url.pathname.startsWith(`/world/${worldId}`), {
    timeout: 15_000,
  });
  return playerPage;
}

/**
 * Spec 022: launches `sceneName` from the Scenes section — the one way
 * to select what's being played (FR-002/FR-002a). `page` must belong to
 * a GM/Owner of `worldId`.
 */
export async function launchSceneByName(page: Page, worldId: string, sceneName: string): Promise<void> {
  await page.goto(`/world/${worldId}/scenes`);
  await page.getByRole("link", { name: sceneName }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });
  await page.getByTestId("launch-scene-button").click();
  await expect(page.getByText("Scene launched.")).toBeVisible({ timeout: 10_000 });
}

export async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(3_000);
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (box) {
    await page.mouse.click(box.x + box.width - 40, box.y + box.height - 40);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }
}
