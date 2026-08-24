import { expect, type Page } from "@playwright/test";

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

/** The full-screen /play sidebar (scene switcher included) is collapsed
 * by default — open it if it isn't already visible. */
export async function ensureSidebarOpen(page: Page): Promise<void> {
  const switcher = page.getByTestId("scene-switcher");
  if (await switcher.isVisible().catch(() => false)) {
    return;
  }
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  await page.getByTestId("sidebar-toggle-button").dispatchEvent("click");
  await expect(switcher).toBeVisible({ timeout: 10_000 });
}

export async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
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
