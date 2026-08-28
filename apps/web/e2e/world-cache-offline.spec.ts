import { expect, test, type Page } from "@playwright/test";
import type { WorldProbe } from "../src/engine/world/probe";
import { inviteAndJoinAsPlayer, uniqueSuffix } from "./fixtures/helpers";

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
async function waitForTokenTrafficToSettle(page: Page): Promise<void> {
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
async function dragToken(
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

/** The signed-in user's own id, as the session endpoint reports it. */
async function currentUserId(page: Page): Promise<string> {
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
async function giveTokenTo(page: Page, tokenId: string, userId: string): Promise<void> {
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
function severableLink(page: Page) {
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
async function waitForOffline(
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
async function waitForOnline(
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

test.describe("Client world cache — playing on through a lost connection (US7)", () => {
  test.setTimeout(420_000);

  /**
   * The bug this test found, for anyone reading the history: the token
   * mutation bridge seeded its engine-id map once, from the scene's tokens
   * at start, and only ever extended it from its own `createToken` calls. A
   * token created afterwards through the token panel — which calls the
   * mutation directly — was therefore never in the map, so every later drag
   * of it read as a *first sighting*, and FR-035a's "creations are not
   * queued offline" branch swallowed an ordinary move. Nothing was queued,
   * so there was nothing to replay and no `ReconcileQueuedChanges` request
   * was ever made. The bridge now learns from `sync` dispatches, which is
   * how the client hears about a token in the first place.
   *
   * Worth noting what nearly hid it: the "the server must not have it yet"
   * assertion below passed throughout, and was read as proof that queueing
   * worked. It is not — a silently dropped edit leaves the server unchanged
   * too. The assertion separates "queued" from "sent anyway", and says
   * nothing about "dropped".
   */
  test("a change made offline is applied on reconnect and reported (SC-015, T083)", async ({
    page,
  }) => {
    const link = severableLink(page);
    await link.install();

    await register(page, "e2eoff");
    const worldId = await createWorldAndPlay(page, `E2E Offline ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    const before = await serverTokenPosition(page, sceneId, tokenId);
    expect(before, "the token should exist server-side before we go offline").toBeTruthy();

    link.cut();
    await waitForOffline(page, link);

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

    link.restore();
    await waitForOnline(page, link);

    // SC-015: applied on reconnect, and reported.
    await expect
      .poll(() => serverTokenPosition(page, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "the queued edit should reach the server on reconnect",
      })
      .not.toBe(before!.x);

    await expect(page.getByTestId("reconcile-report")).toBeVisible({ timeout: 30_000 });
  });


  /**
   * SC-016: two people edit the same token with no connection, and the
   * **player reconnects first**.
   *
   * The ordering is the entire test. A player who reconnects first has their
   * change applied — legitimately, since at that moment nothing contradicts
   * it — and only later does the Game Master come back with a conflicting
   * edit that outranks it. So this is the `Applied → Superseded` path
   * (FR-041), the one case where a change is reported as synced and then
   * stops being true.
   *
   * There is no response left to carry that news: the player is long gone
   * from their own reconcile call by the time the GM makes theirs. It can
   * only reach them as an ordinary world event, which is why the server's
   * token event grows `reconciled`/`by_user`/`by_role` (T079). Asserting
   * only that the two clients converge would pass while the player is never
   * told their work was overridden — and being told is the requirement.
   */
  test("a Game Master reconnecting after a player overrides them, and the player is told (SC-016, T084)", async ({
    browser,
  }) => {
    // The invite flow writes to the clipboard, and a context without that
    // permission throws before the new invite is stored — see
    // scene-live-launch.spec.ts, which hit the same thing.
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();

    await register(gmPage, "e2eogm");
    const worldId = await createWorldAndPlay(gmPage, `E2E Offline Two ${uniqueSuffix()}`);
    await waitForEngineReady(gmPage);
    const sceneId = await firstSceneId(gmPage, worldId);
    const tokenId = await createToken(gmPage);

    const playerPage = await inviteAndJoinAsPlayer(browser, gmPage, worldId, "e2eopl");
    await giveTokenTo(gmPage, tokenId, await currentUserId(playerPage));

    // Both at the table, on the same scene, before either goes offline.
    await gmPage.goto(`/world/${worldId}/play`);
    await waitForEngineReady(gmPage);
    await playerPage.goto(`/world/${worldId}/play`);
    await waitForEngineReady(playerPage);
    await waitForTokenTrafficToSettle(gmPage);
    await waitForTokenTrafficToSettle(playerPage);

    const before = await serverTokenPosition(gmPage, sceneId, tokenId);
    expect(before, "the token should exist server-side before anyone drops").toBeTruthy();

    const gmLink = severableLink(gmPage);
    const playerLink = severableLink(playerPage);
    await gmLink.install();
    await playerLink.install();

    gmLink.cut();
    playerLink.cut();
    await waitForOffline(gmPage, gmLink);
    await waitForOffline(playerPage, playerLink);

    // The same token, two different destinations, neither reaching the
    // server. Only the heartbeat is severed, so each still holds a live
    // subscription — which is what lets the player hear about the GM's
    // replay later, and is also how a real outage of this shape behaves.
    await dragToken(gmPage, tokenId, { dx: 200, dy: -140 });
    await dragToken(playerPage, tokenId, { dx: -170, dy: 110 });

    // Read the GM's position *now*. Once the player reconnects, the event
    // from their applied change reaches this still-subscribed page and moves
    // the token locally — so a reading taken later is the player's position,
    // not the GM's, and the final assertion would be comparing the server
    // against itself.
    const gmIntent = await tokenPosition(gmPage, tokenId);
    const playerIntent = await tokenPosition(playerPage, tokenId);
    // Not merely "a token is there": each drag must actually have moved it.
    // A drag that silently did nothing leaves the local and server positions
    // equal, which makes every convergence assertion below pass by
    // comparing the starting position with itself.
    expect(gmIntent, "the GM's drag must move their own view").toBeTruthy();
    expect(playerIntent, "the player's drag must move their own view").toBeTruthy();
    expect(gmIntent!.x, "the GM's drag must actually move the token").not.toBe(before!.x);
    expect(playerIntent!.x, "the player's drag must actually move the token").not.toBe(
      before!.x,
    );
    expect(
      (await serverTokenPosition(gmPage, sceneId, tokenId))!.x,
      "neither edit may be written through while offline",
    ).toBe(before!.x);

    // The player comes back first, and wins for as long as nothing outranks
    // them.
    playerLink.restore();
    await waitForOnline(playerPage, playerLink);
    await expect
      .poll(() => serverTokenPosition(playerPage, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "the player's queued edit should be applied on their reconnect",
      })
      .toBeCloseTo(playerIntent!.x, 0);
    expect(
      playerLink.reconcileRequests(),
      "the player should have submitted their queue on reconnect",
    ).toBeGreaterThanOrEqual(1);
    await expect(playerPage.getByTestId("reconcile-report")).toBeVisible({
      timeout: 30_000,
    });

    // Then the Game Master, whose edit outranks it (FR-040).
    gmLink.restore();
    await waitForOnline(gmPage, gmLink);
    await expect
      .poll(() => serverTokenPosition(gmPage, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "a Game Master reconnecting later still wins",
      })
      .toBeCloseTo(gmIntent!.x, 0);

    // Convergence, which is the easy half.
    await expect
      .poll(() => tokenPosition(playerPage, tokenId).then((p) => p?.x), {
        timeout: 60_000,
        message: "the player's own view should follow the server",
      })
      .toBeCloseTo(gmIntent!.x, 0);

    // And the half that matters: the player is told, rather than left
    // believing an edit stands that does not.
    await expect(
      playerPage.getByTestId("reconcile-superseded"),
    ).toBeVisible({ timeout: 60_000 });
    await expect(playerPage.getByTestId("reconcile-superseded")).toContainText(
      "the Game Master",
    );

    await playerPage.context().close();
    await gmContext.close();
  });

  test("a queued change against deleted content is discarded with an explanation (T085)", async ({
    page,
  }) => {
    const link = severableLink(page);
    await link.install();

    await register(page, "e2eoffdel");
    const worldId = await createWorldAndPlay(page, `E2E Offline Gone ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    link.cut();
    await waitForOffline(page, link);
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

    link.restore();
    await waitForOnline(page, link);

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
