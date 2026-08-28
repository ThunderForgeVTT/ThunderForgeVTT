import { expect, type Page } from "@playwright/test";
import type { WorldProbe } from "../../src/engine/world/probe";
import { uniqueSuffix } from "./helpers";

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

/**
 * The machinery for testing a client that has lost the server (spec 028 US7).
 *
 * Extracted from `world-cache-offline.spec.ts` when the peer-adjudication
 * suite (`world-cache-isolated.spec.ts`) needed the same severing and the
 * same drag. Two copies of `dragToken` would have been two sets of aim
 * offsets drifting apart, and the drag is the single piece of this suite that
 * has already been got wrong twice.
 *
 * # Only the link to the server is severed
 *
 * `context.setOffline(true)` breaks Vite's dynamic module fetches and crashes
 * the page with an unrelated error before the drop is ever observed — the same
 * reason `live-sync.spec.ts` and `world-event-catchup.spec.ts` route the
 * socket instead. Severing only `/api/graphql` heartbeats leaves HTTP working,
 * which is what lets the reconcile mutation run on reconnect and lets a test
 * act as another client during the outage — and, for the isolated suite, it
 * leaves the peer data channels untouched, which is the entire state under
 * test there.
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

export async function register(page: Page, prefix: string): Promise<string> {
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

export async function createWorldAndPlay(page: Page, name: string): Promise<string> {
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

export async function waitForEngineReady(page: Page): Promise<void> {
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

/**
 * Wait until token traffic goes quiet.
 *
 * `applyTokenWorldEvent` answers any token event by refetching *every*
 * token on the scene and dispatching an upsert for each — so a scene still
 * settling puts a token back where the server has it, on top of a local drag
 * that has not been sent anywhere. Offline, that silently undoes the edit
 * under test: the token ends up exactly where it started, which every
 * convergence assertion afterwards reads as agreement. Two clients joining
 * the same scene generate enough of that traffic to hit it reliably; one
 * client does not, which is why T083 never saw it.
 *
 * The wait is on the traffic itself rather than on `scene-load-indicator`,
 * which is a separate five-resource status that does not reliably reach
 * "ready" here, and would be measuring something adjacent to the problem
 * even if it did.
 */
export async function waitForTokenTrafficToSettle(page: Page): Promise<void> {
  const upserts = () =>
    page.evaluate(
      () =>
        window.__worldProbe
          ?.commands()
          .filter((command) => command.type === "upsert_token").length ?? 0,
    );
  let last = await upserts();
  let quiet = 0;
  for (let i = 0; i < 40 && quiet < 3; i += 1) {
    await page.waitForTimeout(1_000);
    const now = await upserts();
    quiet = now === last ? quiet + 1 : 0;
    last = now;
  }
}

export async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("Bevy canvas element not found");
  return box;
}

/** Create a token through the panel and return its server id. */
export async function createToken(page: Page): Promise<string> {
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
export async function dragCanvas(
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

/**
 * Aim points for a press, nearest first, relative to where the store says
 * the token is.
 *
 * The store's position is not where the token is *drawn*, and the two clients
 * in T084 do not even disagree with it in the same way. `snap_tokens_to_grid`
 * moves every token to its cell centre inside the engine while the store keeps
 * the unsnapped value it was handed — a panel-created token is stored at the
 * world origin and drawn a cell-centre away from it, measured here at 63px on
 * a 128px grid. The player's client has no scene grid at all: `scenes(worldId:)`
 * filters hidden scenes out for non-GMs and a world's auto-created scene is
 * hidden, so that session never receives `set_scene_grid`, does not snap, and
 * hit-tests against the engine's fixed 96px token instead of a grid footprint.
 * One centre-relative aim cannot be right for both, so the press is searched
 * outward from each client's own reading until that client's engine says it
 * grabbed the token.
 *
 * The lattice reaches a full grid cell in every direction, with a step inside
 * the smaller of the two hit areas, so neither client's token can hide between
 * two aim points.
 */
const AIM_OFFSETS: { dx: number; dy: number }[] = (() => {
  const step = 32;
  const reach = 128;
  const points: { dx: number; dy: number }[] = [];
  for (let dx = -reach; dx <= reach; dx += step) {
    for (let dy = -reach; dy <= reach; dy += step) {
      points.push({ dx, dy });
    }
  }
  return points.sort((a, b) => a.dx * a.dx + a.dy * a.dy - (b.dx * b.dx + b.dy * b.dy));
})();

/**
 * Drag a specific token by a screen delta, starting from wherever it
 * actually is.
 *
 * A press on empty canvas is a *deselect*, and the drag then silently does
 * nothing, which is the worst possible failure for this test: every
 * convergence assertion afterwards compares the starting position with itself
 * and passes. So the press is aimed by the probe and **confirmed, while the
 * button is still down, by the selection it produced** — the engine picks its
 * stack on `just_pressed` and reports it, so by the time the button is held
 * the store already knows whether this press grabbed the right token. A press
 * that grabbed nothing (or something else) is released without moving, and the
 * next aim point is tried, so a mis-aimed press can never drag a bystander.
 *
 * World units map 1:1 to pixels at the default zoom, origin at the canvas
 * centre, y pointing up — the same convention token-authoring.spec.ts
 * documents and relies on. What that convention does not say, and what cost
 * this test its drags, is that the engine snaps tokens after the store has
 * recorded them: see `AIM_OFFSETS`.
 */
export async function dragToken(
  page: Page,
  tokenId: string,
  delta: { dx: number; dy: number },
): Promise<void> {
  const box = await canvasBox(page);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;

  // Rounds, because a token can be genuinely ungrabbable for a while: a scene
  // still settling refetches every token and moves it out from under the
  // press. Between rounds the position is read again and the search restarts.
  for (let round = 0; round < 4; round += 1) {
    const at = await tokenPosition(page, tokenId);
    if (!at) throw new Error(`token ${tokenId} is not in this client's world store`);

    for (const offset of AIM_OFFSETS) {
      const from = { x: cx + at.x + offset.dx, y: cy - at.y + offset.dy };
      if (
        from.x < box.x + 8 ||
        from.x > box.x + box.width - 8 ||
        from.y < box.y + 8 ||
        from.y > box.y + box.height - 8
      ) {
        continue;
      }

      await page.mouse.move(from.x, from.y);
      await page.mouse.down();
      // A press and a move coalesced into one frame make the engine compute a
      // zero drag offset, and the token silently does not move. See
      // dragCanvas. The same wait is what lets the selection this press
      // produced reach the store before it is read.
      await page.waitForTimeout(250);

      const grabbed = await page.evaluate(
        () => window.__worldProbe?.state().selectedTokenId ?? null,
      );
      if (grabbed !== tokenId) {
        await page.mouse.up();
        await page.waitForTimeout(80);
        continue;
      }

      await page.mouse.move(from.x + delta.dx, from.y + delta.dy, { steps: 5 });
      await page.waitForTimeout(250);
      await page.mouse.up();
      await page.waitForTimeout(600);

      const moved = await tokenPosition(page, tokenId);
      if (moved && (moved.x !== at.x || moved.y !== at.y)) return;
      // Grabbed and still unmoved: the aim was right, so searching further out
      // would only find the same token again. Re-read and start a new round.
      break;
    }

    await page.waitForTimeout(2_000);
  }

  throw new Error(
    `no press within a grid cell of ${tokenId} grabbed it, or every grab left it ` +
      "where it started — the token is not being drawn where the store says, " +
      "or a scene refetch is landing on top of it",
  );
}

export async function tokenPosition(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  const state = await page.evaluate(() => window.__worldProbe?.state());
  return state?.tokens.find((token) => token.id === tokenId) ?? null;
}

/** The token's position as the *server* has it — the thing that must converge. */
export async function serverTokenPosition(
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

/** The signed-in user's own id, as the session endpoint reports it. */
export async function currentUserId(page: Page): Promise<string> {
  const userId = await page.evaluate(async () => {
    const res = await fetch("/api/authentication/session", {
      credentials: "same-origin",
    });
    const body = (await res.json()) as {
      session?: { user?: { id?: string } } | null;
    };
    return body.session?.user?.id ?? null;
  });
  if (!userId) throw new Error("no signed-in user");
  return userId;
}

/**
 * Hand a token to a player, as the Game Master.
 *
 * Not decoration: the server enforces `owner_user_id = requester` for a
 * non-GM edit, at reconnect exactly as it does live, so a player replaying a
 * move of a token nobody gave them is refused and the test would be
 * measuring authorization rather than precedence.
 */
export async function giveTokenTo(
  page: Page,
  tokenId: string,
  userId: string,
): Promise<void> {
  const ok = await page.evaluate(
    async ({ token, owner }) => {
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
          query: `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
            updateToken(tokenId: $tokenId, input: $input) { tokenId ownerUserId }
          }`,
          variables: { tokenId: token, input: { ownerUserId: owner } },
        }),
      });
      const body = (await res.json()) as {
        data?: { updateToken?: { ownerUserId?: string | null } };
      };
      return body.data?.updateToken?.ownerUserId === owner;
    },
    { token: tokenId, owner: userId },
  );
  if (!ok) throw new Error("could not give the token to the player");
}

export async function firstSceneId(page: Page, worldId: string): Promise<string> {
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
 * Cut the client off from the server, and let it back.
 *
 * The **heartbeat** is what gets blocked, not the WebSocket, because the
 * heartbeat is what the client's sense of connectivity is actually built on
 * (`heartbeat.ts`). Blocking the socket was tried first and is the wrong
 * lever twice over: `graphql-ws` is lazy and may hold no socket at all at the
 * moment of the cut, so there is nothing to sever and nothing notices; and
 * even when it does, socket liveness answers "is anything subscribed" rather
 * than "can this client reach the server".
 *
 * Aborting the request rather than answering an error keeps this honest about
 * what it simulates: a network that is not carrying anything, not a server
 * that is refusing.
 */
export function severableLink(page: Page) {
  let severed = false;
  let blocked = 0;
  let delivered = 0;
  let reconciles = 0;
  return {
    async install() {
      await page.route("**/api/graphql", async (route) => {
        const body = route.request().postData() ?? "";
        const isHeartbeat = body.includes("Heartbeat");
        // Counted here rather than inferred from a side effect: "the queue
        // was never submitted" and "the queue was submitted and refused"
        // look identical from the token's position, and only the first is a
        // client bug. Distinguishing them is what turned T083 around.
        if (body.includes("ReconcileQueuedChanges")) reconciles += 1;
        if (severed && isHeartbeat) {
          blocked += 1;
          await route.abort("internetdisconnected");
          return;
        }
        if (isHeartbeat) delivered += 1;
        await route.fallback();
      });
    },
    cut() {
      severed = true;
      blocked = 0;
    },
    restore() {
      severed = false;
      delivered = 0;
    },
    /** Beats refused since the cut — the client's own offline threshold. */
    blockedBeats: () => blocked,
    /** Beats that got through since the link was restored. */
    deliveredBeats: () => delivered,
    /** `reconcileQueuedChanges` submissions this page has made. */
    reconcileRequests: () => reconciles,
  };
}

/**
 * Wait until the client has decided it cannot reach the server.
 *
 * Measured by the beats the route actually refused, not by a fixed sleep: the
 * client's verdict is three consecutive failures, so counting refusals is the
 * same quantity it is counting, and the test cannot pass by waiting long
 * enough on a client that never tried.
 */
export async function waitForOffline(
  page: Page,
  link: { blockedBeats: () => number },
): Promise<void> {
  await expect
    .poll(() => link.blockedBeats(), {
      timeout: 90_000,
      message: "the client should keep beating, and those beats should be refused",
    })
    .toBeGreaterThanOrEqual(4);
  // One further beat interval, so the failure the threshold turns on has been
  // observed by the client and not merely by the route.
  await page.waitForTimeout(6_000);
}

/** Wait until a heartbeat gets through again. */
export async function waitForOnline(
  page: Page,
  link: { deliveredBeats: () => number },
): Promise<void> {
  await expect
    .poll(() => link.deliveredBeats(), {
      timeout: 90_000,
      message: "a restored link should carry a heartbeat again",
    })
    .toBeGreaterThanOrEqual(1);
}
