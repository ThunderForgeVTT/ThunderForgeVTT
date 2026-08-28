import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import {
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  assetFingerprint,
  createCanvasAsset,
  createScene,
  holdsFingerprint,
  openWorldAndSync,
  peerCounters,
  sceneIds,
  setSceneHidden,
  watchCacheSync,
} from "./fixtures/world-cache";

/**
 * Peer-assisted content distribution, with two real clients (spec 028
 * Phase 10, T093–T095, SC-012/SC-013/SC-014).
 *
 * Everything here needs two browser contexts and a live WebRTC data channel
 * between them, because the properties under test are precisely the ones a
 * single client cannot exhibit. The channel is real: host ICE candidates on
 * loopback, no STUN, which is what the product itself uses.
 *
 * # What makes these tests worth their runtime
 *
 * The protocol's whole safety argument is that a peer is asked for a *hash*,
 * never for a thing — so a hostile peer can waste bandwidth and nothing
 * else. Unit tests prove each half of that against a simulated peer.
 * Only these prove it against a real one: real signaling, real SDP, real
 * chunked transfer over SCTP, and a real fall-back to the server underneath.
 */

/**
 * Make every `CHUNK` this context sends carry the wrong bytes.
 *
 * Installed as an init script in the *serving* peer's context, wrapping
 * `RTCDataChannel.prototype.send`. That is deliberately outside the
 * application: nothing in the shipped code has a "serve corrupt content"
 * switch, and adding one to make a test possible would put the failure mode
 * being tested into the product. The wire format is public — a tag byte,
 * then the 32-byte fingerprint, then the variant's tail — so a test can lie
 * on the channel exactly as a modified client would.
 *
 * Only the payload is touched. The frame stays perfectly well-formed and
 * correctly sized, so the requester cannot reject it on shape: the only
 * thing wrong with it is that the bytes do not hash to what was asked for,
 * which is the single property FR-046 turns on.
 */
async function serveCorruptedBytes(context: BrowserContext): Promise<void> {
  await context.addInitScript(() => {
    const TAG_CHUNK = 3;
    const HEADER = 1 + 32;
    const SEQ = 4;
    const send = RTCDataChannel.prototype.send;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (RTCDataChannel.prototype as any).send = function (data: unknown) {
      let frame: Uint8Array | null = null;
      if (data instanceof ArrayBuffer) frame = new Uint8Array(data);
      else if (ArrayBuffer.isView(data)) {
        frame = new Uint8Array(
          data.buffer,
          data.byteOffset,
          data.byteLength,
        );
      }
      if (frame && frame.length > HEADER + SEQ && frame[0] === TAG_CHUNK) {
        const copy = frame.slice();
        // One flipped byte in the payload is enough, and is the hardest
        // version of the test: everything else about the transfer is right.
        copy[HEADER + SEQ] = copy[HEADER + SEQ] ^ 0xff;
        return send.call(this, copy.buffer as ArrayBuffer);
      }
      return send.call(this, data as ArrayBuffer);
    };
  });
}

/** Count the peer-signaling calls a context makes. */
function countPeerSignaling(page: Page) {
  let signals = 0;
  let rosters = 0;
  void page.route("**/api/graphql", async (route) => {
    const body = route.request().postData() ?? "";
    if (body.includes("sendPeerSignal")) signals += 1;
    if (body.includes("peerSessions")) rosters += 1;
    await route.fallback();
  });
  return { signals: () => signals, rosters: () => rosters };
}

interface Table {
  gmContext: BrowserContext;
  gm: Page;
  player: Page;
  worldId: string;
  sceneId: string;
}

/**
 * A GM and a player in one world, the GM holding a cached asset the player
 * does not have yet — the situation a peer transfer exists for.
 */
async function seatTable(
  browser: Parameters<typeof inviteAndJoinAsPlayer>[0],
  prefix: string,
): Promise<Table> {
  const gmContext = await browser.newContext({
    // The invite flow writes to the clipboard and throws without this.
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gm = await gmContext.newPage();
  const worldId = await registerAndCreateWorld(gm, `E2E Peer ${uniqueSuffix()}`);
  const [sceneId] = await sceneIds(gm, worldId);
  const player = await inviteAndJoinAsPlayer(browser, gm, worldId, prefix);
  return { gmContext, gm, player, worldId, sceneId };
}

test.describe("Client world cache — peer-assisted distribution", () => {
  test.setTimeout(300_000);

  /**
   * SC-012. The requester must verify what a peer sends against the
   * server-published fingerprint, discard it on mismatch, and end up with
   * the right content anyway.
   *
   * The assertion that carries the requirement is the *last* one: the player
   * ends up holding the correct bytes. A test that only checked "the
   * corrupted content was rejected" would pass on a client that rejected it
   * and then gave up — which fails FR-048 exactly as badly, and is the more
   * likely bug.
   */
  test("bytes a peer corrupts are rejected, and the server supplies them instead (SC-012, T093)", async ({
    browser,
  }) => {
    const { gmContext, gm, player, worldId, sceneId } = await seatTable(
      browser,
      "e2epeerbad",
    );

    // The GM lies on the wire from here on.
    await serveCorruptedBytes(gmContext);

    const assetId = await createCanvasAsset(gm, worldId, sceneId, 11);
    const fingerprint = await assetFingerprint(gm, assetId);

    const gmSync = watchCacheSync(gm);
    await openWorldAndSync(gm, worldId, gmSync);
    expect(
      await holdsFingerprint(gm, worldId, fingerprint),
      "the serving peer must actually hold the content first",
    ).toBe(true);

    const playerSync = watchCacheSync(player);
    await openWorldAndSync(player, worldId, playerSync);

    // Deliberately not gated on seeing `connectedPeers` reach one first.
    // That was the first shape of this test and it failed — because the
    // client drops a peer that lies the moment it catches it, so the count
    // rises and falls inside a window a poll can miss. The verification
    // counter below is the durable evidence, and it is strictly better: it
    // cannot be reached without a channel having formed, a transfer having
    // completed, and the result having been rejected.
    await expect
      .poll(() => peerCounters(player).then((c) => c.failures), {
        timeout: 90_000,
        message: "content that does not hash to what was asked for must be discarded",
      })
      .toBeGreaterThanOrEqual(1);

    // And the outcome is unchanged: the player has the real bytes, from the
    // server, filed under the fingerprint the server published. `holdsFingerprint`
    // reads OPFS, and `write_blob` refuses to file bytes under a name they do
    // not hash to — so this passing means the stored content is genuinely correct,
    // not merely present.
    await expect
      .poll(() => holdsFingerprint(player, worldId, fingerprint), {
        timeout: 90_000,
        message: "a peer that lies must cost time, never the content itself",
      })
      .toBe(true);

    await player.context().close();
    await gmContext.close();
  });

  /**
   * SC-014. A peer holding content the requester may not see must never be
   * a way to get it.
   *
   * The scene is hidden and *not* the one being played, which is the only
   * shape that genuinely denies a player: the scene a world is playing is
   * readable by the people playing it, hidden or not. So this also pins the
   * edge of that carve-out from the other side — if it ever widened to "any
   * hidden scene", this test fails rather than the rule quietly eroding.
   *
   * Nothing here has to trust the peer. Entitlement comes from the server's
   * plan, and the client cannot express a request for a fingerprint the plan
   * does not list.
   */
  test("a peer cannot supply content the requester was never entitled to (SC-014, T094)", async ({
    browser,
  }) => {
    const { gmContext, gm, player, worldId, sceneId } = await seatTable(
      browser,
      "e2epeerperm",
    );

    // Art the player is entitled to, on the scene being played.
    const sharedAssetId = await createCanvasAsset(gm, worldId, sceneId, 21);
    const sharedFingerprint = await assetFingerprint(gm, sharedAssetId);

    // Art on a hidden scene nobody has launched: the GM's prep.
    const prepSceneId = await createScene(gm, worldId, `Prep ${uniqueSuffix()}`);
    await setSceneHidden(gm, prepSceneId, true);
    const secretAssetId = await createCanvasAsset(gm, worldId, prepSceneId, 22);
    const secretFingerprint = await assetFingerprint(gm, secretAssetId);
    expect(
      secretFingerprint,
      "the two assets must differ, or this test proves nothing",
    ).not.toBe(sharedFingerprint);

    const gmSync = watchCacheSync(gm);
    await openWorldAndSync(gm, worldId, gmSync);
    await expect
      .poll(() => holdsFingerprint(gm, worldId, secretFingerprint), {
        timeout: 60_000,
        message: "the GM must hold the content this test says the player cannot get",
      })
      .toBe(true);

    const playerSync = watchCacheSync(player);
    await openWorldAndSync(player, worldId, playerSync);

    await expect
      .poll(() => peerCounters(player).then((c) => c.peers), { timeout: 60_000 })
      .toBeGreaterThanOrEqual(1);

    // The entitled content arrives, which is what makes the negative below
    // mean something: the channel is open and working.
    await expect
      .poll(() => holdsFingerprint(player, worldId, sharedFingerprint), {
        timeout: 90_000,
        message: "the player should still get what they are entitled to",
      })
      .toBe(true);

    expect(
      await holdsFingerprint(player, worldId, secretFingerprint),
      "a peer holding hidden-scene art must not be a way around the server's answer",
    ).toBe(false);

    // The server must not serve it either — the plan and the bytes have to
    // agree, which is the split `auth::scene_visibility` exists to close.
    const status = await player.evaluate(async (id) => {
      const res = await fetch(`/api/canvas-assets/${id}.webp`, {
        credentials: "same-origin",
        cache: "no-store",
      });
      return res.status;
    }, secretAssetId);
    expect(status, "the byte route must deny it too").not.toBe(200);

    await player.context().close();
    await gmContext.close();
  });

  /**
   * SC-013. With peer transfer off, every outcome is identical and only
   * timing differs.
   *
   * "Disabled" has to mean *no connection was ever attempted* — the IP
   * exposure the setting exists to prevent happens when the connection is
   * made, not when bytes move. So this counts signaling calls at the wire
   * rather than reading the client's own opinion of itself.
   */
  test("with peer transfer disabled the client is server-only, and the result is the same (SC-013, T095)", async ({
    browser,
  }) => {
    const { gmContext, gm, player, worldId, sceneId } = await seatTable(
      browser,
      "e2epeeroff",
    );

    // Set before any application script runs, which is what a user who
    // turned it off in a previous session actually has.
    await player.context().addInitScript(() => {
      window.localStorage.setItem("thunderforge:peer-transfer-enabled", "false");
    });

    const assetId = await createCanvasAsset(gm, worldId, sceneId, 31);
    const fingerprint = await assetFingerprint(gm, assetId);

    const gmSync = watchCacheSync(gm);
    await openWorldAndSync(gm, worldId, gmSync);
    expect(await holdsFingerprint(gm, worldId, fingerprint)).toBe(true);

    const signaling = countPeerSignaling(player);
    const playerSync = watchCacheSync(player);
    await openWorldAndSync(player, worldId, playerSync);

    // The outcome is the one that matters, and it is unchanged.
    await expect
      .poll(() => holdsFingerprint(player, worldId, fingerprint), {
        timeout: 90_000,
        message: "server-only transfer must reach exactly the same result",
      })
      .toBe(true);

    // Give a connection every chance to happen before concluding none did.
    await player.waitForTimeout(5_000);

    expect(
      signaling.signals(),
      "a disabled client must not signal — the exposure is the connection, not the bytes",
    ).toBe(0);
    expect(
      signaling.rosters(),
      "a disabled client must not even ask who else is here",
    ).toBe(0);
    const counters = await peerCounters(player);
    expect(counters.peers).toBe(0);
    expect(counters.bytes).toBe(0);

    await player.context().close();
    await gmContext.close();
  });
});
