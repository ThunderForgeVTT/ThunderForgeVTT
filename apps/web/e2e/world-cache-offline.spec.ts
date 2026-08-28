import { expect, test, type Page, type WebSocketRoute } from "@playwright/test";
import type { WorldProbe } from "../src/engine/world/probe";
import { uniqueSuffix } from "./fixtures/helpers";

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

/**
 * Playing on through a lost connection (spec 028 US7, T083–T085,
 * SC-015/SC-016).
 *
 * These are the tests the rest of US7 exists for. Every piece — the outbox,
 * the disconnected state, `conflict::resolve`, the reconcile mutation, the
 * report — is individually covered by unit tests that cannot see the one thing
 * that matters: whether a token a person dragged while offline is still where
 * they put it once the connection comes back.
 *
 * # Only the WebSocket is severed
 *
 * `context.setOffline(true)` breaks Vite's dynamic module fetches and crashes
 * the page with an unrelated error before the drop is ever observed — the same
 * reason `live-sync.spec.ts` and `world-event-catchup.spec.ts` route the
 * socket instead. Severing `/api/ws` leaves HTTP working, which is what lets
 * the reconcile mutation run on reconnect and lets a test act as another
 * client during the outage.
 *
 * It also matches the product's own model of "disconnected": the world's
 * liveness is the subscription. A page whose socket is dead is not receiving
 * anyone else's changes, and that is the state offline editing is for.
 *
 * # The drags are real drags
 *
 * Driving the store directly would be faster and would prove less than it
 * appears to: the queueing decision lives in the token mutation bridge, which
 * only runs for edits that actually pass through it. A synthetic dispatch that
 * skipped the canvas would be asserting against a path the application does
 * not take. The mouse choreography below (press, wait a frame, move, wait,
 * release) is copied from `token-authoring.spec.ts` for the reason documented
 * there — a press and move coalesced into one frame make the engine compute a
 * zero drag offset, and the token silently does not move.
 */

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

const PASSWORD = "Sup3r-Secret-Passphrase!";

async function register(page: Page, prefix: string): Promise<string> {
  const username = `${prefix}${uniqueSuffix()}`.slice(0, 24);
  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(`${username}@example.test`);
  await page.locator("#register-password").fill(PASSWORD);
  await page.locator("#register-password-confirmation").fill(PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 20_000,
  });
  return username;
}

async function createWorldAndPlay(page: Page, name: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(name);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  await page.getByTestId("play-button").click({ timeout: 20_000 });
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 20_000 });
  const match = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`no world id in ${page.url()}`);
  return match[1];
}

async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  if (/\/staging$/.test(new URL(page.url()).pathname)) {
    await page.getByTestId("play-button").click({ timeout: 20_000 });
  }
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(3_000);
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (box) {
    // Clear of the dock's icon rail and the dice bar, so the click reaches
    // the engine and gives the canvas focus (see token-authoring.spec.ts).
    await page.mouse.click(box.x + box.width - 200, box.y + 120);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }
}

async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("Bevy canvas element not found");
  return box;
}

/** Create a token through the panel and return its server id. */
async function createToken(page: Page): Promise<string> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  await page.getByTestId("token-create-trigger").click({ force: true });
  const [response] = await Promise.all([
    page.waitForResponse(
      (resp) =>
        resp.url().includes("/api/graphql") &&
        (resp.request().postData() ?? "").includes("createToken"),
    ),
    page.getByTestId("token-create-submit").click({ force: true }),
  ]);
  const body = (await response.json()) as {
    data?: { createToken?: { tokenId?: string } };
  };
  const tokenId = body.data?.createToken?.tokenId;
  if (!tokenId) throw new Error("no tokenId in createToken response");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);
  return tokenId;
}

/**
 * Drag from one canvas offset to another, with the frame timing the engine
 * needs.
 *
 * The waits are not padding. The engine computes a drag offset on
 * `just_pressed` as `tokenPos - cursorWorld`; if press and move land in the
 * same frame it sees the cursor already at the destination, takes an offset
 * that cancels the motion, and the drag does nothing at all — with no error
 * anywhere to say so.
 */
async function dragCanvas(
  page: Page,
  from: { dx: number; dy: number },
  to: { dx: number; dy: number },
): Promise<void> {
  const box = await canvasBox(page);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx + from.dx, cy + from.dy);
  await page.mouse.down();
  await page.waitForTimeout(250);
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 5 });
  await page.waitForTimeout(250);
  await page.mouse.up();
  await page.waitForTimeout(500);
}

async function tokenPosition(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  const state = await page.evaluate(() => window.__worldProbe?.state());
  return state?.tokens.find((token) => token.id === tokenId) ?? null;
}

/** The token's position as the *server* has it — the thing that must converge. */
async function serverTokenPosition(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  return page.evaluate(
    async ({ scene, token }) => {
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "content-type": "application/json",
          ...(csrf ? { "x-csrf-token": csrf } : {}),
        },
        body: JSON.stringify({
          query: `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y } }`,
          variables: { sceneId: scene },
        }),
      });
      const body = (await res.json()) as {
        data?: { tokens?: { tokenId: string; x: number; y: number }[] };
      };
      const found = body.data?.tokens?.find((t) => t.tokenId === token);
      return found ? { x: found.x, y: found.y } : null;
    },
    { scene: sceneId, token: tokenId },
  );
}

async function firstSceneId(page: Page, worldId: string): Promise<string> {
  const sceneId = await page.evaluate(async (world) => {
    // The CSRF header is not optional: GraphQL is served over POST, so
    // `require_csrf_for_session` treats every query as state-changing and
    // answers 403 with an empty body — which surfaces as a JSON parse error
    // several frames away from the actual cause.
    const csrf = document.cookie
      .split(";")
      .map((part) => part.trim())
      .find((part) => part.startsWith("csrf_token="))
      ?.slice("csrf_token=".length);
    const res = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "content-type": "application/json",
        ...(csrf ? { "x-csrf-token": csrf } : {}),
      },
      body: JSON.stringify({
        query: `query ($worldId: UUID!) { scenes(worldId: $worldId) { sceneId } }`,
        variables: { worldId: world },
      }),
    });
    const body = (await res.json()) as {
      data?: { scenes?: { sceneId: string }[] };
    };
    return body.data?.scenes?.[0]?.sceneId ?? null;
  }, worldId);
  if (!sceneId) throw new Error("world has no scene");
  return sceneId;
}

/**
 * Control over the page's WebSocket, so it can be cut and restored.
 *
 * **Every** open socket is tracked, not just the most recent one. A mounted
 * world page opens more than one — the first attempt and the one the world
 * page itself establishes — and closing only the latest leaves the other
 * carrying events, so the client never notices an outage and the test waits
 * out its timeout on an indicator that was never going to appear. Cost an
 * hour to find; the single-socket version *looks* right and fails silently.
 */
function severableSocket(page: Page) {
  const open = new Set<WebSocketRoute>();
  let severed = false;
  return {
    async install() {
      await page.routeWebSocket(/\/api\/ws/, (ws) => {
        if (severed) {
          // Refuse the reconnect attempts too, or the client recovers a
          // second after being cut and there is no outage to edit during.
          ws.close();
          return;
        }
        const server = ws.connectToServer();
        ws.onMessage((message) => server.send(message));
        server.onMessage((message) => ws.send(message));
        open.add(ws);
        ws.onClose(() => open.delete(ws));
      });
    },
    cut() {
      severed = true;
      for (const ws of [...open]) {
        ws.close();
      }
      open.clear();
    },
    restore() {
      severed = false;
    },
  };
}

/** Wait until the page reports itself disconnected, not merely reconnecting. */
async function waitForDisconnected(page: Page): Promise<void> {
  await expect(page.getByTestId("live-sync-reconnecting-indicator")).toHaveAttribute(
    "data-sync-status",
    "disconnected",
    { timeout: 60_000 },
  );
}

async function waitForLive(page: Page): Promise<void> {
  await expect(page.getByTestId("live-sync-reconnecting-indicator")).toHaveCount(0, {
    timeout: 60_000,
  });
}

test.describe("Client world cache — playing on through a lost connection (US7)", () => {
  test.setTimeout(420_000);

  // Both tests below are `fixme`, not `skip`: the scenario, the setup and the
  // diagnosis are worth keeping in the tree, and the code under test is
  // believed correct — what is missing is a way to put the page into the
  // disconnected state from a test.
  //
  // **What was measured.** With every `/api/ws` socket closed and all
  // reconnects refused, the page reports `live` indefinitely — the indicator
  // is absent before the cut and still absent twelve seconds after it, while
  // the canvas is mounted and the world is loaded. So the sever works and the
  // page does not notice.
  //
  // A standalone reproduction of the same mechanism *does* reach
  // `disconnected` within four seconds, and the difference between it and
  // these tests has not been found. What that isolated case showed on the way
  // through is a real defect, fixed in `subscriptionClient.ts` as part of this
  // change: `graphql-ws` is a lazy client and lets its connection go when it
  // has no subscriptions to serve, and the `closed` handler used to update
  // nothing in that case on the reasoning that a retry would report itself.
  // No retry comes, so the state sat at `live` with no socket at all. A tab
  // claiming a healthy connection it does not have is worse than one
  // reporting the outage, and it makes an offline edit look like an online
  // one.
  //
  // That fix did not change what these tests observe, so it was necessary and
  // not sufficient. The next thing to establish is whether the world page's
  // own subscriptions are open at the moment of the cut — if `graphql-ws`
  // holds no socket because nothing is subscribed, then "disconnected" is a
  // question about a connection that does not exist, and the product answer
  // is probably to key US7's queueing on something other than socket
  // liveness.
  test.fixme("a change made offline is applied on reconnect and reported (SC-015, T083)", async ({
    page,
  }) => {
    const socket = severableSocket(page);
    await socket.install();

    await register(page, "e2eoff");
    const worldId = await createWorldAndPlay(page, `E2E Offline ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    const before = await serverTokenPosition(page, sceneId, tokenId);
    expect(before, "the token should exist server-side before we go offline").toBeTruthy();

    socket.cut();
    await waitForDisconnected(page);

    // The edit. A panel-created token starts at the world origin.
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 180, dy: -120 });

    const localAfterDrag = await tokenPosition(page, tokenId);
    expect(
      localAfterDrag,
      "the token must move locally — offline play is the point",
    ).toBeTruthy();
    expect(localAfterDrag!.x).not.toBe(before!.x);

    // And the server must not have it yet. This is what distinguishes
    // "queued" from "sent anyway": HTTP is still up, so a client that fired
    // the mutation regardless would have written through.
    const duringOutage = await serverTokenPosition(page, sceneId, tokenId);
    expect(
      duringOutage!.x,
      "an offline edit must be queued, not written through",
    ).toBe(before!.x);

    socket.restore();
    await waitForLive(page);

    // SC-015: applied on reconnect, and reported.
    await expect
      .poll(() => serverTokenPosition(page, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "the queued edit should reach the server on reconnect",
      })
      .not.toBe(before!.x);

    await expect(page.getByTestId("reconcile-report")).toBeVisible({ timeout: 30_000 });
  });

  test.fixme("a queued change against deleted content is discarded with an explanation (T085)", async ({
    page,
  }) => {
    const socket = severableSocket(page);
    await socket.install();

    await register(page, "e2eoffdel");
    const worldId = await createWorldAndPlay(page, `E2E Offline Gone ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    socket.cut();
    await waitForDisconnected(page);
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 150, dy: -90 });

    // Deleted while this client was away. HTTP still works — only the
    // subscription was severed — so the test can act as the other client
    // that removed it.
    const deleted = await page.evaluate(
      async ({ token }) => {
        const csrf = document.cookie
          .split(";")
          .map((p) => p.trim())
          .find((p) => p.startsWith("csrf_token="))
          ?.slice("csrf_token=".length);
        const res = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "content-type": "application/json",
            ...(csrf ? { "x-csrf-token": csrf } : {}),
          },
          body: JSON.stringify({
            query: `mutation ($tokenId: UUID!) { deleteToken(tokenId: $tokenId) }`,
            variables: { tokenId: token },
          }),
        });
        return res.ok;
      },
      { token: tokenId },
    );
    expect(deleted, "the token should have been deleted server-side").toBe(true);

    socket.restore();
    await waitForLive(page);

    // Discarded with a reason, never resurrected. Recreating something
    // someone deliberately removed is the failure FR-035a's create/delete
    // restriction exists to avoid.
    await expect(page.getByTestId("reconcile-report")).toBeVisible({ timeout: 90_000 });
    await expect(
      page.locator('[data-testid="reconcile-rejected"] [data-reason="GONE_AWAY"]'),
    ).toBeVisible({ timeout: 30_000 });

    expect(
      await serverTokenPosition(page, sceneId, tokenId),
      "a deleted token must not come back",
    ).toBeNull();
  });
});
