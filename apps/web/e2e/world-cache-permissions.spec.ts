import { expect, test, type Browser, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import {
  assetFingerprint,
  assetStatus,
  countAssetRequests,
  createCanvasAsset,
  createScene,
  holdsFingerprint,
  openWorldAndSync,
  probeBlobs,
  sceneIds,
  serviceWorkerHoldsAsset,
  sessionKeyState,
  setSceneHidden,
  syncPlan,
  watchCacheSync,
  worldBlobs,
  type GqlResult,
} from "./fixtures/world-cache";


/**
 * Spec 028 User Story 2 (T042-T044): cached content stops being readable
 * when the permission that justified caching it goes away.
 *
 * This is the disclosure half of the feature. Story 1 proves the cache is
 * fast; these prove it is not a back door. Every assertion below is about
 * one of three things:
 *
 *   1. the server refuses (FR-014 — authorization is re-established on every
 *      open, never inferred from possession),
 *   2. the bytes leave the local store (FR-015),
 *   3. whatever bytes remain are inert without the session key (FR-016,
 *      FR-016a, FR-016b).
 *
 * # Every fixture here asserts its own success
 *
 * The repeated failure mode of this feature has been a helper that swallowed
 * its error and produced a confident, wrong green: a storm with no events, a
 * "starvation" that was an inert stimulus, a cold open reporting zero
 * requests because nothing had ever uploaded. A revocation test is
 * especially vulnerable to it — "the cached bytes are gone" is
 * indistinguishable from "the bytes were never written" unless the setup
 * proved they were written. So each test asserts the *positive* state first
 * (the player could read the asset, the blob is on disk under its
 * fingerprint) and only then revokes.
 */

/** The player member of a world, from the GM's session. Asserts there is one. */
async function playerUserId(page: Page, worldId: string): Promise<string> {
  const res = await graphql<
    GqlResult<{ worldMembers: { userId: string; role: string }[] }>
  >(
    page,
    `
      query ($worldId: ID!) {
        worldMembers(worldId: $worldId) {
          userId
          role
        }
      }
    `,
    { worldId },
  );
  expect(res.errors, `worldMembers failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  const players = (res.data?.worldMembers ?? []).filter(
    (m) => m.role === "Player",
  );
  expect(
    players.length,
    `expected exactly one Player member, saw ${JSON.stringify(res.data?.worldMembers)}`,
  ).toBe(1);
  return players[0].userId;
}



test.describe("Client world cache — losing access (US2, T042-T044)", () => {
  // Two browser contexts, an engine start in each, a real upload and two full
  // world opens. All of it is setup for a handful of assertions.
  test.setTimeout(300_000);

  /**
   * The whole of T042's setup: a player caches a world, then loses their
   * membership, then opens it again.
   *
   * Every step asserts itself. In particular it proves the player *could*
   * read the asset and *did* end up holding it on disk before anything is
   * revoked — without that, "the bytes are gone" afterwards would be
   * indistinguishable from "the bytes were never there", which is the exact
   * false green this feature keeps producing.
   */
  async function cacheAWorldThenRevokeMembership(
    browser: Browser,
    gm: Page,
  ): Promise<{
    player: Page;
    worldId: string;
    assetId: string;
    fingerprint: string;
    sync: ReturnType<typeof watchCacheSync>;
  }> {
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Revoke ${uniqueSuffix()}`,
      "e2erevokegm",
    );

    // scenes.hidden defaults to true — without this the player is a member of
    // a world with nothing in it they may see, and every later assertion
    // would pass vacuously.
    const scenes = await sceneIds(gm, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    await setSceneHidden(gm, scenes[0], false);
    const assetId = await createCanvasAsset(gm, worldId, scenes[0], 12345);

    const player = await inviteAndJoinAsPlayer(
      browser,
      gm,
      worldId,
      "e2erevokepl",
    );
    const sync = watchCacheSync(player);

    // Precondition, asserted: while a member, the player really can read the
    // asset's bytes — and this is the fingerprint they will be stored under.
    const fingerprint = await assetFingerprint(player, assetId);

    const first = await openWorldAndSync(player, worldId, sync);
    expect(
      first.status,
      `the player's first sync should succeed: ${JSON.stringify(first)}`,
    ).toBe("synced");
    expect(
      first.fetch,
      "a cold open should be told to fetch the world's asset",
    ).toBeGreaterThan(0);

    // Prefetch is deliberately not awaited by the sync, so poll for the blob
    // rather than assuming it is already written.
    await expect
      .poll(() => holdsFingerprint(player, worldId, fingerprint), {
        timeout: 60_000,
        message: "the player should end up holding the asset locally",
      })
      .toBe(true);
    const heldBefore = await worldBlobs(player, worldId);
    console.log(
      `[cache-perms] player holds ${heldBefore.length} blob(s) for ${worldId} before revocation`,
    );

    // --- revoke ---
    const targetUser = await playerUserId(gm, worldId);
    const removal = await graphql<GqlResult<{ removeMember: boolean }>>(
      gm,
      `
        mutation ($worldId: ID!, $userId: ID!) {
          removeMember(worldId: $worldId, userId: $userId)
        }
      `,
      { worldId, userId: targetUser },
    );
    expect(
      removal.errors,
      `removeMember failed: ${JSON.stringify(removal.errors)}`,
    ).toBe(undefined);
    expect(removal.data?.removeMember, "removeMember should report true").toBe(
      true,
    );

    // --- the next open ---
    const before = sync.count();
    await player.goto(`/world/${worldId}/play`);
    // No waitForEngineReady: a denied open is not required to reach a
    // running canvas, and insisting on one would make the test assert the
    // failure mode's cosmetics instead of its substance.
    await sync.next(before);

    return { player, worldId, assetId, fingerprint, sync };
  }

  test("a revoked member is denied on their next open (T042, SC-004)", async ({
    browser,
    page,
  }) => {
    const { player, worldId, assetId, sync } =
      await cacheAWorldThenRevokeMembership(browser, page);

    // FR-014: authorization is re-established from the database on every
    // open. The server must refuse this session outright, and its refusal
    // must not distinguish "you may not see this" from "no such world".
    const plan = await syncPlan(player, worldId);
    expect(
      plan.errors?.[0]?.message,
      `worldSyncPlan should be refused: ${JSON.stringify(plan)}`,
    ).toContain("not a member of this world");
    expect(plan.data?.worldSyncPlan ?? null).toBeNull();

    // And the origin no longer serves them the bytes.
    expect(
      await assetStatus(player, assetId),
      "a revoked member must not be able to fetch the asset from the server",
    ).not.toBe(200);

    // T045b: the service worker was a second local store that outlived this
    // revocation. It kept a cache-first plaintext copy of every
    // `/api/canvas-assets/*` response, per browser profile, cleared only on
    // logout — and a revoked member stays signed in. It now caches nothing
    // and purges on activate, so no Cache Storage entry may hold these bytes.
    expect(
      await serviceWorkerHoldsAsset(player, assetId),
      "no Cache Storage entry may hold plaintext bytes for a revoked asset " +
        "(see public/sw.js — it caches nothing and purges on activate)",
    ).toBe(0);

    sync.stop();
    await player.context().close();
  });

  /**
   * The other half of SC-004, and the half that does not hold yet.
   *
   * This was marked `test.fail()` when written, because nothing implemented
   * FR-015 for *whole-world* revocation: eviction was driven entirely by the
   * `evict` list of a successful plan, and a revoked member's plan request is
   * refused outright, so `run_sync` took its transport-error branch,
   * republished what it already held, and returned `degraded` — leaving the
   * blobs on disk and readable under that user's still-live session key.
   *
   * T045a added the missing path and this now passes. The marker is gone.
   *
   * The distinction that makes that path safe, and the thing to preserve if
   * this test ever needs changing: an *authorization refusal* discards, a
   * *transient failure* does not. The client keys off the server's
   * `extensions.code == "FORBIDDEN"`, not a message substring and not "any
   * error" — because discarding a user's cache every time their connection
   * blips would cost more than the bug this fixes.
   */
  test("a revoked member's cached copy is discarded, not merely unusable (T042, SC-004)", async ({
    browser,
    page,
  }) => {
    const { player, worldId, sync } = await cacheAWorldThenRevokeMembership(
      browser,
      page,
    );

    await expect
      .poll(() => worldBlobs(player, worldId).then((b) => b.length), {
        timeout: 30_000,
        message:
          "a revoked member's locally-held blobs for the world must be discarded",
      })
      .toBe(0);

    sync.stop();
    await player.context().close();
  });

  test("hiding one scene evicts only its asset; the rest still loads from cache (T043, US2 scenario 2)", async ({
    browser,
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Partial ${uniqueSuffix()}`,
      "e2epartialgm",
    );

    // Two scenes, both player-visible to start with. `scenes.hidden` is the
    // per-object visibility axis that actually applies here: ADR-050's
    // permission ladder covers actors/items/abilities/lore, not scenes or
    // canvas assets, so asset visibility rides on world membership plus this
    // GM-only flag (see authorized_current in world_sync_plan.rs).
    const existing = await sceneIds(page, worldId);
    expect(existing.length, "a new world should have a scene").toBeGreaterThan(
      0,
    );
    const keptScene = existing[0];
    const hiddenScene = await createScene(page, worldId, "To be hidden");
    await setSceneHidden(page, keptScene, false);
    await setSceneHidden(page, hiddenScene, false);

    // Different noise seeds, so the two assets are genuinely different bytes
    // and land under different filenames.
    const keptAsset = await createCanvasAsset(page, worldId, keptScene, 999);
    const doomedAsset = await createCanvasAsset(
      page,
      worldId,
      hiddenScene,
      4242,
    );

    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2epartialpl",
    );
    const sync = watchCacheSync(player);

    // Both readable while both scenes are visible — asserted, and the
    // fingerprints are the on-disk filenames to look for.
    const keptFingerprint = await assetFingerprint(player, keptAsset);
    const doomedFingerprint = await assetFingerprint(player, doomedAsset);
    expect(
      keptFingerprint,
      "the two assets must differ, or this test cannot tell them apart",
    ).not.toBe(doomedFingerprint);

    const first = await openWorldAndSync(player, worldId, sync);
    expect(first.status, JSON.stringify(first)).toBe("synced");
    expect(
      first.fetch,
      "a cold open should be told to fetch both assets",
    ).toBeGreaterThanOrEqual(2);

    await expect
      .poll(
        async () =>
          (await holdsFingerprint(player, worldId, keptFingerprint)) &&
          (await holdsFingerprint(player, worldId, doomedFingerprint)),
        {
          timeout: 60_000,
          message: "the player should end up holding both assets locally",
        },
      )
      .toBe(true);

    // --- partial revocation: one scene becomes GM-only ---
    await setSceneHidden(page, hiddenScene, true);

    // The server's answer, first. Claiming to hold both, the player is now
    // told to evict exactly the one they may no longer see — and told
    // nothing at all about the one they may, which is what "already current"
    // looks like on this wire.
    //
    // The byte route is asserted below, after the plan. It used to be the
    // gap this comment described: `GET /api/canvas-assets/<id>` authorized on
    // world membership alone, the player was still a member, and a URL they
    // already knew kept answering long after the plan stopped admitting the
    // asset existed. T045c closed it — both now ask
    // `auth::scene_visibility`.
    const plan = await syncPlan(player, worldId, [
      { id: `asset:${keptAsset}`, fingerprint: keptFingerprint },
      { id: `asset:${doomedAsset}`, fingerprint: doomedFingerprint },
    ]);
    expect(
      plan.errors,
      `worldSyncPlan failed: ${JSON.stringify(plan.errors)}`,
    ).toBe(undefined);
    expect(
      plan.data?.worldSyncPlan.evict,
      "the hidden scene's asset must be evicted",
    ).toContain(`asset:${doomedAsset}`);
    expect(
      plan.data?.worldSyncPlan.evict,
      "the visible scene's asset must not be evicted",
    ).not.toContain(`asset:${keptAsset}`);
    expect(
      (plan.data?.worldSyncPlan.fetch ?? []).map((f) => f.id),
      "nothing the player may not see may appear in fetch",
    ).not.toContain(`asset:${doomedAsset}`);

    // T045c: and the bytes themselves. The player knows this id — they were
    // holding the asset a moment ago — so this is exactly the case the plan
    // cannot cover: a direct fetch of a URL already in hand. 404, not 403,
    // because a hidden asset and a nonexistent one must be indistinguishable.
    expect(
      await assetStatus(player, doomedAsset),
      "the byte route must stop serving art from a scene the player cannot see",
    ).toBe(404);
    expect(
      await assetStatus(player, keptAsset),
      "the visible scene's asset must still be served — the refusal is about " +
        "scene visibility, not about the player having lost the world",
    ).toBe(200);

    // --- reopen ---
    const requestsForKept = countAssetRequests(player, keptAsset);
    const before = sync.count();
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    const second = await sync.next(before);
    console.log(
      `[cache-perms] partial-revocation sync ${JSON.stringify(second)}`,
    );

    expect(second.status, JSON.stringify(second)).toBe("synced");
    expect(
      second.held,
      "the reopen should present the two blobs the player already holds",
    ).toBeGreaterThanOrEqual(2);
    expect(
      second.evicted,
      "the now-hidden scene's asset should be evicted",
    ).toBeGreaterThanOrEqual(1);
    expect(
      second.blobsRemoved,
      "eviction should actually delete bytes, not just index rows",
    ).toBeGreaterThanOrEqual(1);

    // FR-015 for the forbidden half, FR-017 for the permitted half: the
    // asset the player may no longer see is gone from disk, and the one they
    // may still see is still there.
    await expect
      .poll(() => holdsFingerprint(player, worldId, doomedFingerprint), {
        timeout: 60_000,
        message: "the hidden scene's asset must be discarded locally",
      })
      .toBe(false);
    expect(
      await holdsFingerprint(player, worldId, keptFingerprint),
      "the still-permitted asset must survive the eviction",
    ).toBe(true);

    // ...and it was served from that local copy rather than re-downloaded.
    const refetched = requestsForKept.stop();
    console.log(
      `[cache-perms] requests for the kept asset on reopen: ${refetched}`,
    );
    expect(
      refetched,
      "the still-permitted asset should load from cache, not the network",
    ).toBe(0);

    sync.stop();
    await player.context().close();
  });

  test("after sign-out the stored bytes are inert, checked in OPFS before any cleanup (T044, SC-004a)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Signout ${uniqueSuffix()}`,
      "e2esignout",
    );
    const scenes = await sceneIds(page, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 7777);

    const sync = watchCacheSync(page);
    const fingerprint = await assetFingerprint(page, assetId);
    const summary = await openWorldAndSync(page, worldId, sync);
    expect(summary.status, JSON.stringify(summary)).toBe("synced");

    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 60_000,
        message: "there must be a stored blob before sign-out to reason about",
      })
      .toBe(true);

    // Even before sign-out the file is not the asset: it does not hash to the
    // fingerprint it is named for, and carries none of the image's framing.
    // (Nonce + GCM tag is 28 bytes of overhead over the plaintext.)
    const sealedBefore = await probeBlobs(page, worldId);
    expect(sealedBefore.length).toBeGreaterThan(0);
    for (const blob of sealedBefore) {
      expect(
        blob.sha256,
        `${blob.path} must not be the plaintext it is named for`,
      ).not.toBe(blob.claimedFingerprint);
      expect(blob.looksLikeImage, `${blob.path} must not be a bare image`).toBe(
        false,
      );
    }

    const keyStateBefore = await sessionKeyState(page);

    // --- sign out, through the app's own control ---
    //
    // Through the UI, not by posting to the logout endpoint: anything the
    // app wires into sign-out (clearing the service worker's asset cache
    // already lives there) runs on this path and would be skipped by a bare
    // API call, which would quietly make this test weaker than it looks.
    await page.goto("/worlds");
    await page.getByRole("button", { name: "Menu" }).click();
    await page.getByRole("menuitem", { name: "Sign out" }).click();
    await page.waitForURL(/\/login$/, { timeout: 15_000 });
    // Asserted, not assumed: the session really is gone. Asked of the origin
    // (the `?probe=` URL bypasses the asset service worker), because the
    // whole test is meaningless if the sign-out silently failed.
    expect(
      await assetStatus(page, assetId),
      "a signed-out session must not be served world content",
    ).not.toBe(200);

    // --- immediately, before any lazy reclamation could have run ---
    const surviving = await probeBlobs(page, worldId);
    console.log(
      `[cache-perms] ${surviving.length} blob(s) survive sign-out for ${worldId}`,
    );

    // The claim is *not* "the files are gone" — FR-016b explicitly allows
    // reclamation to be lazy, and relying on deletion for confidentiality is
    // the mistake this requirement exists to prevent. The claim is that
    // whatever survives is unreadable: sealed AES-GCM, opening for nobody who
    // does not hold the key, and the key itself is non-extractable so it can
    // never be carried off with the bytes.
    for (const blob of surviving) {
      expect(
        blob.sha256,
        `${blob.path} must not have become plaintext`,
      ).not.toBe(blob.claimedFingerprint);
      expect(blob.looksLikeImage, `${blob.path} must not be a bare image`).toBe(
        false,
      );
      expect(
        blob.decryptsWithAForeignKey,
        `${blob.path} must not open under a key that is not the session key`,
      ).toBe(false);
    }

    // FR-016a: the key is discarded on sign-out, which is *why* the bytes
    // above are inert rather than merely awaiting deletion. Both readings are
    // printed, because "keys=0" only means something next to a "keys=1" taken
    // before — on its own it could equally be a probe that found no database
    // and made an empty one.
    const keyStateAfter = await sessionKeyState(page);
    console.log(
      `[cache-perms] session key before sign-out: ${keyStateBefore}; ` +
        `after: ${keyStateAfter}`,
    );
    expect(
      keyStateBefore,
      "the cache must actually have held a session key before sign-out",
    ).toContain("keys=1");
    expect(
      keyStateAfter,
      "sign-out must discard the session key (FR-016a)",
    ).toContain("keys=0");

    sync.stop();
  });
});
