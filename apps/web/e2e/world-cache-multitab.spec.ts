import { expect, test, type Page } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import {
  assetFingerprint,
  assetStatus,
  countAssetRequests,
  createCanvasAsset,
  holdsFingerprint,
  openWorldAndSync,
  probeBlobs,
  sceneIds,
  sessionKeyState,
  watchCacheSync,
  worldBlobs,
} from "./fixtures/world-cache";

/**
 * Spec 028 User Story 3 (T055e, T055f): the cache behaves when the same
 * browser profile has the app open more than once.
 *
 * The implementation for this landed with unit and mutation coverage — Web
 * Locks around key creation and eviction, two signal carriers for sign-out —
 * but its browser legs were unproven, and they are the only place the
 * platform primitives involved actually exist. `navigator.locks`,
 * `BroadcastChannel`, the `storage` event and OPFS's shared-per-origin
 * semantics have no native test double worth trusting; a Rust unit test can
 * prove the ordering the code intends and nothing at all about whether two
 * real tabs agree.
 *
 * The division of labour runs the other way too, and the first test below
 * says so where it matters: a browser cannot schedule two engine boots into
 * the same millisecond, so the *collision* these locks exist for is the
 * crate's tests' job. These tests prove the outcome in a real profile.
 *
 * # Both tabs live in one browser context, deliberately
 *
 * A Playwright *context* is the profile boundary: same cookies, same OPFS,
 * same IndexedDB, same lock manager. Two contexts would be two profiles and
 * would share none of it, which would make every assertion here pass for the
 * wrong reason — no key to race over, no store to corrupt, no signal to
 * receive. `context.newPage()` is what makes these tabs rather than
 * browsers.
 */

/**
 * Resolve when `text` appears in this page's console, or when `timeout`
 * elapses.
 *
 * Used for exactly one thing: the engine's own report that it dropped its
 * in-memory key. That drop has no other external evidence — it is a
 * `CryptoKey` leaving a wasm-side `RefCell`, with no storage write, no
 * request, and no DOM change of its own — so the log line is the observation
 * point. The behavioural consequences are asserted separately below; this
 * says *the receiving tab reacted*, they say *the reaction had the intended
 * effect*.
 *
 * Must be installed before the event it waits for: a console listener sees
 * nothing that happened before it was attached.
 */
function waitForConsole(
  page: Page,
  text: string,
  timeout = 20_000,
): Promise<boolean> {
  return new Promise((resolve) => {
    const onConsole = (msg: { text(): string }) => {
      if (msg.text().includes(text)) {
        page.off("console", onConsole);
        resolve(true);
      }
    };
    page.on("console", onConsole);
    setTimeout(() => {
      page.off("console", onConsole);
      resolve(false);
    }, timeout);
  });
}

/** Sign out through the app's own header control, as a person would. */
async function signOutViaUi(page: Page): Promise<void> {
  await page.goto("/worlds");
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await page.waitForURL(/\/login$/, { timeout: 15_000 });
}

test.describe("Client world cache — more than one tab (US3, T055e/T055f)", () => {
  // Two engine starts, a real upload, and several full page loads each.
  test.setTimeout(240_000);

  test("two tabs opening one world share one key and one store, and both stay warm (T055e, FR-021, FR-021a)", async ({
    browser,
  }) => {
    const context = await browser.newContext();
    const tabA = await context.newPage();

    const worldId = await registerAndCreateWorld(
      tabA,
      `E2E Multitab ${uniqueSuffix()}`,
      "e2emultitab",
    );
    const scenes = await sceneIds(tabA, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    const assetId = await createCanvasAsset(tabA, worldId, scenes[0], 4242);
    const fingerprint = await assetFingerprint(tabA, assetId);

    const tabB = await context.newPage();
    const syncA = watchCacheSync(tabA);
    const syncB = watchCacheSync(tabB);

    // The race, run on purpose. Neither tab has a session key yet, so both
    // reach for one at the same moment — which is precisely FR-021a's
    // scenario. Sequential opens would prove nothing: the second tab would
    // simply find the first tab's key already stored.
    const [summaryA, summaryB] = await Promise.all([
      openWorldAndSync(tabA, worldId, syncA),
      openWorldAndSync(tabB, worldId, syncB),
    ]);
    console.log(
      `[cache-multitab] tabA=${JSON.stringify(summaryA)} tabB=${JSON.stringify(summaryB)}`,
    );

    // Neither tab is allowed to have degraded. A cache that failed in one tab
    // and worked in the other would satisfy every "no corruption" check below
    // while being exactly the outcome FR-021a exists to prevent.
    expect(summaryA.status, JSON.stringify(summaryA)).toBe("synced");
    expect(summaryB.status, JSON.stringify(summaryB)).toBe("synced");

    // One key for the profile, not one per tab. Two keys is not a corruption
    // — everything written under the loser simply stops opening, which
    // degrades to a cache miss (FR-016c) — but a cache that silently never
    // works is still broken.
    //
    // # What this does not prove, and where that is proved instead
    //
    // This is *not* evidence that the FR-021a Web Lock serialises anything.
    // Measured, not assumed: with key creation mutated to take no cross-tab
    // lock at all, this test still passes, keys=1 and both tabs warm. Two
    // real tabs never collide at key-creation time — a second engine boot is
    // seconds behind the first, so the later tab's re-check finds the earlier
    // tab's stored key whether or not a lock was held. That is exactly what
    // `without_web_locks_both_tabs_still_finish_with_a_key` in
    // `crates/thunderforge-cache-browser/src/crypto.rs` already states about
    // the degraded path.
    //
    // The *ordering* — that the re-check and the generation both happen
    // inside the critical section — is proved by the interleaving model in
    // that crate's tests (`the_recheck_happens_with_the_lock_held`), which
    // can schedule the collision a browser cannot. What this assertion adds
    // is the outcome those tests cannot reach: that two real tabs, with a
    // real lock manager and a real IndexedDB, converge on one key.
    const keys = await sessionKeyState(tabA);
    console.log(`[cache-multitab] session key state: ${keys}`);
    expect(keys, "two tabs must not each end up on their own key").toContain(
      "keys=1",
    );

    // FR-021: one store, one copy of the asset. Both tabs resolved the same
    // scene background at the same time, so both had bytes in hand for the
    // same fingerprint at roughly the same moment.
    await expect
      .poll(() => holdsFingerprint(tabA, worldId, fingerprint), {
        timeout: 60_000,
        message: "the concurrently-opened world should end up cached",
      })
      .toBe(true);

    const blobs = await worldBlobs(tabA, worldId);
    console.log(
      `[cache-multitab] blobs: ${blobs.map((b) => `${b.path}=${b.size}B`).join(", ")}`,
    );
    expect(
      blobs.filter((b) => b.path.endsWith(`/${fingerprint}.bin`)).length,
      "the asset must be stored once, not once per tab",
    ).toBe(1);

    // "Never readable as complete" starts with never being empty. A file
    // created at its final name and not yet written is the shape a racing
    // reader would misread, and OPFS's own write semantics — a swap file
    // committed on close — are what should make it unobservable.
    for (const blob of blobs) {
      expect(blob.size, `${blob.path} must not be a zero-length stub`).toBeGreaterThan(0);
    }

    // And what is on disk is sealed, not the image. Cheap to check here, and
    // it means a concurrency bug that wrote plaintext could not hide behind
    // the byte-count assertions above.
    for (const blob of await probeBlobs(tabA, worldId)) {
      expect(
        blob.sha256,
        `${blob.path} must not be the plaintext it is named for`,
      ).not.toBe(blob.claimedFingerprint);
      expect(blob.looksLikeImage, `${blob.path} must not be a bare image`).toBe(
        false,
      );
    }

    // FR-021's second half: the cache still *works*, in both. A store that
    // survived the race but that neither tab can read from would pass every
    // assertion so far. Each tab reloads and must serve the asset locally.
    for (const [name, tab] of [
      ["A", tabA],
      ["B", tabB],
    ] as const) {
      const requests = countAssetRequests(tab, assetId);
      await tab.reload();
      await waitForEngineReady(tab);
      const refetched = requests.stop();
      console.log(`[cache-multitab] tab ${name} refetches on reload: ${refetched}`);
      expect(
        refetched,
        `tab ${name} should serve the asset from the shared local store`,
      ).toBe(0);
    }

    syncA.stop();
    syncB.stop();
    await context.close();
  });

  test("signing out in one tab makes cached content unreadable in another, without reloading it (T055f, FR-021b, FR-021e)", async ({
    browser,
  }) => {
    const context = await browser.newContext();
    const tabA = await context.newPage();

    const worldId = await registerAndCreateWorld(
      tabA,
      `E2E Multitab Signout ${uniqueSuffix()}`,
      "e2emtsignout",
    );
    const scenes = await sceneIds(tabA, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    const assetId = await createCanvasAsset(tabA, worldId, scenes[0], 5150);
    const fingerprint = await assetFingerprint(tabA, assetId);

    // Tab B is the one this test is about: it holds a live `CryptoKey` in
    // wasm memory, and deleting the stored record cannot reach it.
    const tabB = await context.newPage();
    const syncB = watchCacheSync(tabB);
    const summary = await openWorldAndSync(tabB, worldId, syncB);
    expect(summary.status, JSON.stringify(summary)).toBe("synced");

    await expect
      .poll(() => holdsFingerprint(tabB, worldId, fingerprint), {
        timeout: 60_000,
        message: "there must be cached content to lose before signing out",
      })
      .toBe(true);
    expect(
      await sessionKeyState(tabB),
      "the cache must hold a session key before sign-out",
    ).toContain("keys=1");

    // Prove tab B is reading from the cache *now*. Without this, "it stopped
    // serving from the cache" afterwards is unfalsifiable — a tab that never
    // served from the cache passes that assertion too. This reload is also
    // the last one tab B is allowed: everything after it must hold without
    // the page being reloaded.
    const warm = countAssetRequests(tabB, assetId);
    await tabB.reload();
    await waitForEngineReady(tabB);
    const warmRefetches = warm.stop();
    console.log(`[cache-multitab] tab B refetches while warm: ${warmRefetches}`);
    expect(
      warmRefetches,
      "tab B should be serving the asset from the cache before sign-out",
    ).toBe(0);

    // The whole point of FR-021b is "without requiring those tabs to
    // reload", so that has to be checked rather than assumed. A value on
    // `window` survives client-side navigation and does not survive a
    // document load, which makes it a direct answer to "is this the same
    // document that was warm a moment ago?".
    await tabB.evaluate(() => {
      (window as unknown as Record<string, string>).__multitabSentinel =
        "same-document";
    });

    // Installed before the sign-out, because a console listener cannot see
    // what already happened.
    const engineDroppedKey = waitForConsole(tabB, "dropping the cache key");

    await signOutViaUi(tabA);

    // FR-021e/FR-021f: tab B stops presenting a signed-in application. It
    // never mounted a fresh document to learn this — the announcement
    // reached it.
    // `/login?returnTo=…` — the guard preserves where the tab was, so the
    // match cannot be anchored at the end of the path.
    await tabB.waitForURL(/\/login(\?|$)/, { timeout: 20_000 });
    expect(
      await tabB.evaluate(
        () => (window as unknown as Record<string, string>).__multitabSentinel,
      ),
      "tab B must have reacted in the same document — a reload would make " +
        "this test prove nothing about FR-021b",
    ).toBe("same-document");

    // FR-021b: the receiving tab let go of the key it was holding in memory.
    expect(
      await engineDroppedKey,
      "tab B's engine should report dropping its in-memory cache key",
    ).toBe(true);

    // The stored record is gone too, which is what makes the drop permanent
    // rather than a thing the next read would undo.
    const keysAfter = await sessionKeyState(tabB);
    console.log(`[cache-multitab] tab B key state after sign-out: ${keysAfter}`);
    expect(keysAfter, "sign-out must discard the stored key").toContain(
      "keys=0",
    );

    // FR-016b: the bytes may still be on disk — reclamation is allowed to be
    // lazy, and it is never what makes the content safe. What must hold is
    // that they are inert.
    const surviving = await probeBlobs(tabB, worldId);
    console.log(
      `[cache-multitab] ${surviving.length} blob(s) survive sign-out in tab B`,
    );
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

    // And the network is not a way back in either: the session cookie is
    // shared, so tab B lost it the moment tab A signed out.
    expect(
      await assetStatus(tabB, assetId),
      "a signed-out tab must not be served world content",
    ).not.toBe(200);

    syncB.stop();
    await context.close();
  });
});
