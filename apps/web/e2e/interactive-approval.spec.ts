import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, User Story 6 — a player asks, the Game Master decides.
 *
 * Two browsers, because the whole story is that two people are involved and
 * one of them is waiting.
 *
 * # The claim that needed the most care
 *
 * *Nothing expires into approval* (FR-027). There is no timeout anywhere in
 * this feature, and a test can only demonstrate an absence by waiting and
 * finding nothing changed — so that is what this does, deliberately, between
 * the asking and the deciding.
 *
 * # And the one most likely to be got wrong
 *
 * Approval runs the effect with the permission it has **now**, not the
 * permission it had when the player asked. A Game Master who locks the door
 * and then approves a queued request to open it has contradicted themselves,
 * and the lock is the more recent statement. Trusting the request's own moment
 * would make approval a way to perform something currently forbidden — which
 * is a privilege-escalation shape, arrived at by being helpful.
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

async function doorState(
  page: Page,
  sceneId: string,
  wallId: string,
): Promise<string> {
  const data = await gql<{ walls: { wallId: string; doorState: string }[] }>(
    page,
    `query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId doorState } }`,
    { sceneId },
  );
  return data.walls.find((w) => w.wallId === wallId)?.doorState ?? "missing";
}

async function queueLength(page: Page, sceneId: string): Promise<number> {
  const data = await gql<{
    pendingInteractionRequests: { requestId: string }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      pendingInteractionRequests(sceneId: $sceneId) { requestId }
    }`,
    { sceneId },
  );
  return data.pendingInteractionRequests.length;
}

test("a player asks, and nothing happens until the Game Master says so", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Approval ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const wall = await gql<{ createWall: { wallId: string } }>(
    page,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId,
        x1: 0,
        y1: 0,
        x2: 200,
        y2: 0,
        blocksVision: true,
        blocksMovement: true,
      },
    },
  );
  const wallId = wall.createWall.wallId;

  await gql(
    page,
    `mutation ($wallId: UUID!) { setDoorDesignation(wallId: $wallId, isDoor: true) }`,
    { wallId },
  );

  const found = await gql<{
    interactives: { interactiveId: string; subjectRef: string }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { interactiveId subjectRef }
    }`,
    { sceneId },
  );
  const interactiveId = found.interactives.find(
    (i) => i.subjectRef === wallId,
  )!.interactiveId;

  await gql(
    page,
    `mutation ($id: UUID!, $input: GraphQLUpdateInteractiveInput!) {
      updateInteractive(interactiveId: $id, input: $input) { interactiveId }
    }`,
    { id: interactiveId, input: { activation: "requires_approval" } },
  );

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  // --- asking ------------------------------------------------------------

  const asked = await gql<{
    activateInteractive: { outcome: string; requestId: string | null };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome requestId }
    }`,
    { id: interactiveId },
  );
  expect(asked.activateInteractive.outcome).toBe("requested");
  expect(asked.activateInteractive.requestId).toBeTruthy();
  expect(await doorState(page, sceneId, wallId)).toBe("CLOSED");

  // It reaches the Game Master, on a page they could have opened anywhere.
  expect(await queueLength(page, sceneId)).toBe(1);

  // And a player is not shown the queue: their own outcome reaches them
  // directly, and the rest is what other people asked for.
  const playerQueue = await graphql<{ errors?: { message: string }[] }>(
    playerPage,
    `
      query ($sceneId: UUID!) {
        pendingInteractionRequests(sceneId: $sceneId) {
          requestId
        }
      }
    `,
    { sceneId },
  );
  expect(playerQueue.errors?.length).toBeGreaterThan(0);

  // --- doing nothing leaves it pending -----------------------------------

  // An absence, demonstrated the only way an absence can be: by waiting and
  // finding nothing changed. There is no timeout to race, which is the point.
  await page.waitForTimeout(3_000);
  expect(
    await queueLength(page, sceneId),
    "silence is not consent (FR-027)",
  ).toBe(1);
  expect(await doorState(page, sceneId, wallId)).toBe("CLOSED");

  // --- refusing changes nothing ------------------------------------------

  await gql(
    page,
    `mutation ($id: UUID!) { refuseRequest(requestId: $id) { outcome } }`,
    { id: asked.activateInteractive.requestId },
  );
  expect(await doorState(page, sceneId, wallId)).toBe("CLOSED");
  expect(await queueLength(page, sceneId)).toBe(0);

  // A decision already made is not reopened by asking again.
  const reDecide = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($id: UUID!) {
        approveRequest(requestId: $id) {
          outcome
        }
      }
    `,
    { id: asked.activateInteractive.requestId },
  );
  expect(reDecide.errors?.length).toBeGreaterThan(0);

  // --- approving runs it -------------------------------------------------

  const second = await gql<{
    activateInteractive: { requestId: string | null };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome requestId }
    }`,
    { id: interactiveId },
  );

  const approved = await gql<{ approveRequest: { outcome: string } }>(
    page,
    `mutation ($id: UUID!) { approveRequest(requestId: $id) { outcome } }`,
    { id: second.activateInteractive.requestId },
  );
  expect(approved.approveRequest.outcome).toBe("performed");
  expect(await doorState(page, sceneId, wallId)).toBe("OPEN");

  // --- permission is re-checked at decision time --------------------------

  const third = await gql<{
    activateInteractive: { requestId: string | null };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome requestId }
    }`,
    { id: interactiveId },
  );

  // The GM changes their mind *after* the asking.
  await gql(
    page,
    `mutation ($wallId: UUID!) { setDoorLock(wallId: $wallId, locked: true) }`,
    { wallId },
  );

  const contradicted = await gql<{
    approveRequest: { outcome: string; reason: string | null };
  }>(
    page,
    `mutation ($id: UUID!) { approveRequest(requestId: $id) { outcome reason } }`,
    { id: third.activateInteractive.requestId },
  );
  expect(
    contradicted.approveRequest.outcome,
    "the lock is the more recent statement of what the GM wants",
  ).toBe("refused");
  expect(contradicted.approveRequest.reason).toBe("locked");
  expect(
    await doorState(page, sceneId, wallId),
    "and the door did not move",
  ).toBe("OPEN");

  // The request is decided either way — the GM answered, and it must not
  // linger in their queue because the world moved underneath it.
  expect(await queueLength(page, sceneId)).toBe(0);

  // --- a player cannot decide, including their own ------------------------

  const fourth = await gql<{
    activateInteractive: { requestId: string | null };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome requestId }
    }`,
    { id: interactiveId },
  );
  const selfApprove = await graphql<{ errors?: { message: string }[] }>(
    playerPage,
    `
      mutation ($id: UUID!) {
        approveRequest(requestId: $id) {
          outcome
        }
      }
    `,
    { id: fourth.activateInteractive.requestId },
  );
  expect(
    selfApprove.errors?.length,
    "approving your own request is the whole thing this gate prevents",
  ).toBeGreaterThan(0);

  console.log(
    `[approval] requested=true noExpiry=true refusedChangedNothing=true approvedRan=true lockWinsAtDecisionTime=true`,
  );

  await playerPage.context().close();
});
