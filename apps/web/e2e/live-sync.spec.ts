import { test, expect, type Page } from "@playwright/test";

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
async function registerAndCreateWorldOnDashboard(
  page: Page,
  worldName: string,
): Promise<void> {
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
  // The waits inside this test declare 45 seconds of patience in two places.
  // On Playwright's 30-second default they could never be spent: the test died
  // first, and the failure was reported against whichever wait happened to be
  // pending rather than against what was slow. The budget now exceeds what the
  // test says it is willing to wait for.
  test.setTimeout(120_000);

  test("a dropped connection shows a persistent reconnecting indicator, then auto-recovers and re-fetches on restore", async ({
    page,
  }) => {
    // Record the page's own /api/ws sockets, rather than proxying them.
    //
    // This used to `page.routeWebSocket` the connection and sever the proxy,
    // and that shape failed three ways at once. It kept a single "active"
    // handle, which is the *last* socket routed — and a world page holds two:
    // the world-event client in `engine/world/sync/subscriptionClient.ts`,
    // whose state this indicator reports, and a private one that
    // `engine/bevy/index.ts` opens for peer-transfer signalling once the
    // engine has started. It also forwarded reconnects straight through, so
    // `graphql-ws` was back in about a second and the banner flashed too
    // briefly to assert on. Closing alone is not an outage.
    //
    // Measured rather than assumed: a probe reading `getLiveSyncState()` on
    // each sample showed live -> reconnecting within 500ms of the close and
    // back to live by 1500ms. Roughly a one-second window to catch.
    //
    // So this now does what `world-event-catchup.spec.ts` does, for the same
    // reasons: record the real sockets, close every open one, and hold the
    // endpoint blocked over CDP so the outage lasts long enough to be
    // observed. Scoped to `/api/ws` so Vite's own module fetches keep working
    // — a full `context.setOffline(true)` breaks the lazy route chunks this
    // app loads, which is why these specs never used it.
    await page.addInitScript(() => {
      const Native = window.WebSocket;
      const sockets: WebSocket[] = [];
      (window as unknown as { __e2eSockets: WebSocket[] }).__e2eSockets =
        sockets;
      class RecordingWebSocket extends Native {
        constructor(url: string | URL, protocols?: string | string[]) {
          super(url, protocols);
          if (String(url).includes("/api/ws")) {
            sockets.push(this);
          }
        }
      }
      window.WebSocket = RecordingWebSocket as unknown as typeof WebSocket;
    });

    let sceneWallsRequests = 0;
    page.on("request", (request) => {
      if (
        request.url().includes("/graphql") &&
        request.method() === "POST" &&
        (request.postData()?.includes("SceneWalls") ?? false)
      ) {
        sceneWallsRequests += 1;
      }
    });

    await registerAndCreateWorldOnDashboard(
      page,
      `E2E Live Sync ${uniqueSuffix()}`,
    );
    await enterWorldPlay(page);

    // Every world gets exactly one scene at creation
    // (graphql::tests::create_world_always_yields_exactly_one_scene), so
    // WorldPage's server-authoritative activeSceneId selection should
    // pick it up without a manual scene-create step.
    const indicator = page.getByTestId("live-sync-reconnecting-indicator");

    // Initial connect: the indicator (shown for "connecting"/"reconnecting")
    // must clear once the subscription reaches "live".
    await expect(indicator).toBeHidden({ timeout: 20_000 });

    // Polled, not asserted once. The socket is opened by whichever panel
    // first subscribes, which is not synchronous with the indicator clearing:
    // this used to be reached only after `toBeHidden` had spent many seconds
    // waiting out a banner that was stuck at "connecting" regardless of the
    // real connection, and that delay was doing the waiting this check needs.
    // With the indicator reporting the truth promptly, the socket is
    // legitimately not open yet at this instant.
    const openSocketCount = () =>
      page.evaluate(() => {
        const sockets = (window as unknown as { __e2eSockets: WebSocket[] })
          .__e2eSockets;
        return sockets.filter((socket) => socket.readyState === WebSocket.OPEN)
          .length;
      });

    await expect
      .poll(openSocketCount, {
        timeout: 20_000,
        message: "expected the page to have an active /api/ws connection",
      })
      .toBeGreaterThan(0);

    // The scene has to be loaded before it can be re-loaded.
    await expect
      .poll(() => sceneWallsRequests, {
        timeout: 30_000,
        message:
          "the scene should have loaded once before the connection is severed",
      })
      .toBeGreaterThanOrEqual(1);

    // Block first, then close — otherwise the client is back before the
    // block lands.
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.setBlockedURLs", { urls: ["*/api/ws*"] });

    // 4499 is graphql-ws's "Terminated", deliberately excluded from its fatal
    // close codes — the client retries this one rather than treating it as a
    // clean, intentional shutdown that needs no reconnection.
    const severed = await page.evaluate(() => {
      const sockets = (window as unknown as { __e2eSockets: WebSocket[] })
        .__e2eSockets;
      const open = sockets.filter(
        (socket) => socket.readyState === WebSocket.OPEN,
      );
      for (const socket of open) {
        socket.close(4499, "e2e sever");
      }
      return open.length;
    });
    expect(severed, "expected open sockets to sever").toBeGreaterThan(0);

    // FR-009/FR-009a: a persistent, non-dead-end "reconnecting" indicator
    // — not silent stale data, not a state requiring manual action.
    await expect(indicator).toBeVisible({ timeout: 15_000 });
    await expect(indicator).toContainText("Reconnecting", { timeout: 15_000 });

    // Let it back in. `graphql-ws` is already retrying with backoff, so the
    // next attempt after this succeeds and the client resumes on its own.
    await cdp.send("Network.setBlockedURLs", { urls: [] });

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
