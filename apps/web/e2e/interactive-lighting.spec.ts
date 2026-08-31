import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, User Story 3 — a lever on a wall changes the lighting for
 * everybody.
 *
 * This is the story that first says something about the *architecture* rather
 * than about a feature: it is the second independent contributor, added
 * without editing the interaction core, which is the earliest point at which
 * the seam is more than a shape with one user.
 *
 * # Two things worth asserting that a screenshot would not catch
 *
 * A light is "off" when its intensity is zero, so switching one off destroys
 * the only record of how bright it was. A lever pulled twice must return the
 * room to the brightness the Game Master chose, not to some default — so the
 * exact value is checked on the way back.
 *
 * And a switch wired to a lamp the GM has since deleted must still work on the
 * rest, and must *say* so. Silently doing four fifths of the job is how a GM
 * concludes the whole switch is broken (FR-019).
 */

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

async function intensities(
  page: Page,
  sceneId: string,
): Promise<Record<string, number>> {
  const data = await gql<{
    lightSources: { lightId: string; intensity: number }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      lightSources(sceneId: $sceneId) { lightId intensity }
    }`,
    { sceneId },
  );
  return Object.fromEntries(
    data.lightSources.map((l) => [l.lightId, l.intensity]),
  );
}

test("a switch changes the lighting for the whole table", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Lights ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // Three lamps at deliberately different brightnesses, so restoring one to
  // "1.0" rather than to its own value would fail rather than coincide.
  const chosen = [0.4, 0.7, 0.9];
  const lightIds: string[] = [];
  for (const [index, intensity] of chosen.entries()) {
    const created = await gql<{ createLightSource: { lightId: string } }>(
      page,
      `mutation ($input: GraphQLCreateLightSourceInput!) {
        createLightSource(input: $input) { lightId }
      }`,
      {
        input: {
          sceneId,
          x: index * 100,
          y: 0,
          radius: 120,
          intensity,
          castsShadows: true,
        },
      },
    );
    lightIds.push(created.createLightSource.lightId);
  }

  // --- a lever on the wall ------------------------------------------------

  const lever = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: -100, y: 0, tokenType: "object" } },
  );

  const created = await gql<{
    createInteractive: { interactiveId: string; available: boolean };
  }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId available }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: lever.createToken.tokenId,
        effectId: "light.toggle",
        effectConfig: { lights: lightIds, mode: "toggle" },
        trigger: "click",
        activation: "anyone",
      },
    },
  );
  const interactiveId = created.createInteractive.interactiveId;
  expect(created.createInteractive.available).toBe(true);

  // --- a player pulls it --------------------------------------------------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  const off = await gql<{ activateInteractive: { outcome: string } }>(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: interactiveId },
  );
  expect(off.activateInteractive.outcome).toBe("performed");

  // Everybody sees it, including the Game Master's own view — the change is
  // in the world, not in one browser.
  const afterOff = await intensities(page, sceneId);
  for (const id of lightIds) {
    expect(afterOff[id], "every named light went out").toBe(0);
  }
  const playerAfterOff = await intensities(playerPage, sceneId);
  for (const id of lightIds) {
    expect(playerAfterOff[id]).toBe(0);
  }

  // --- and pulls it again -------------------------------------------------

  await gql(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: interactiveId },
  );

  const afterOn = await intensities(page, sceneId);
  for (const [index, id] of lightIds.entries()) {
    // The assertion this story turns on: each lamp comes back to *its own*
    // brightness. A switch that restored everything to a default would pass a
    // "the lights are on again" check and quietly relight the room wrong.
    expect(
      afterOn[id],
      `light ${index} returned to the brightness the GM chose`,
    ).toBeCloseTo(chosen[index], 5);
  }

  // --- a lamp the Game Master deleted -------------------------------------

  await gql(
    page,
    `mutation ($lightId: UUID!) { deleteLightSource(lightId: $lightId) }`,
    { lightId: lightIds[2] },
  );

  const partial = await gql<{
    activateInteractive: { outcome: string; notices: string[] };
  }>(
    page,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome notices }
    }`,
    { id: interactiveId },
  );

  expect(partial.activateInteractive.outcome).toBe("performed");
  const stillThere = await intensities(page, sceneId);
  expect(stillThere[lightIds[0]], "the rest still work").toBe(0);
  expect(stillThere[lightIds[1]]).toBe(0);
  expect(
    partial.activateInteractive.notices.length,
    "the GM is told about the lamp that has gone, rather than left to guess",
  ).toBeGreaterThan(0);

  // A player is told nothing: it is a note about the authoring, and they have
  // no use for it and no way to act on it.
  await gql(
    page,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: interactiveId },
  );
  const playerAttempt = await gql<{
    activateInteractive: { notices: string[] };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { notices }
    }`,
    { id: interactiveId },
  );
  expect(playerAttempt.activateInteractive.notices).toEqual([]);

  console.log(
    `[lighting] lamps=${lightIds.length} restoredExactly=true missingReportedToGm=true playerNotices=0`,
  );

  await playerPage.context().close();
});
