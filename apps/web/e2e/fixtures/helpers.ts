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
        throw new Error(
          `Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`,
        );
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
 * Invite a fresh registered user into `worldId` as a Player, and return their
 * logged-in `Page` in its own browser context.
 *
 * The two sessions are genuinely independent — separate cookies, separate tab,
 * separate engine instance — because that is what the specs using this are
 * about: what a Game Master sees against what a player sees, at the same
 * moment, on the same scene.
 *
 * # Why membership is created over GraphQL rather than clicked
 *
 * It used to drive the whole invite flow through the UI: click "Generate Join
 * Link", read the code out of a readonly input, register, visit `/join/:code`,
 * click "Join Campaign". Two of those buttons intermittently never appeared
 * under suite load, and the test then died on a 300-second timeout.
 *
 * That made this helper the single largest source of noise in the suite, and
 * the noise *moved*: across four full runs it took out `interactive-prop`
 * (twice, at two different buttons), `genie-resource-trade`, and
 * `invite-membership`. Whichever spec happened to be holding it when the
 * machine was busy failed, so the failure list looked broad and unrelated and
 * changed between runs. Run alone, its victims passed in seconds.
 *
 * The deeper problem was that roughly twenty specs were paying to exercise the
 * invite *UI* when none of them are about it. Exactly one spec is —
 * `invite-membership.spec.ts` — and it still clicks every button, because that
 * is its subject. Everything else gets its player the short way.
 *
 * Still real in every way that matters to the callers: a real second account,
 * a real registration, a real session cookie, and a real `joinWorld` mutation
 * with the server enforcing membership exactly as it would have.
 */
export async function inviteAndJoinAsPlayer(
  browser: Browser,
  gmPage: Page,
  worldId: string,
  credentialPrefix = "e2eplayer",
): Promise<Page> {
  const invite = await graphql<{
    data?: { generateInviteCode?: { inviteCode: string } };
    errors?: { message: string }[];
  }>(
    gmPage,
    `
      mutation ($input: GenerateInviteCodeInput!) {
        generateInviteCode(input: $input) {
          inviteCode
        }
      }
    `,
    { input: { worldId, maxUses: 10 } },
  );
  const inviteCode = invite.data?.generateInviteCode?.inviteCode;
  if (!inviteCode) {
    throw new Error(
      `Could not generate an invite: ${JSON.stringify(invite.errors ?? invite)}`,
    );
  }

  const playerContext = await browser.newContext();
  const playerPage = await playerContext.newPage();
  await register(playerPage, freshCredentials(credentialPrefix));

  const joined = await graphql<{
    data?: { joinWorld?: { id: string } };
    errors?: { message: string }[];
  }>(
    playerPage,
    `
      mutation ($input: JoinWorldInput!) {
        joinWorld(input: $input) {
          id
        }
      }
    `,
    { input: { inviteCode } },
  );
  if (!joined.data?.joinWorld?.id) {
    throw new Error(
      `Could not join the world: ${JSON.stringify(joined.errors ?? joined)}`,
    );
  }

  // Left on the world, as the clicked flow left it, so callers that go
  // straight to asserting on the page still work.
  await playerPage.goto(`/world/${worldId}`);
  return playerPage;
}

/**
 * Spec 022: launches `sceneName` from the Scenes section — the one way
 * to select what's being played (FR-002/FR-002a). `page` must belong to
 * a GM/Owner of `worldId`.
 */
export async function launchSceneByName(
  page: Page,
  worldId: string,
  sceneName: string,
): Promise<string> {
  await page.goto(`/world/${worldId}/scenes`);
  await page.getByRole("link", { name: sceneName }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), {
    timeout: 10_000,
  });
  // Captured before the launch click, because the detail route's path is the
  // only place the new scene's id is stated plainly. Callers need it to build
  // a `/play?sceneId=` URL: several specs read the scene under test out of
  // that query parameter, and a bare `/play` leaves them querying whichever
  // scene the server considers active — which is not always this one, and
  // fails as "the token I just made does not exist".
  const sceneId = /\/scenes\/([^/?#]+)/.exec(new URL(page.url()).pathname)?.[1];
  if (!sceneId) {
    throw new Error(`could not read a scene id from ${page.url()}`);
  }
  await page.getByTestId("launch-scene-button").click();
  // Launch enters play (spec 031 FR-021), so arriving at the play view *is*
  // the confirmation. It used to leave the GM on the scene page with a "Scene
  // launched." message, which is what this waited for — and which meant the
  // players were looking at a map the person who launched it was not.
  await page.waitForURL(new RegExp(`/world/${worldId}/play`), {
    timeout: 15_000,
  });
  return sceneId;
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

/** The GM's authoring tools, as the left-hand rail names them. */
/**
 * Re-exported from the rail itself rather than restated.
 *
 * This was a hand-written copy — `"walls" | "lights" | "shapes" | "tokens"` —
 * and it had drifted: the rail has grown `select` and `interactions` since,
 * and nothing noticed because `e2e/` was outside the typechecked set. A test
 * that armed either one was a type error nobody was told about.
 *
 * `openGmTool` below still only works for tools that *have* a panel, which is
 * a runtime property rather than a type one: Select renders no flyout by
 * design, so waiting for `gm-tool-panel-select` waits forever. That is a
 * caller's problem to know, and is documented on the function.
 */
export type { GmToolId } from "@/components/world/GmToolRail/GmToolRail";
import type { GmToolId } from "@/components/world/GmToolRail/GmToolRail";

/**
 * Open one of the GM's authoring tools and wait for its panel.
 *
 * # Why every spec that touches a tool has to call this
 *
 * The rail mounts **only the open tool's content** — deliberately, so that a
 * tool the GM is not using does not leave listeners attached to the canvas
 * (ShapeTool's text sub-tool is the case that forced it). So `wall-tool`,
 * `shape-tool` and the rest are simply not in the DOM until their icon is
 * clicked, and a spec that asserts one is visible straight after the engine
 * loads is describing the layout this replaced.
 *
 * Idempotent: clicking an already-open tool would close it, so this checks
 * first. That matters because the rail is a toggle, and a helper that blindly
 * clicked would turn "make sure this is open" into "flip it".
 */
export async function openGmTool(page: Page, tool: GmToolId): Promise<void> {
  const panel = page.getByTestId(`gm-tool-panel-${tool}`);
  if (await panel.isVisible().catch(() => false)) {
    return;
  }
  await page.getByTestId(`gm-tool-${tool}`).click();
  await expect(panel).toBeVisible({ timeout: 10_000 });
}

/**
 * Open one of the right-hand dock's sections — `chat`, `actors`, `combat`,
 * `clocks`, `settings`.
 *
 * Same shape and same reason as `openGmTool`: the dock is a toggle, and its
 * sections are what "the sidebar" used to be.
 */
export async function openDockTab(page: Page, tab: string): Promise<void> {
  const trigger = page.getByTestId(`world-dock-tab-${tab}`);
  await expect(trigger).toBeVisible({ timeout: 10_000 });
  if ((await trigger.getAttribute("aria-expanded")) === "true") {
    return;
  }
  await trigger.click();
}

/**
 * Wait until the engine's world store holds at least `atLeast` walls.
 *
 * # Why a reload needs this and a first load does not
 *
 * `waitForEngineReady` waits for the canvas and then settles for a fixed few
 * seconds — which is enough for the engine to start, and says nothing about
 * whether the *scene's content* has arrived. On a first load there is nothing
 * to arrive. After a reload there is, and it comes over a separate round trip:
 * the walls are fetched and dispatched into the store after the canvas is
 * already up.
 *
 * Clicking in that window selects nothing, because the wall the click is aimed
 * at does not exist yet on this client. The symptom is a "Selected wall" panel
 * that never appears, which reads exactly like selection being broken — it
 * cost a long debugging session to find that the store, the props and the DOM
 * were all correct and simply arrived later than the click.
 *
 * Polling the store rather than sleeping longer, because "how long does a
 * refetch take" is not a constant and a fixed wait is either flaky or slow.
 */
export async function waitForWallsLoaded(
  page: Page,
  atLeast = 1,
): Promise<void> {
  await expect
    .poll(
      async () =>
        page.evaluate(async () => {
          const bevy = (await import(
            /* @vite-ignore */ "/src/engine/bevy/index.ts"
          )) as typeof import("../../src/engine/bevy/index");
          const state = bevy.getBoundWorldStore()?.getState();
          return Object.keys(state?.walls ?? {}).length;
        }),
      {
        message: `the scene's walls should reach this client (expected at least ${atLeast})`,
        timeout: 30_000,
      },
    )
    .toBeGreaterThanOrEqual(atLeast);
}

/** As `waitForWallsLoaded`, for shapes. */
export async function waitForShapesLoaded(
  page: Page,
  atLeast = 1,
): Promise<void> {
  await expect
    .poll(
      async () =>
        page.evaluate(async () => {
          const bevy = (await import(
            /* @vite-ignore */ "/src/engine/bevy/index.ts"
          )) as typeof import("../../src/engine/bevy/index");
          const state = bevy.getBoundWorldStore()?.getState();
          return Object.keys(state?.shapes ?? {}).length;
        }),
      {
        message: `the scene's shapes should reach this client (expected at least ${atLeast})`,
        timeout: 30_000,
      },
    )
    .toBeGreaterThanOrEqual(atLeast);
}
