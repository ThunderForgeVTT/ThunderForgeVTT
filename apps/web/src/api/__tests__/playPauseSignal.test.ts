import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
import { isPlayPaused } from "@/api/playPause";
import {
  onPlayPaused,
  rearmPlayPaused,
  reportPlayPaused,
  reportPlayPausedIn,
  resetPlayPausedForTests,
} from "@/api/playPauseSignal";

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
