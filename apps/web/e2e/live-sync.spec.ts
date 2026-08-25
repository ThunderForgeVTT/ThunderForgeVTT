import { test, expect, type Page, type WebSocketRoute } from "@playwright/test";

/**
 * specs/005-live-canvas-sync, User Story 2 (T012/T013): the
 * reconnect-triggered full resync this project's own tasks.md tracked as
 * a real, unbuilt gap (see `docs/adrs/20260825-048-...md`'s "What did NOT
 * ship" section) — now implemented in
 * `apps/web/src/engine/world/sync/subscriptionClient.ts` (`LiveSyncState`,
 * `retryAttempts: Infinity` + exponential-backoff `retryWait`) and wired
 * into `apps/web/src/pages/world/WorldPage.tsx` (the
 * `live-sync-reconnecting-indicator` testid, and a full re-fetch of
 * walls/lights/shapes/tokens on every transition back into `live`).
 *
 * Deliberately does NOT use `waitForEngineReady`/`canvasBox`/
 * `clickCanvasAt` from `map-editor-tooling.spec.ts`/`canvas-authoring.spec.ts`
 * — this feature's own connection-state machine and its UI indicator
 * render independently of the Bevy/WASM canvas actually mounting, so this
 * test does not depend on (and is not blocked by) the sandbox's
 * documented "headless Chromium can't render the canvas" limitation that
 * blocks every canvas-interaction e2e test in this project today.
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
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
}

/** Same helper as `map-editor-tooling.spec.ts` (duplicated rather than
 * shared, per this project's established e2e convention — see that
 * file's own top-of-file doc comment). */
async function registerAndCreateWorldOnDashboard(page: Page, worldName: string): Promise<void> {
  await register(page, freshCredentials("e2elivesync"));

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
}

async function enterWorldPlay(page: Page): Promise<void> {
  await expect(page).toHaveURL(/\/world\/[^/]+\/staging$/);
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

test.describe("Live sync reconnect (US2, T012/T013)", () => {
  test("a dropped connection shows a persistent reconnecting indicator, then auto-recovers and re-fetches on restore", async ({
    page,
  }) => {
    // Route only the /api/ws WebSocket, transparently forwarding to the
    // real server, so this test can sever *just* the subscription
    // transport's connection on demand — a full `context.setOffline(true)`
    // also breaks Vite dev-server's own dynamic module fetches (this app
    // lazy-loads route chunks), crashing the page with an unrelated
    // "Failed to fetch dynamically imported module" error before the
    // WebSocket drop is even observed.
    let activeClientWs: WebSocketRoute | null = null;
    await page.routeWebSocket(/\/api\/ws/, (ws) => {
      const server = ws.connectToServer();
      ws.onMessage((message) => server.send(message));
      server.onMessage((message) => ws.send(message));
      activeClientWs = ws;
      ws.onClose(() => {
        if (activeClientWs === ws) {
          activeClientWs = null;
        }
      });
    });

    await registerAndCreateWorldOnDashboard(page, `E2E Live Sync ${uniqueSuffix()}`);
    await enterWorldPlay(page);

    // Every world gets exactly one scene at creation
    // (graphql::tests::create_world_always_yields_exactly_one_scene), so
    // WorldPage's server-authoritative activeSceneId selection should
    // pick it up without a manual scene-create step.
    const indicator = page.getByTestId("live-sync-reconnecting-indicator");

    // Initial connect: the indicator (shown for "connecting"/"reconnecting")
    // must clear once the subscription reaches "live".
    await expect(indicator).toBeHidden({ timeout: 20_000 });

    // Simulate a dropped connection (FR-008/FR-008a) by closing just the
    // WebSocket, from the routing layer — everything else (HTTP, module
    // loading) keeps working normally.
    expect(activeClientWs, "expected the page to have an active /api/ws connection").not.toBeNull();
    activeClientWs?.close();

    // FR-009/FR-009a: a persistent, non-dead-end "reconnecting" indicator
    // — not silent stale data, not a state requiring manual action. The
    // route handler above passes the client's next reconnect attempt
    // straight through to the real server, so no further test-side action
    // is needed to let it recover.
    await expect(indicator).toBeVisible({ timeout: 15_000 });
    await expect(indicator).toContainText("Reconnecting", { timeout: 15_000 });

    // On the transition back to `live`, WorldPage.tsx must trigger a full
    // scene re-fetch (T016) — observed here as a fresh SceneWalls query
    // hitting /graphql after reconnect, not just the indicator clearing.
    const resyncRequest = page.waitForRequest(
      (req) =>
        req.url().includes("/graphql") &&
        req.method() === "POST" &&
        (req.postData()?.includes("SceneWalls") ?? false),
      { timeout: 45_000 },
    );

    await expect(indicator).toBeHidden({ timeout: 45_000 });
    await resyncRequest;
  });
});
