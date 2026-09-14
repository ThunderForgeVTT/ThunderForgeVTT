import { expect, type Page } from "@playwright/test";
import type { GqlResult } from "./admin";
import { graphql } from "./helpers";

/**
 * Spec 051: pausing a world's play, from a test.
 *
 * Every operator helper here goes through GraphQL on a page signed in as the
 * seeded administrator (`openAdminPage` in `./admin.ts`). They are for the
 * steps a spec is *not* about. A spec about the portal itself drives
 * `/admin/play-pauses` the way an operator does, as `play-pause.spec.ts`
 * does for the pause.
 */

export interface PlayPauseRow {
  id: string;
  worldId: string;
  worldName: string;
  pausedAt: string;
  grounds: string;
  liftedAt: string | null;
}

export interface PauseRequestRow {
  id: string;
  worldId: string;
  state: "PENDING" | "APPROVED" | "DECLINED";
  triggers: {
    kind: string;
    entityId: string | null;
    caseId: string | null;
  }[];
  playedNow: boolean;
}

type GqlError = { message: string; extensions?: Record<string, unknown> };

/** A GraphQL answer whose refusal carries its extensions. */
export interface GqlAnswer<T> {
  data?: T | null;
  errors?: GqlError[];
}

const PAUSE_FIELDS = `id worldId worldName pausedAt grounds liftedAt`;

function refused(what: string, errors: unknown): Error {
  return new Error(`${what} was refused: ${JSON.stringify(errors)}`);
}

/** Pause `worldId`'s play as an operator, and return the pause. */
export async function pauseWorldAsOperator(
  adminPage: Page,
  worldId: string,
  grounds: string,
): Promise<{ pause: PlayPauseRow; alreadyPaused: boolean }> {
  const result = await pauseWorldPlayRaw(adminPage, worldId, grounds);
  if (!result.data?.pauseWorldPlay) {
    throw refused("pauseWorldPlay", result.errors ?? result);
  }
  return result.data.pauseWorldPlay;
}

/**
 * `pauseWorldPlay` as whoever `page` is signed in as, answered raw.
 *
 * Raw because the callers that matter most are asserting a refusal (FR-006).
 */
export async function pauseWorldPlayRaw(
  page: Page,
  worldId: string,
  grounds: string,
): Promise<
  GqlAnswer<{ pauseWorldPlay: { pause: PlayPauseRow; alreadyPaused: boolean } }>
> {
  return graphql(
    page,
    `
      mutation PauseWorldPlay($worldId: UUID!, $grounds: String!) {
        pauseWorldPlay(worldId: $worldId, grounds: $grounds) {
          pause { ${PAUSE_FIELDS} }
          alreadyPaused
        }
      }
    `,
    { worldId, grounds },
  );
}

/** Lift a pause as an operator. */
export async function liftPauseAsOperator(
  adminPage: Page,
  pauseId: string,
  grounds: string,
): Promise<PlayPauseRow> {
  const result = await graphql<GqlResult<{ liftWorldPlayPause: PlayPauseRow }>>(
    adminPage,
    `
      mutation LiftWorldPlayPause($pauseId: UUID!, $grounds: String!) {
        liftWorldPlayPause(pauseId: $pauseId, grounds: $grounds) {
          ${PAUSE_FIELDS}
        }
      }
    `,
    { pauseId, grounds },
  );
  if (!result.data?.liftWorldPlayPause) {
    throw refused("liftWorldPlayPause", result.errors ?? result);
  }
  return result.data.liftWorldPlayPause;
}

/** Approve or decline a pause request as an operator. */
export async function decideRequestAsOperator(
  adminPage: Page,
  requestId: string,
  decision: "APPROVE" | "DECLINE",
  note: string,
): Promise<{
  decidedHere: boolean;
  request: PauseRequestRow;
  pause: PlayPauseRow | null;
}> {
  const result = await graphql<
    GqlResult<{
      decidePlayPauseRequest: {
        decidedHere: boolean;
        request: PauseRequestRow;
        pause: PlayPauseRow | null;
      };
    }>
  >(
    adminPage,
    `
      mutation DecidePlayPauseRequest($requestId: UUID!, $decision: PauseDecision!, $note: String!) {
        decidePlayPauseRequest(requestId: $requestId, decision: $decision, note: $note) {
          decidedHere
          request { id worldId state playedNow triggers { kind entityId caseId } }
          pause { ${PAUSE_FIELDS} }
        }
      }
    `,
    { requestId, decision, note },
  );
  if (!result.data?.decidePlayPauseRequest) {
    throw refused("decidePlayPauseRequest", result.errors ?? result);
  }
  return result.data.decidePlayPauseRequest;
}

/** The pending pause request for `worldId`, or `null` when there is none. */
export async function pendingRequestFor(
  adminPage: Page,
  worldId: string,
): Promise<PauseRequestRow | null> {
  const result = await graphql<
    GqlResult<{ playPauseRequests: { nodes: PauseRequestRow[] } }>
  >(
    adminPage,
    `
      query PlayPauseRequests {
        playPauseRequests(state: PENDING, first: 200) {
          nodes {
            id
            worldId
            state
            playedNow
            triggers {
              kind
              entityId
              caseId
            }
          }
        }
      }
    `,
    {},
  );
  if (!result.data?.playPauseRequests) {
    throw refused("playPauseRequests", result.errors ?? result);
  }
  return (
    result.data.playPauseRequests.nodes.find(
      (request) => request.worldId === worldId,
    ) ?? null
  );
}

/**
 * Mark a page's document so a later check can tell whether it was reloaded.
 *
 * A reload builds a new `window`, so the mark is gone after one. Put it on
 * before the pause and `expectPausedNotice` reads it after.
 */
export async function markDocument(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as { __e2eSameDocument?: boolean }).__e2eSameDocument =
      true;
  });
}

/**
 * The notice a paused table is sent to, as the spec requires it (FR-010 to
 * FR-012, contracts/live-play-lock.md "The notice").
 *
 * - on `/world/:id/paused`, headed "Play is paused", naming the world;
 * - the heading holds focus, and the status is a polite live region;
 * - no grounds, and no word that reads as a reason or as blame;
 * - reached without a reload: the document marked by `markDocument` is still
 *   the one showing, and the browser's own navigation record says so too.
 */
export async function expectPausedNotice(
  page: Page,
  worldName: string,
  options: { grounds?: string; marked?: boolean } = {},
): Promise<void> {
  await expect(page).toHaveURL(/\/world\/[^/]+\/paused$/);
  const notice = page.getByTestId("play-paused-notice");
  const heading = notice.getByRole("heading", { name: "Play is paused" });
  await expect(heading).toBeVisible({ timeout: 15_000 });
  await expect(heading).toBeFocused();

  const status = notice.getByRole("status");
  await expect(status).toHaveAttribute("aria-live", "polite");
  await expect(status).toContainText(worldName, { timeout: 15_000 });
  await expect(status).toContainText(
    "has been paused by an operator of this instance",
  );

  if (options.grounds) {
    await expect(page.locator("body")).not.toContainText(options.grounds);
  }
  await expect(notice).not.toContainText(
    /takedown|report|violation|reason|grounds|abuse|infring/i,
  );

  if (options.marked) {
    const sameDocument = await page.evaluate(
      () =>
        (window as unknown as { __e2eSameDocument?: boolean })
          .__e2eSameDocument === true,
    );
    expect(sameDocument, "the page must not have been reloaded").toBe(true);
  }
  const navigationType = await page.evaluate(
    () =>
      (
        performance.getEntriesByType("navigation")[0] as
          | PerformanceNavigationTiming
          | undefined
      )?.type ?? "none",
  );
  expect(navigationType).not.toBe("reload");
}

const CASE_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** What the claimant picks under "Content type" on `/legal/dmca`. */
export type TakedownContentType =
  | "Scene"
  | "Actor / NPC / character"
  | "Item"
  | "Lore entry";

/**
 * File a statutorily complete takedown notice through the real `/legal/dmca`
 * form, as a claimant, and return the case reference the page gives back.
 *
 * `claimant` should be a signed-out page: the intake needs no account.
 */
export async function fileTakedown(
  claimant: Page,
  contentType: TakedownContentType,
  entityId: string,
  entityName: string,
): Promise<string> {
  await claimant.goto("/legal/dmca");
  await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible();

  await claimant.getByLabel("Content type").click();
  await claimant.getByRole("option", { name: contentType }).click();
  await claimant.locator("#dmca-entity-id").fill(entityId);
  await claimant.locator("#dmca-claimant-name").fill("Map Publisher");
  await claimant.locator("#dmca-claimant-contact").fill("rights@example.test");
  await claimant
    .locator("#dmca-work-description")
    .fill("A published battle map, registered copyright.");
  await claimant
    .locator("#dmca-infringing-location")
    .fill(`"${entityName}" in a ThunderForge world.`);
  await claimant.locator("#dmca-good-faith").click();
  await claimant.locator("#dmca-accuracy").click();
  await claimant.locator("#dmca-signature").fill("Map Publisher");

  await claimant.getByTestId("takedown-notice-submit").click();
  const accepted = claimant.getByTestId("takedown-notice-accepted");
  await expect(accepted).toBeVisible({ timeout: 15_000 });
  const caseId = (await accepted.locator("code").innerText()).trim();
  expect(caseId).toMatch(CASE_ID_PATTERN);
  return caseId;
}

/**
 * A takedown on a scene, filed through `/legal/dmca`. Moved here from
 * `dmca-scene-takedown.spec.ts` for spec 051's request e2e (T043).
 */
export async function fileSceneTakedown(
  claimant: Page,
  sceneId: string,
  sceneName: string,
): Promise<string> {
  return fileTakedown(claimant, "Scene", sceneId, sceneName);
}
