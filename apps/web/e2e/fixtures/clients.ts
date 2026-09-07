import type { Browser, Page } from "@playwright/test";
import { login, type Credentials } from "./helpers";

/**
 * Another signed-in client for an account that is already signed in
 * (spec 036 US2, FR-016 and FR-017).
 *
 * # Why this could not exist before
 *
 * `create_session` used to revoke every live session for an account on each
 * login, so a second browser context signed the first one out. Every
 * cross-client assertion in this suite therefore had to be made between two
 * *different people* — `inviteAndJoinAsPlayer` registers a second account
 * rather than opening a second window — and "one person, two windows", which
 * is the far more common real situation, could not be expressed at all.
 *
 * ADR-073 removed that. This is what the removal is for.
 *
 * # Why it signs in rather than copying cookies
 *
 * Copying `storageState` between contexts would produce a working second
 * client whether or not the eviction was removed, which would make this
 * fixture useless as evidence. Signing in for real is exactly what the
 * regression guard in `concurrent-sessions.spec.ts` needs — if login ever
 * revokes again, the first client dies and the test says so.
 */
export async function openAnotherClient(
  browser: Browser,
  creds: Credentials,
  kind: "context",
): Promise<Page>;
export async function openAnotherClient(
  browser: Browser,
  creds: Credentials,
  kind: "tab",
  from: Page,
): Promise<Page>;
export async function openAnotherClient(
  browser: Browser,
  creds: Credentials,
  kind: "context" | "tab",
  from?: Page,
): Promise<Page> {
  if (kind === "tab") {
    if (!from) {
      throw new Error(
        'openAnotherClient(kind: "tab") needs the page to open the tab beside',
      );
    }
    // A second tab in one browser profile shares the session cookie, so there
    // is nothing to sign in to. This is the case that always worked, and it
    // is here so a test can say which of the two it means.
    return from.context().newPage();
  }

  // Its own cookie jar — the real "second browser" case, and the one that
  // used to end the first session.
  const context = await browser.newContext();
  const page = await context.newPage();
  await login(page, creds.username, creds.password);
  await page.waitForURL((url) => !url.pathname.startsWith("/login"), {
    timeout: 15_000,
  });
  return page;
}
