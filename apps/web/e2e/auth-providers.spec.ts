import { test, expect, type Browser, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

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
  const startUrl = new URL(
    `/api/authentication/oauth/${providerKey}/start`,
    origin,
  );
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

interface LiveProvider {
  providerKey: string;
  displayName: string;
}

/** The providers the server considers live — exactly the
 * `enabled AND configured` rows `/authentication/setup/status` builds its
 * payload from, read straight from the table that endpoint's only input.
 *
 * Read over psql rather than over the endpoint deliberately: every auth
 * route sits behind a per-IP-per-path rate limiter, and the SPA already
 * calls `setup/status` on every page load. A test that polls it too just
 * brings the whole dev stack closer to answering 429 — which surfaces as
 * "ThunderForge could not load the current instance state", i.e. as a
 * failure that looks nothing like its cause.
 */
/**
 * Which stack this file's direct SQL talks to.
 *
 * From the environment, matching `global-setup.ts`, because the stack under
 * test is no longer always *the* dev stack: `scripts/e2e-parallel.mjs` runs
 * each shard against its own database, and a shard's backend materialises the
 * env-configured providers into that one at startup.
 *
 * These were four separate literal `thunderforge` / `thunderforge-postgres`
 * pairs. Under sharding every one of them was wrong in a different way — the
 * read below described somebody else's stack, and the writes registered a user
 * in the shard's database and then promoted them in the dev one, which cannot
 * pass and quietly mutates a database no test in this run owns.
 */
const POSTGRES_CONTAINER =
  process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
const POSTGRES_DB = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
const POSTGRES_USER = process.env.THUNDERFORGE_DB_USER ?? "postgres";

/**
 * One `psql` against the stack under test.
 *
 * A failure is restated rather than swallowed: an empty result would skip
 * every scenario here and report a green run, and the raw `psql` error gives
 * no hint that a *test helper* chose the database — which is how this file
 * came to fail in 0ms with the rest of it skipped, saying nothing about why.
 */
function psqlExec(sql: string, extraArgs: string[] = []): string {
  try {
    return execFileSync(
      "docker",
      [
        "exec",
        POSTGRES_CONTAINER,
        "psql",
        "-U",
        POSTGRES_USER,
        "-d",
        POSTGRES_DB,
        ...extraArgs,
        "-c",
        sql,
      ],
      { encoding: "utf-8" },
    );
  } catch (error) {
    throw new Error(
      `psql against ${POSTGRES_CONTAINER}/${POSTGRES_DB} failed: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

function liveProviders(): LiveProvider[] {
  const output = psqlExec(
    "SELECT provider_key, display_name FROM oauth_providers WHERE enabled AND configured ORDER BY provider_key;",
    ["-t", "-A", "-F", "|"],
  );

  return output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [providerKey, displayName] = line.split("|");
      return { providerKey, displayName };
    });
}

/** The OAUTH_* fixture listed in this file's header is a property of the
 * dev stack these tests run against: the server reads those variables once
 * at startup, so a test can neither set them nor restart the server to pick
 * them up. Tests pinned to one *named* fixture provider therefore check it
 * is actually present and skip — naming the variables to set — rather than
 * reporting a dev stack started without the fixture as a product failure.
 * Everything not tied to a specific fixture provider still runs
 * unconditionally. */
function skipUnlessProviderConfigured(
  providerKey: string,
  envVarPrefix: string,
): void {
  test.skip(
    !liveProviders().some((provider) => provider.providerKey === providerKey),
    `Dev stack has no "${providerKey}" OAuth provider; start it with ${envVarPrefix}* set (see this file's header) to exercise this scenario.`,
  );
}

test.describe("Env-var-configured OAuth providers render as login buttons (US1, T010-T012)", () => {
  test("Discord — configured via OAUTH_DISCORD_* env vars — renders as a login button and its start endpoint redirects toward the real Discord authorize URL with a client-derived redirect_uri", async ({
    page,
  }) => {
    skipUnlessProviderConfigured("discord", "OAUTH_DISCORD_");
    await gotoLogin(page);
    await expect(
      page.getByRole("button", { name: /Continue with Discord/i }),
    ).toBeVisible();
    await checkOAuthStartRedirect(page, "discord", "discord.com");
  });

  test("two named Keycloak instances (default + _WORK_) both render as distinct login buttons and redirect to their own realms (quickstart Scenario 2, FR-012)", async ({
    page,
  }) => {
    // The second, *named* instance is what makes this test what it is —
    // one Keycloak alone proves nothing about instance suffixes.
    skipUnlessProviderConfigured("keycloak__work", "OAUTH_KEYCLOAK_WORK_");
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
    skipUnlessProviderConfigured("myservice", "OAUTH_MYSERVICE_");
    await gotoLogin(page);
    await expect(
      page.getByRole("button", { name: /Continue with Myservice/i }),
    ).toBeVisible();
    await checkOAuthStartRedirect(page, "myservice", "myservice.example");
  });
});

test.describe("Username/password stays first-class regardless of OAuth provider count (US2, T015)", () => {
  test("username/password sign-up and sign-in remain visible and functional alongside every configured provider, and resolve to one unified account", async ({
    page,
  }) => {
    // This test signs an account up, which may have to sit out a
    // rate-limit window (see `registerAccount`).
    test.setTimeout(180_000);
    await gotoLogin(page);

    // Username/password controls are present and not visually subordinated
    // — they render in the same primary form as always, above the OAuth
    // button row, regardless of how many provider buttons also render.
    await expect(page.locator("#login-identifier")).toBeVisible();
    await expect(page.locator("#login-password")).toBeVisible();

    // Asserted against whatever the stack under test actually has
    // configured, rather than against one named fixture provider: the
    // claim under test is "regardless of provider count", so pinning it to
    // Discord only ever proved it for one particular dev-stack shape.
    for (const provider of liveProviders()) {
      await expect(
        page.getByRole("button", {
          name: `Continue with ${provider.displayName}`,
        }),
      ).toBeVisible();
    }

    // A brand-new user can still sign up with just username/password.
    await registerAccount(
      page,
      `authproviders${uniqueSuffix()}`,
      "Sup3r-Secret-Passphrase!",
    );

    // Confirm the account is real and independent of any OAuth path — the
    // account-linking/unified-identity guarantee itself (a user later
    // linking an OAuth provider to this same account) is already covered
    // by this app's existing OAuth-linking test coverage; this test's job
    // is only to confirm username/password sign-up isn't regressed by this
    // feature's provider-list changes.
    expect(page.url()).not.toContain("/register");
  });
});

/** Signs one brand-new account up through the real registration form.
 *
 * Retries on 429 rather than failing. `/authentication/register` is capped
 * at 15 requests a minute per IP (`auth_middleware.rs`) — deliberately,
 * it's the credential-stuffing guard — and on a dev stack shared with the
 * rest of the e2e suite that budget can already be spent when this file
 * starts. A 429 there surfaces as a navigation timeout on a page that
 * still shows the form, which reads exactly like a broken sign-up flow and
 * is nothing of the kind; waiting for the window to roll is the only
 * correct response.
 */
async function registerAccount(
  page: Page,
  username: string,
  password: string,
): Promise<void> {
  for (let attempt = 0; ; attempt += 1) {
    await page.goto("/register");
    await page.locator("#register-username").fill(username);
    await page.locator("#register-email").fill(`${username}@example.test`);
    await page.locator("#register-password").fill(password);
    await page.locator("#register-password-confirmation").fill(password);
    const [response] = await Promise.all([
      page.waitForResponse((candidate) =>
        candidate.url().includes("/authentication/register"),
      ),
      page.getByRole("button", { name: "Create account" }).click(),
    ]);

    if (response.status() !== 429) {
      await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
        timeout: 15_000,
      });
      return;
    }

    expect(
      attempt,
      "registration stayed rate-limited for a full window",
    ).toBeLessThan(2);
    // The limiter's window is 60s; half of it is enough for the oldest
    // entries to age out when the budget was only just exhausted.
    test.setTimeout(180_000);
    await page.waitForTimeout(30_000);
  }
}

/** Registers a fresh user via the real UI, then promotes it to admin via a
 * direct DB write against the same `thunderforge-postgres` docker-compose
 * service this whole e2e suite already depends on (postgres/rustfs, per
 * `compose.yml`) — there is no UI-driven way to create a *second* admin
 * once one already exists (the setup/bootstrap flow explicitly refuses
 * once `admin_exists`), so this is the only practical way for a test to
 * reach an admin session without depending on a pre-existing admin
 * account's unknown password. */
async function registerAndLoginAsAdmin(page: Page): Promise<string> {
  const username = `authadmin${uniqueSuffix()}`;

  await registerAccount(page, username, "Sup3r-Secret-Passphrase!");

  psqlExec(`UPDATE users SET is_admin = true WHERE username = '${username}';`);

  // The session's admin status is resolved server-side per-request from
  // `users.is_admin`, not cached in the session cookie itself, so a fresh
  // navigation is enough to pick up the promotion — no re-login needed.
  await gotoAdminConfiguration(page);
  return username;
}

const ADMIN_STATE_PATH = path.join(
  __dirname,
  ".demo",
  "auth-providers-admin-state.json",
);

/** One admin, registered once for the whole file, replayed into each admin
 * test as a stored session.
 *
 * `/authentication/register` is rate-limited to 15 requests a minute per IP
 * (`auth_middleware.rs`) — deliberately, it is the credential-stuffing
 * guard. Registering a throwaway admin per test spent that budget on
 * something every one of those tests wanted identically, and on a busy dev
 * stack the fourth registration came back 429 and the test failed as a
 * navigation timeout with nothing on screen to explain it. */
test.beforeAll(async ({ browser }) => {
  // Registration may have to sit out a rate-limit window (see
  // `registerAccount`), which no 30s hook budget survives.
  test.setTimeout(180_000);
  fs.mkdirSync(path.dirname(ADMIN_STATE_PATH), { recursive: true });
  const context = await browser.newContext();
  try {
    await registerAndLoginAsAdmin(await context.newPage());
    await context.storageState({ path: ADMIN_STATE_PATH });
  } finally {
    await context.close();
  }
});

/** Opens the admin panel and waits for its data, not just its URL: the page
 * shows a full-screen loader until the admin payload arrives, and both
 * `count()` and `filter({ has })` against a still-loading page silently
 * read zero rather than waiting. */
async function gotoAdminConfiguration(page: Page): Promise<void> {
  await page.goto("/admin/configuration");
  await page.waitForURL((url) => url.pathname.startsWith("/admin"), {
    timeout: 10_000,
  });
  await expect(providerCards(page).first()).toBeVisible();
}

/** Every provider row in the admin panel's "OAuth providers" card. The
 * client-id field is what distinguishes them from the page's other
 * `<article>`s. */
function providerCards(page: Page) {
  return page
    .locator("article")
    .filter({ has: page.locator('input[id$="-client-id"]') });
}

/** Puts one seeded, admin-sourced provider row back to how the seed
 * migration leaves it. Both admin tests below flip a provider on and would
 * otherwise be order-dependent against a shared dev database: run twice,
 * the second run's unconditional toggle click would turn back off what the
 * first run left on, and the "it goes live" assertion would fail for a
 * reason that has nothing to do with the app. */
function resetProviderRow(providerKey: string): void {
  psqlExec(
    `UPDATE oauth_providers SET oauth_client_id = NULL, oauth_client_secret = NULL, configured = false, enabled = false WHERE provider_key = '${providerKey}' AND config_source = 'admin';`,
  );
}

function escapeForRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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
  // An explicitly empty storage state, not just a new context: the admin
  // describes below set `storageState`, and Playwright applies the same
  // context options to `browser.newContext()` — so an unqualified new
  // context would arrive holding the admin's session and be redirected
  // off `/login` before it could see a single button.
  const context = await browser.newContext({
    storageState: { cookies: [], origins: [] },
  });
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
  test.use({ storageState: ADMIN_STATE_PATH });

  test("admin adds credentials for a provider with no env vars set, and it goes live with no server restart (quickstart Scenario 5 steps 1-2)", async ({
    page,
    browser,
  }) => {
    // GitHub has no OAUTH_GITHUB_* env vars set in this file's dev-stack
    // run, so it's an admin-sourced row; start it from the seeded state so
    // the toggle click below always means "enable".
    resetProviderRow("github");

    await gotoAdminConfiguration(page);

    const githubCard = page
      .locator("article")
      .filter({ hasText: "GitHub" })
      .first();
    await expect(githubCard).toBeVisible();
    await githubCard
      .locator('input[id$="-client-id"]')
      .fill("e2e-github-client-id");
    await githubCard
      .locator('input[id$="-client-secret"]')
      .fill("e2e-github-client-secret");
    // Seeded providers start disabled (per the seed migration) — filling
    // credentials doesn't implicitly enable them, matching the form's
    // existing "enabled" toggle being a separate, explicit control.
    await githubCard.getByRole("switch").click();
    await githubCard.getByRole("button", { name: "Update provider" }).click();
    await expect(
      githubCard.getByText("Provider configuration updated."),
    ).toBeVisible();

    // No server restart between this save and the check below — the
    // sign-in screen must reflect it on the very next load. Checked from a
    // fresh, unauthenticated context (see checkLoginButtonPresence's doc
    // comment) since `/login` redirects an already-authenticated `page` away.
    await checkLoginButtonPresence(browser, /Continue with GitHub/i, true);

    // Leave GitHub as the seed leaves it, so neither a rerun of this file
    // nor any other spec inherits a live GitHub button.
    resetProviderRow("github");
  });

  test("an env-sourced provider renders read-only (except enabled), and disabling it takes its login button away (quickstart Scenario 5 steps 3-4)", async ({
    page,
    browser,
  }) => {
    await gotoAdminConfiguration(page);

    // Whichever provider *this* dev stack was started with OAUTH_* vars
    // for. The claim under test is "an env-sourced row is read-only except
    // for enabled", not "Discord is env-sourced" — which provider carries
    // env credentials is a property of the stack (see the header's env
    // fixture), so the row is found by the indicator rather than by name.
    const envCard = page
      .locator("article")
      .filter({ has: page.getByTestId(/env-sourced-indicator$/) })
      .first();
    test.skip(
      (await envCard.count()) === 0,
      "Dev stack has no env-sourced OAuth provider; start it with any OAUTH_<PROVIDER>_* set (see this file's header) to exercise this scenario.",
    );
    await expect(envCard).toBeVisible();
    await expect(envCard.locator('input[id$="-display-name"]')).toBeDisabled();
    await expect(envCard.locator('input[id$="-client-id"]')).toBeDisabled();
    await expect(envCard.locator('input[id$="-client-secret"]')).toBeDisabled();

    // The card's heading is the display name the login screen renders, so
    // the button this row is responsible for can be named without knowing
    // which provider it is.
    const displayName = (
      await envCard.locator("h3").first().innerText()
    ).trim();
    const loginButton = new RegExp(
      `Continue with ${escapeForRegExp(displayName)}$`,
      "i",
    );

    const save = async (card: typeof envCard): Promise<void> => {
      await card.getByRole("button", { name: "Update provider" }).click();
      await expect(
        card.getByText("Provider configuration updated."),
      ).toBeVisible();
    };

    // Normalise first: an earlier interrupted run could have left this row
    // switched off, and "disabling it removes the button" says nothing if
    // the button was already gone.
    const toggle = envCard.getByRole("switch");
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await save(envCard);
    }
    await checkLoginButtonPresence(browser, loginButton, true);

    // The enabled toggle is the one thing that stays interactive on an
    // env-sourced row (FR-006/FR-008) — toggle it off and save.
    await toggle.click();
    await save(envCard);
    await checkLoginButtonPresence(browser, loginButton, false);

    // Toggle back on so this test doesn't leave a persistent side effect
    // for the rest of this file's suite (order-independence).
    await page.goto("/admin/configuration");
    const envCardAgain = page
      .locator("article")
      .filter({ has: page.getByTestId(/env-sourced-indicator$/) })
      .first();
    await envCardAgain.getByRole("switch").click();
    await save(envCardAgain);
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

    await gotoAdminConfiguration(page);
    await page.waitForTimeout(1_000);

    expect(secretLeaks).toEqual([]);
  });
});

test.describe("Custom login-button branding (US4, T022-T023)", () => {
  test("default Keycloak instance shows the preset default label; the named _WORK_ instance shows its OAUTH_KEYCLOAK_WORK_LABEL override (quickstart Scenario 6 steps 1-2)", async ({
    page,
  }) => {
    skipUnlessProviderConfigured("keycloak__work", "OAUTH_KEYCLOAK_WORK_");
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

  // Nested so the sign-in-screen test above keeps an unauthenticated
  // context: `/login` bounces an authenticated session straight to its
  // authenticated home.
  test.describe("with an admin session", () => {
    test.use({ storageState: ADMIN_STATE_PATH });

    test("admin sets a custom display name on a non-env provider row, with no env var involved (quickstart Scenario 6 step 3)", async ({
      page,
      browser,
    }) => {
      // Start from the seeded state, so the toggle click below always
      // means "enable" however a previous run ended.
      resetProviderRow("google");

      await gotoAdminConfiguration(page);

      const googleCard = page
        .locator("article")
        .filter({ hasText: "Google" })
        .first();
      await expect(googleCard).toBeVisible();
      await googleCard
        .locator('input[id$="-client-id"]')
        .fill("e2e-google-client-id");
      await googleCard
        .locator('input[id$="-client-secret"]')
        .fill("e2e-google-client-secret");
      await googleCard.getByRole("switch").click();
      // Display name filled last, and deliberately keeps the word "Google" in
      // it — the `hasText: "Google"` filter above re-resolves on every
      // further action against `googleCard`, so replacing it entirely would
      // invalidate this locator for the remaining steps in this test.
      await googleCard
        .locator('input[id$="-display-name"]')
        .fill("Google (Big G)");
      await googleCard.getByRole("button", { name: "Update provider" }).click();
      await expect(
        googleCard.getByText("Provider configuration updated."),
      ).toBeVisible();

      await checkLoginButtonPresence(
        browser,
        /Continue with Google \(Big G\)/i,
        true,
      );

      // Leave Google as it started (unconfigured, disabled) so this test
      // doesn't leave a persistent side effect for the rest of this suite.
      psqlExec(
        "UPDATE oauth_providers SET display_name = 'Google', oauth_client_id = NULL, oauth_client_secret = NULL, configured = false, enabled = false WHERE provider_key = 'google';",
      );
    });
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
