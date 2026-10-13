import { expect, type Browser, type Page } from "./test";
import { launchSceneByName } from "./helpers";

/**
 * Account, world, scene and invite helpers shared by the
 * `token-authoring-*.spec.ts` files. Moved here verbatim when
 * `token-authoring.spec.ts` was split by feature.
 */

export function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

/** Spec 009: `/world/:id/play` now lands on the staging page first — the
 * canvas only appears in full-screen mode after clicking "Play". */
export async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
  // Spec 010: "Play" is now a real navigation from /staging to /play.
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
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

export async function register(page: Page, creds: Credentials): Promise<void> {
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

export async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

export async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2etok"));

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  // Spec 010: CreateWorldPage now navigates to /world/{id}/staging (not
  // the canvas directly, and not the dashboard).
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  await clickPlay(page);
  const match = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

/** Registers a brand-new account and returns its user id, captured from
 * the register REST response body (`session.user.id` — see
 * `apps/web/src/types/auth.ts`'s `AuthSessionResponse`) rather than any
 * UI element, since no page in this app currently displays a user's own
 * id anywhere (T021-T023's ownership-grant flow needs it to type into
 * `TokenPanel`'s "Owner user ID" input). */
export async function registerAndGetUserId(
  page: Page,
  prefix: string,
): Promise<string> {
  const creds = freshCredentials(prefix);
  const [response] = await Promise.all([
    page.waitForResponse(
      // `.ok()` (2xx), not `status() === 200`: the register endpoint's
      // real status turned out to be a non-200 2xx (found live — this
      // exact-200 check made the very first version of this helper hang
      // forever waiting for a response that had already arrived).
      (resp) =>
        resp.url().includes("/api/authentication/register") && resp.ok(),
    ),
    (async () => {
      await page.goto("/register");
      await page.locator("#register-username").fill(creds.username);
      await page.locator("#register-email").fill(creds.email);
      await page.locator("#register-password").fill(creds.password);
      await page
        .locator("#register-password-confirmation")
        .fill(creds.password);
      await page.getByRole("button", { name: "Create account" }).click();
    })(),
  ]);
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
  const body = (await response.json()) as {
    session?: { user?: { id?: string } };
  };
  const userId = body.session?.user?.id;
  if (!userId) {
    throw new Error("Could not extract user id from register response");
  }
  return userId;
}

/** As the GM (on the `/world/{worldId}/play` view), briefly navigates to
 * the world dashboard to generate an invite code via
 * `CampaignSettingsPanel` (spec 005 US4), then returns to the play view.
 * Mirrors `invite-membership.spec.ts`'s flow, adapted since this file's
 * `registerAndCreateWorld` (unlike that file's) leaves the GM already
 * inside the play view, not the dashboard. */
export async function generateInviteCodeFromDashboard(
  page: Page,
  worldId: string,
): Promise<string> {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(`/world/${worldId}/players`);
  await page.getByRole("button", { name: "Generate Join Link" }).click();
  const inviteCode = await extractInviteCode(page);
  await page.goto(`/world/${worldId}/play`);
  return inviteCode;
}

/** As a freshly-registered player, redeems an invite code and lands on
 * the world's play view. Mirrors `invite-membership.spec.ts`'s join
 * flow, continuing past the dashboard into `/play` via the same "Enter
 * world" link `registerAndCreateWorld` uses for the GM. */
export async function joinWorldAndEnterPlay(
  page: Page,
  inviteCode: string,
  worldId: string,
): Promise<void> {
  await page.goto(`/join/${inviteCode}`);
  await expect(page.getByRole("button", { name: "Join Campaign" })).toBeVisible(
    {
      timeout: 10_000,
    },
  );
  await page.getByRole("button", { name: "Join Campaign" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}(/actor-select)?$`), {
    timeout: 15_000,
  });

  // Straight to `/play`, rather than through the landing page's "Enter world"
  // link and staging.
  //
  // Spec 017's claim gate (`useActorClaimGate`) redirects *any* non-GM member
  // without an actor claim to `/actor-select` — not only one with characters
  // waiting to be claimed — and that page renders no "Enter world" link. The
  // player here never claims anything, so the redirect always happens.
  //
  // Testing the URL to decide does not work either, and the attempt is worth
  // recording: the redirect is a client-side `navigate(..., { replace: true })`
  // fired from an effect, so it lands *after* the `waitForURL` above resolves.
  // A branch on `page.url()` races it, and losing that race puts the click
  // back on a page that will never show the link.
  //
  // That click used to be unbounded — Playwright's default action timeout is
  // off and this config sets none — so it was limited only by the test's own
  // 480s budget. The spec burned its full eight minutes and then reported
  // `playerContext.close()` failing in the `finally`: a hundred lines from the
  // cause, naming nothing to do with it.
  //
  // `/play` is not behind the gate, which is why `inviteAndJoinAsPlayer` — used
  // by the two player specs above that pass — goes straight there. This spec is
  // about dragging player-owned tokens, not about the invite UI.
  await page.goto(`/world/${worldId}/play`);
}

export async function createScene(page: Page, name: string): Promise<void> {
  // In full-screen canvas mode, "New scene" lives inside the
  // (collapsed-by-default) sidebar. Spec 010: staging is now its own
  // route (not mounted alongside `/play`), so there is exactly one
  // "new-scene-button" in the DOM here — no `:visible` disambiguation
  // against a second, hidden-but-mounted staging copy is needed anymore;
  // just ensure the Settings dock section is actually open before clicking.
  const newSceneButton = page.getByTestId("new-scene-button");
  if (!(await newSceneButton.isVisible().catch(() => false))) {
    await page.getByTestId("world-dock-tab-settings").click();
    await expect(newSceneButton).toBeVisible({ timeout: 10_000 });
  }
  await newSceneButton.click();
  await page.locator('[data-testid="new-scene-name-input"]:visible').fill(name);
  await page.locator('[data-testid="create-scene-submit"]:visible').click();
  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 10_000,
  });
  await expect(
    page.locator('[data-testid="scene-switcher"]:visible'),
  ).toContainText(name);

  // Make it the world's *active* scene, not merely this client's selection.
  //
  // Which scene a reload lands on is server state (ADR-046, spec 022), and
  // creating one through the switcher does not launch it. Every test in this
  // file creates its scene, creates a token in it, and then reloads — which
  // silently returned to the world's auto-created default scene, where that
  // token does not exist. The US3 ownership test is where it showed: its
  // first token was left behind in the abandoned scene while the two created
  // after the reload landed in the default one, so assigning ownership to
  // the first found no such row in the panel.
  const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1];
  if (!worldId) {
    throw new Error(`createScene needs to be on a world route: ${page.url()}`);
  }
  // Back to /play *on this scene*: the `sceneId` query parameter is how
  // several tests below identify the scene whose persisted rows they then
  // assert on, and a bare `/play` drops it.
  const sceneId = await launchSceneByName(page, worldId, name);
  await page.goto(`/world/${worldId}/play?sceneId=${sceneId}`);
}

/** Opens a second, independent browser context reusing the first
 * session's login via `storageState` (same pattern as
 * map-editor-tooling.spec.ts / canvas-authoring-walls.spec.ts's "sync across
 * sessions" tests). Navigates the new page to the same URL as
 * `sourcePage`. */
export async function secondSessionSameLogin(
  browser: Browser,
  sourcePage: Page,
): Promise<{
  context: Awaited<ReturnType<Browser["newContext"]>>;
  page: Page;
}> {
  const sourceContext = sourcePage.context();
  const storageState = await sourceContext.storageState();
  const context = await browser.newContext({ storageState });
  const page = await context.newPage();
  await page.goto(sourcePage.url());
  return { context, page };
}
