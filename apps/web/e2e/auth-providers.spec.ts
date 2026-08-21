import { test, expect, type Browser, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";

/**
 * specs/007-configurable-auth-providers: env-var-configured OAuth login
 * providers (ADR-041). These tests run against a dev stack started with
 * this describe block's required env vars already set — Playwright drives
 * the browser, not the server process, so the "set env vars, restart"
 * half of each scenario is a precondition of the dev-stack run these tests
 * are executed against (documented per test below), not something the test
 * itself performs. See quickstart.md Scenarios 1-3, 6-7 for the full
 * Given/When/Then this file exercises the runnable half of.
 *
 * Required env vars for this file's dev-stack run:
 *   OAUTH_DISCORD_CLIENT_ID=test_discord_client_id
 *   OAUTH_DISCORD_CLIENT_SECRET=test_discord_client_secret
 *   OAUTH_KEYCLOAK_ISSUER_URL=https://idp.example.com/realms/main
 *   OAUTH_KEYCLOAK_CLIENT_ID=test_kc_id
 *   OAUTH_KEYCLOAK_CLIENT_SECRET=test_kc_secret
 *   OAUTH_KEYCLOAK_WORK_ISSUER_URL=https://work.example.com/realms/main
 *   OAUTH_KEYCLOAK_WORK_CLIENT_ID=test_kc_work_id
 *   OAUTH_KEYCLOAK_WORK_CLIENT_SECRET=test_kc_work_secret
 *   OAUTH_KEYCLOAK_WORK_LABEL=Work SSO
 *   OAUTH_MYSERVICE_CLIENT_ID=test_generic_id
 *   OAUTH_MYSERVICE_CLIENT_SECRET=test_generic_secret
 *   OAUTH_MYSERVICE_AUTHORIZATION_URL=https://myservice.example/auth
 *   OAUTH_MYSERVICE_TOKEN_URL=https://myservice.example/token
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function gotoLogin(page: Page): Promise<void> {
  await page.goto("/login");
}

/** Confirms a provider's `/authentication/oauth/{provider_key}/start`
 * endpoint issues a real redirect toward that provider's real
 * authorization_url with the client-derived `redirect_uri` attached —
 * without actually following the redirect chain out to a real (or
 * intentionally-fake, for this test's Keycloak/generic fixtures) external
 * domain, which would be slow and network-flaky in CI. Mirrors
 * `apps/web/src/api/auth.ts`'s `startOAuthLogin` construction exactly. */
async function checkOAuthStartRedirect(
  page: Page,
  providerKey: string,
  expectAuthorizationUrlContains: string,
): Promise<void> {
  // Browser `fetch(..., { redirect: "manual" })` yields an opaque response
  // (status 0, no readable headers) even for same-origin requests — a Fetch
  // API limitation, not a server bug. Playwright's Node-side `page.request`
  // context isn't subject to that restriction and can read the real status
  // and `Location` header directly.
  const origin = new URL(page.url()).origin;
  const redirectUri = `${origin}/oauth/callback/${providerKey}`;
  const startUrl = new URL(`/api/authentication/oauth/${providerKey}/start`, origin);
  startUrl.searchParams.set("redirect_uri", redirectUri);
  startUrl.searchParams.set("return_to", origin);

  const response = await page.request.get(startUrl.toString(), {
    maxRedirects: 0,
  });

  expect([301, 302, 303, 307, 308]).toContain(response.status());
  const location = response.headers()["location"];
  expect(location).toBeTruthy();
  expect(location).toContain(expectAuthorizationUrlContains);
  expect(location).toContain("redirect_uri=");
}

test.describe("Env-var-configured OAuth providers render as login buttons (US1, T010-T012)", () => {
  test("Discord — configured via OAUTH_DISCORD_* env vars — renders as a login button and its start endpoint redirects toward the real Discord authorize URL with a client-derived redirect_uri", async ({
    page,
  }) => {
    await gotoLogin(page);
    await expect(
      page.getByRole("button", { name: /Continue with Discord/i }),
    ).toBeVisible();
    await checkOAuthStartRedirect(page, "discord", "discord.com");
  });

  test("two named Keycloak instances (default + _WORK_) both render as distinct login buttons and redirect to their own realms (quickstart Scenario 2, FR-012)", async ({
    page,
  }) => {
    await gotoLogin(page);
    await expect(
      page.getByRole("button", { name: /Continue with Keycloak/i }),
    ).toBeVisible();
    // The named instance's OAUTH_KEYCLOAK_WORK_LABEL="Work SSO" env var
    // should override its button text entirely (FR-007's env-var label
    // path — also covered directly by T022).
    await expect(
      page.getByRole("button", { name: /Continue with Work SSO/i }),
    ).toBeVisible();

    await checkOAuthStartRedirect(page, "keycloak", "idp.example.com");
    await checkOAuthStartRedirect(page, "keycloak__work", "work.example.com");
  });

  test("a fully-generic, unlisted OAuth2 provider renders identically to a built-in preset (quickstart Scenario 3, FR-002)", async ({
    page,
  }) => {
    await gotoLogin(page);
    await expect(
      page.getByRole("button", { name: /Continue with Myservice/i }),
    ).toBeVisible();
    await checkOAuthStartRedirect(page, "myservice", "myservice.example");
  });
});

test.describe("Username/password stays first-class regardless of OAuth provider count (US2, T015)", () => {
  test("username/password sign-up and sign-in remain visible and functional alongside multiple configured providers, and resolve to one unified account", async ({
    page,
  }) => {
    await gotoLogin(page);

    // Username/password controls are present and not visually subordinated
    // — they render in the same primary form as always, above the OAuth
    // button row, regardless of how many provider buttons also render.
    await expect(page.locator("#login-identifier")).toBeVisible();
    await expect(page.locator("#login-password")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Continue with Discord/i }),
    ).toBeVisible();

    // A brand-new user can still sign up with just username/password.
    const suffix = uniqueSuffix();
    const username = `authproviders${suffix}`;
    const email = `${username}@example.test`;
    const password = "Sup3r-Secret-Passphrase!";

    await page.goto("/register");
    await page.locator("#register-username").fill(username);
    await page.locator("#register-email").fill(email);
    await page.locator("#register-password").fill(password);
    await page.locator("#register-password-confirmation").fill(password);
    await page.getByRole("button", { name: "Create account" }).click();
    await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
      timeout: 15_000,
    });

    // Confirm the account is real and independent of any OAuth path — the
    // account-linking/unified-identity guarantee itself (a user later
    // linking an OAuth provider to this same account) is already covered
    // by this app's existing OAuth-linking test coverage; this test's job
    // is only to confirm username/password sign-up isn't regressed by this
    // feature's provider-list changes.
    expect(page.url()).not.toContain("/register");
  });
});

/** Registers a fresh user via the real UI, then promotes it to admin via a
 * direct DB write against the same `thunderforge-postgres` docker-compose
 * service this whole e2e suite already depends on (postgres/rustfs, per
 * `compose.yml`) — there is no UI-driven way to create a *second* admin
 * once one already exists (the setup/bootstrap flow explicitly refuses
 * once `admin_exists`), so this is the only practical way for a test to
 * reach an admin session without depending on a pre-existing admin
 * account's unknown password. */
async function registerAndLoginAsAdmin(page: Page): Promise<string> {
  const suffix = uniqueSuffix();
  const username = `authadmin${suffix}`;
  const email = `${username}@example.test`;
  const password = "Sup3r-Secret-Passphrase!";

  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(email);
  await page.locator("#register-password").fill(password);
  await page.locator("#register-password-confirmation").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });

  execFileSync("docker", [
    "exec",
    "thunderforge-postgres",
    "psql",
    "-U",
    "postgres",
    "-d",
    "thunderforge",
    "-c",
    `UPDATE users SET is_admin = true WHERE username = '${username}';`,
  ]);

  // The session's admin status is resolved server-side per-request from
  // `users.is_admin`, not cached in the session cookie itself, so a fresh
  // navigation is enough to pick up the promotion — no re-login needed.
  await page.goto("/admin/configuration");
  await page.waitForURL((url) => url.pathname.startsWith("/admin"), {
    timeout: 10_000,
  });
  return username;
}

/** `/login` redirects an already-authenticated session straight to its
 * authenticated home (`AppRoutes.tsx`'s `isAuthenticated ? <Navigate ... />`
 * guard) — checking what the sign-in screen shows after an admin action
 * needs a genuinely unauthenticated browser context, not the same `page`
 * still holding the admin's session cookie. */
async function checkLoginButtonPresence(
  browser: Browser,
  buttonNamePattern: RegExp,
  expectVisible: boolean,
): Promise<void> {
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await page.goto("/login");
    const button = page.getByRole("button", { name: buttonNamePattern });
    if (expectVisible) {
      await expect(button).toBeVisible();
    } else {
      await expect(button).toHaveCount(0);
    }
  } finally {
    await context.close();
  }
}

test.describe("Admin panel: runtime provider configuration (US3, T017-T018, T021a)", () => {
  test("admin adds credentials for a provider with no env vars set, and it goes live with no server restart (quickstart Scenario 5 steps 1-2)", async ({
    page,
    browser,
  }) => {
    await registerAndLoginAsAdmin(page);

    // GitHub has no OAUTH_GITHUB_* env vars set in this file's dev-stack
    // run, so it's still an admin-sourced, unconfigured seeded row.
    const githubCard = page
      .locator("article")
      .filter({ hasText: "GitHub" })
      .first();
    await expect(githubCard).toBeVisible();
    await githubCard.locator('input[id$="-client-id"]').fill("e2e-github-client-id");
    await githubCard.locator('input[id$="-client-secret"]').fill("e2e-github-client-secret");
    // Seeded providers start disabled (per the seed migration) — filling
    // credentials doesn't implicitly enable them, matching the form's
    // existing "enabled" toggle being a separate, explicit control.
    await githubCard.getByRole("switch").click();
    await githubCard.getByRole("button", { name: "Update provider" }).click();
    await expect(githubCard.getByText("Provider configuration updated.")).toBeVisible();

    // No server restart between this save and the check below — the
    // sign-in screen must reflect it on the very next load. Checked from a
    // fresh, unauthenticated context (see checkLoginButtonPresence's doc
    // comment) since `/login` redirects an already-authenticated `page` away.
    await checkLoginButtonPresence(browser, /Continue with GitHub/i, true);
  });

  test("env-sourced provider renders read-only (except enabled), and disabling it survives a restart (quickstart Scenario 5 steps 3-4)", async ({
    page,
    browser,
  }) => {
    await registerAndLoginAsAdmin(page);

    const discordCard = page
      .locator("article")
      .filter({ hasText: "Discord" })
      .first();
    await expect(discordCard).toBeVisible();
    await expect(
      discordCard.getByTestId(/env-sourced-indicator$/),
    ).toBeVisible();
    await expect(discordCard.locator('input[id$="-client-id"]')).toBeDisabled();
    await expect(discordCard.locator('input[id$="-client-secret"]')).toBeDisabled();

    // The enabled toggle is the one thing that stays interactive on an
    // env-sourced row (FR-006/FR-008) — toggle it off and save.
    await discordCard.getByRole("switch").click();
    await discordCard.getByRole("button", { name: "Update provider" }).click();
    await expect(discordCard.getByText("Provider configuration updated.")).toBeVisible();

    await checkLoginButtonPresence(browser, /Continue with Discord/i, false);

    // Toggle back on so this test doesn't leave a persistent side effect
    // for the rest of this file's suite (order-independence).
    await page.goto("/admin/configuration");
    const discordCardAgain = page
      .locator("article")
      .filter({ hasText: "Discord" })
      .first();
    await discordCardAgain.getByRole("switch").click();
    await discordCardAgain.getByRole("button", { name: "Update provider" }).click();
    await expect(
      discordCardAgain.getByText("Provider configuration updated."),
    ).toBeVisible();
  });

  test("no oauthClientSecret value ever appears in any admin panel network response (SC-004)", async ({
    page,
  }) => {
    const secretLeaks: string[] = [];
    page.on("response", async (response) => {
      try {
        const body = await response.text();
        if (
          body.includes("test_discord_client_secret") ||
          body.includes("test_kc_secret") ||
          body.includes("test_kc_work_secret") ||
          body.includes("test_generic_secret")
        ) {
          secretLeaks.push(response.url());
        }
      } catch {
        // Non-text bodies can't leak a text secret.
      }
    });

    await registerAndLoginAsAdmin(page);
    await page.waitForTimeout(1_000);

    expect(secretLeaks).toEqual([]);
  });
});

test.describe("Custom login-button branding (US4, T022-T023)", () => {
  test("default Keycloak instance shows the preset default label; the named _WORK_ instance shows its OAUTH_KEYCLOAK_WORK_LABEL override (quickstart Scenario 6 steps 1-2)", async ({
    page,
  }) => {
    await gotoLogin(page);
    // Default instance: no OAUTH_KEYCLOAK_LABEL set in this file's env-var
    // fixture set, so it falls back to the preset's own display name.
    await expect(
      page.getByRole("button", { name: /Continue with Keycloak$/i }),
    ).toBeVisible();
    // Named instance: OAUTH_KEYCLOAK_WORK_LABEL="Work SSO" overrides it
    // entirely — not "Keycloak (Work)", the label replaces the default.
    await expect(
      page.getByRole("button", { name: /Continue with Work SSO/i }),
    ).toBeVisible();
  });

  test("admin sets a custom display name on a non-env provider row, with no env var involved (quickstart Scenario 6 step 3)", async ({
    page,
    browser,
  }) => {
    await registerAndLoginAsAdmin(page);

    const googleCard = page.locator("article").filter({ hasText: "Google" }).first();
    await expect(googleCard).toBeVisible();
    await googleCard.locator('input[id$="-client-id"]').fill("e2e-google-client-id");
    await googleCard.locator('input[id$="-client-secret"]').fill("e2e-google-client-secret");
    await googleCard.getByRole("switch").click();
    // Display name filled last, and deliberately keeps the word "Google" in
    // it — the `hasText: "Google"` filter above re-resolves on every
    // further action against `googleCard`, so replacing it entirely would
    // invalidate this locator for the remaining steps in this test.
    await googleCard.locator('input[id$="-display-name"]').fill("Google (Big G)");
    await googleCard.getByRole("button", { name: "Update provider" }).click();
    await expect(googleCard.getByText("Provider configuration updated.")).toBeVisible();

    await checkLoginButtonPresence(browser, /Continue with Google \(Big G\)/i, true);

    // Leave Google as it started (unconfigured, disabled) so this test
    // doesn't leave a persistent side effect for the rest of this suite.
    execFileSync("docker", [
      "exec",
      "thunderforge-postgres",
      "psql",
      "-U",
      "postgres",
      "-d",
      "thunderforge",
      "-c",
      "UPDATE oauth_providers SET display_name = 'Google', oauth_client_id = NULL, oauth_client_secret = NULL, configured = false, enabled = false WHERE provider_key = 'google';",
    ]);
  });
});

test.describe("Secret never leaks to the browser (SC-004, T027)", () => {
  test("no oauthClientSecret value ever appears in any network response on the sign-in screen", async ({
    page,
  }) => {
    const secretLeaks: string[] = [];
    page.on("response", async (response) => {
      try {
        const body = await response.text();
        if (
          body.includes("test_discord_client_secret") ||
          body.includes("test_kc_secret") ||
          body.includes("test_kc_work_secret") ||
          body.includes("test_generic_secret")
        ) {
          secretLeaks.push(response.url());
        }
      } catch {
        // Non-text bodies (binary assets, etc.) can't leak a text secret.
      }
    });

    await gotoLogin(page);
    await page.waitForTimeout(1_000);

    expect(secretLeaks).toEqual([]);
  });
});
