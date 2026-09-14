import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

/**
 * Spec 051 FR-024 (found in T059): a world-event stream opened around play,
 * not in it, ends quietly on a pause instead of sending the page to the
 * notice. A stream opened for play still announces it.
 */

type Sink = {
  next: (result: { data?: unknown; errors?: unknown }) => void;
  complete: () => void;
};

const sinks: Sink[] = [];

vi.mock("graphql-ws", () => ({
  createClient: () => ({
    subscribe: (_payload: unknown, sink: Sink) => {
      sinks.push(sink);
      return () => undefined;
    },
  }),
}));

const { subscribeToWorldEvents } = await import("../subscriptionClient");
const { onPlayPaused, rearmPlayPaused } = await import("@/api/playPauseSignal");

const WORLD = "01a09eac-5c12-726a-b00b-916e9b6df732";
const PAUSED = [
  {
    message: "Play in this world is paused.",
    extensions: { code: "WORLD_PLAY_PAUSED", worldId: WORLD },
  },
];

beforeAll(() => {
  vi.stubGlobal("window", {
    location: { protocol: "http:", host: "localhost" },
  });
});

afterEach(() => {
  sinks.length = 0;
  rearmPlayPaused(WORLD);
});

async function heardAfterPause(options?: {
  announcePause?: boolean;
}): Promise<string[]> {
  const heard: string[] = [];
  const stop = onPlayPaused((worldId) => heard.push(worldId));
  const iterator = subscribeToWorldEvents(WORLD, options)[
    Symbol.asyncIterator
  ]();
  const ended = iterator.next();
  sinks.at(-1)?.next({ errors: PAUSED });
  sinks.at(-1)?.complete();
  expect((await ended).done).toBe(true);
  stop();
  return heard;
}

describe("a paused world's event stream", () => {
  it("announces the pause by default, because a stream for play is play", async () => {
    expect(await heardAfterPause()).toEqual([WORLD]);
  });

  it("ends quietly for a listener around play", async () => {
    expect(await heardAfterPause({ announcePause: false })).toEqual([]);
  });
});
