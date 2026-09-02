import { expect, test } from "@playwright/test";
import type { WorldProbe } from "../src/engine/world/probe";
import {
  freshCredentials,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

/**
 * Preload prepares; Launch moves the table. Spec 031 FR-020, SC-004.
 *
 * # Why this needs an end-to-end test at all
 *
 * The whole value of Preload is negative: it is defined by what does *not*
 * happen at the table. A unit test can assert that `preloadScene` issues no
 * mutation, but that is a statement about one function, and the requirement is
 * a statement about every connected player. ADR-046 makes the active scene
 * server-authoritative and broadcast, so the failure mode is a single stray
 * write on the wrong path — which is exactly the kind of thing that passes
 * every test written around the function that was supposed to do it.
 *
 * So this watches the player, not the Game Master.
 *
 * # Why the assertion is shaped as "nothing new arrived"
 *
 * Asserting the player still sees scene A is weaker than it looks: they would
 * also still see scene A during the second before a broadcast landed. Reading
 * the player's command log and requiring it to be unchanged across a settle
 * window catches the transient too, and names what went wrong when it fails.
 */

/**
 * Commands that mean "the table moved", as the store actually logs them.
 *
 * Taken from the store's own emitters rather than guessed. A plausible-looking
 * name that nothing ever emits would make this test count zero of them, pass
 * unconditionally, and assert nothing at all — which is worse than no test,
 * because it reads like coverage.
 */
const SCENE_CHANGING = [
  "set_scene_grid",
  "set_scene_background",
  "set_scene_playing",
];

async function sceneCommandCount(
  page: import("@playwright/test").Page,
): Promise<number> {
  return page.evaluate(
    (types) =>
      window.__worldProbe
        ?.commands()
        .filter((command) => types.includes(command.type)).length ?? 0,
    SCENE_CHANGING,
  );
}

test("a connected player observes nothing when the GM preloads another scene", async ({
  browser,
}) => {
  test.setTimeout(240_000);

  // Clipboard permission is what the invite flow writes through; without it
  // the invite is never stored (see scene-live-launch.spec.ts).
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();

  await register(gmPage, freshCredentials("e2egmpreload"));
  await gmPage.goto("/worlds/create");
  const worldName = `E2E Preload ${uniqueSuffix()}`;
  await gmPage.locator("#world-name").fill(worldName);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(gmPage.url()).pathname,
  )![1];

  // A second scene to preload. The world's auto-created scene is the one the
  // table will be playing, so preloading it would prove nothing — it is
  // already loaded.
  const otherScene = `Preload Target ${uniqueSuffix()}`;
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByTestId("new-scene-name-input").fill(otherScene);
  await gmPage.getByTestId("add-scene-button").click();
  await expect(gmPage.getByRole("link", { name: otherScene })).toBeVisible({
    timeout: 15_000,
  });

  // Put the table on the *first* scene, so there is something for the player
  // to be watching that a stray write could disturb.
  const scenes = await gmPage.getByTestId("scenes-table").getByRole("link").all();
  const playingSceneName = (
    await Promise.all(scenes.map((link) => link.textContent()))
  ).find((name) => name && name !== otherScene);
  expect(
    playingSceneName,
    "the world should have an auto-created scene besides the one under test",
  ).toBeTruthy();
  await launchSceneByName(gmPage, worldId, playingSceneName!);

  const playerPage = await inviteAndJoinAsPlayer(
    browser,
    gmPage,
    worldId,
    "e2eplpreload",
  );

  await playerPage.goto(`/world/${worldId}/play`);
  await expect(playerPage.locator("canvas")).toBeVisible({ timeout: 30_000 });

  // Let the player's scene finish arriving. Measuring the baseline while
  // commands were still streaming in would make any later count meaningless.
  await expect
    .poll(() => sceneCommandCount(playerPage), {
      timeout: 60_000,
      message: "the player must receive the scene they are playing first",
    })
    .toBeGreaterThan(0);
  await expect(playerPage.getByTestId("scene-load-indicator")).toHaveCount(0, {
    timeout: 60_000,
  });

  const before = await sceneCommandCount(playerPage);
  const tokensBefore = await playerPage.evaluate(
    () => window.__worldProbe?.state().tokenIds.length ?? -1,
  );

  // The act under test.
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByRole("link", { name: otherScene }).click();
  await gmPage.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), {
    timeout: 15_000,
  });
  await gmPage.getByTestId("preload-scene-button").click();
  await expect(gmPage.getByTestId("scene-action-explainer")).toBeVisible();
  // Preload resolves either way by design — a scene with no background has
  // nothing to warm, and that is a legitimate outcome rather than a failure.
  // What matters here is only that it finished, so the settle window below
  // starts after the work, not during it.
  await expect(gmPage.getByTestId("preload-scene-button")).toHaveText(
    /^Preload$/,
    { timeout: 30_000 },
  );

  // The Game Master is still on the scene page. Preload does not enter play;
  // that is Launch's job and the difference the interface promises.
  expect(new URL(gmPage.url()).pathname).toMatch(
    new RegExp(`^/world/${worldId}/scenes/[^/]+$`),
  );

  // A settle window, because a broadcast that had been sent would arrive
  // within it. Deliberately generous: this test's whole job is to be slow
  // enough that a real regression cannot slip past it.
  await gmPage.waitForTimeout(5_000);

  expect(
    await sceneCommandCount(playerPage),
    "preloading must not send the player a scene change",
  ).toBe(before);
  expect(
    await playerPage.evaluate(
      () => window.__worldProbe?.state().tokenIds.length ?? -1,
    ),
    "preloading must not change what is on the player's map",
  ).toBe(tokensBefore);
  await expect(playerPage.getByTestId("scene-load-indicator")).toHaveCount(0);

  await playerPage.context().close();
  await gmContext.close();
});
