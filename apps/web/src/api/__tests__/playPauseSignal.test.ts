import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
import { isPlayPaused } from "@/api/playPause";
import {
  onChangesNotKept,
  onPlayPaused,
  rearmPlayPaused,
  reportChangesNotKept,
  reportPlayPaused,
  reportPlayPausedIn,
  reportPlayPausedInSyncReason,
  resetPlayPausedForTests,
  takeChangesNotKept,
} from "@/api/playPauseSignal";
import { changesNotKeptSentence } from "@/pages/world/PlayPausedPage";

/**
 * Spec 051 US1 (T022, T023): a pause arrives by several roads at once, and a
 * page must hear of it once per world — and again after it has left the
 * notice.
 */

const WORLD = "5a1e2b7c-0000-4000-8000-000000000001";
const OTHER = "5a1e2b7c-0000-4000-8000-000000000002";

const pausedError = (worldId: string) => ({
  message: "Play in this world has been paused by an operator.",
  extensions: {
    code: "WORLD_PLAY_PAUSED",
    worldId,
    pausedAt: "2026-09-13T12:00:00Z",
  },
});

beforeEach(() => {
  resetPlayPausedForTests();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the pause signal", () => {
  it("announces a world once however many roads report it", () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    reportPlayPaused(WORLD);
    reportPlayPausedIn([pausedError(WORLD)]);
    reportPlayPaused(WORLD);
    reportPlayPaused(OTHER);

    expect(heard).toEqual([WORLD, OTHER]);
  });

  it("announces again once re-armed", () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    reportPlayPaused(WORLD);
    rearmPlayPaused(WORLD);
    reportPlayPaused(WORLD);

    expect(heard).toEqual([WORLD, WORLD]);
  });

  it("reaches every listener even when one unsubscribes another mid-call", () => {
    const heard: string[] = [];
    let second: (() => void) | null = null;
    onPlayPaused(() => {
      heard.push("first");
      second?.();
    });
    second = onPlayPaused(() => heard.push("second"));

    reportPlayPaused(WORLD);

    expect(heard).toEqual(["first", "second"]);
  });

  it("ignores errors that are not a pause", () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    expect(
      reportPlayPausedIn([
        { message: "nope", extensions: { code: "FORBIDDEN", worldId: WORLD } },
      ]),
    ).toBe(false);
    expect(reportPlayPausedIn(undefined)).toBe(false);
    expect(heard).toEqual([]);
  });
});

describe("the transport", () => {
  it("announces a refused request and still throws a pause the caller can name", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          new Response(
            JSON.stringify({ data: null, errors: [pausedError(WORLD)] }),
            { status: 200, headers: { "content-type": "application/json" } },
          ),
        ),
    );
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    const failure = await postGraphQL(
      `mutation Heartbeat($worldId: UUID!) { heartbeat(worldId: $worldId) }`,
      { worldId: WORLD },
    ).catch((error: unknown) => error);

    expect(failure).toBeInstanceOf(GraphQLRequestError);
    expect(isPlayPaused(failure)).toBe(true);
    expect(heard).toEqual([WORLD]);
  });

  it("does not call an ordinary failure a pause", () => {
    expect(
      isPlayPaused(new GraphQLRequestError("no", { codes: ["FORBIDDEN"] })),
    ).toBe(false);
    expect(isPlayPaused(new Error("WORLD_PLAY_PAUSED"))).toBe(false);
  });
});

/**
 * Spec 051 US2 (T039): opening a paused world. Its first request is the world
 * cache's sync plan, asked by the engine's own fetch rather than through
 * `graphqlClient`, so the refusal reaches the page only as the `reason` of a
 * degraded sync summary.
 */
describe("the sync plan's refusal", () => {
  it("is a pause when the error inside the reason carries the code", () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    const reason = `server rejected sync: ${JSON.stringify({
      ...pausedError(WORLD),
      path: ["worldSyncPlan"],
    })}`;

    expect(reportPlayPausedInSyncReason(OTHER, reason)).toBe(true);
    // The world the error names wins over the one the caller guessed.
    expect(heard).toEqual([WORLD]);
  });

  it("is not a pause for any other degraded sync", () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));

    for (const reason of [
      undefined,
      "cache unavailable on this browser",
      "sync transport failed: offline",
      'server rejected sync: {"message":"nope","extensions":{"code":"FORBIDDEN"}}',
      "server rejected sync: {not json",
      // The code in the prose, not in the error, is not evidence.
      "server rejected sync: WORLD_PLAY_PAUSED",
    ]) {
      expect(reportPlayPausedInSyncReason(WORLD, reason)).toBe(false);
    }
    expect(heard).toEqual([]);
  });
});

/**
 * Spec 051 US2 (T038): offline changes a pause refused reach the notice,
 * which usually mounts before the reconcile that counts them has answered.
 */
describe("changes not kept", () => {
  it("adds up until taken, per world, and tells listeners", () => {
    const told: string[] = [];
    onChangesNotKept((worldId) => told.push(worldId));

    reportChangesNotKept(WORLD, 1);
    reportChangesNotKept(WORLD, 2);
    reportChangesNotKept(OTHER, 0);

    expect(told).toEqual([WORLD, WORLD]);
    expect(takeChangesNotKept(OTHER)).toBe(0);
    expect(takeChangesNotKept(WORLD)).toBe(3);
    expect(takeChangesNotKept(WORLD)).toBe(0);
  });

  it("says it plainly, and says nothing for none", () => {
    expect(changesNotKeptSentence(0)).toBeNull();
    expect(changesNotKeptSentence(1)).toBe(
      "1 change you made while offline wasn't kept.",
    );
    expect(changesNotKeptSentence(4)).toBe(
      "4 changes you made while offline weren't kept.",
    );
  });
});
