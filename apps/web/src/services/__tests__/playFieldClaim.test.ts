import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Spec 036 US3a, the client half.
 *
 * The subscription is mocked rather than a socket being opened: what is worth
 * testing here is how this service *reads* what the server says — that a
 * displaced window stops believing it holds the table, and that a broken
 * stream never reads as holding it — not that graphql-ws works.
 */

type Handlers = {
  next: (claim: unknown) => void;
  error: (message: string) => void;
};

const subscribe =
  vi.fn<
    (worldId: string, clientId: string, handlers: Handlers) => () => void
  >();

vi.mock("../../engine/world/sync/subscriptionClient", () => ({
  subscribeToPlayField: (
    worldId: string,
    clientId: string,
    handlers: Handlers,
  ) => subscribe(worldId, clientId, handlers),
}));

const {
  CLIENT_ID,
  claimPlayField,
  getPlayFieldState,
  resetPlayFieldStateForTests,
  subscribeToPlayFieldState,
} = await import("../playFieldClaim");

/** The handlers the service last registered. */
function handlers(): Handlers {
  const call = subscribe.mock.calls.at(-1);
  if (!call) {
    throw new Error("nothing subscribed");
  }
  return call[2];
}

describe("the play-field claim", () => {
  beforeEach(() => {
    subscribe.mockReset();
    subscribe.mockReturnValue(() => {});
    resetPlayFieldStateForTests();
  });

  it("says nothing until the server has answered", () => {
    expect(getPlayFieldState().status).toBe("unknown");
  });

  it("holds the table when the server says the claim is ours", () => {
    claimPlayField("world-1");
    handlers().next({
      clientId: CLIENT_ID,
      worldId: "world-1",
      claimedAt: "now",
      isMine: true,
    });

    expect(getPlayFieldState()).toMatchObject({
      status: "holding",
      holderClientId: CLIENT_ID,
    });
  });

  it("becomes a companion when another window takes the table", () => {
    claimPlayField("world-1");
    handlers().next({
      clientId: CLIENT_ID,
      worldId: "world-1",
      claimedAt: "now",
      isMine: true,
    });
    handlers().next({
      clientId: "another-window",
      worldId: "world-1",
      claimedAt: "later",
      isMine: false,
    });

    // The point of the whole feature: a displaced window finds out on the
    // same stream that made it the holder, rather than carrying on drawing a
    // canvas it no longer owns.
    expect(getPlayFieldState()).toMatchObject({
      status: "companion",
      holderClientId: "another-window",
    });
  });

  it("reports nobody holding it when the claim is released", () => {
    claimPlayField("world-1");
    handlers().next(null);

    expect(getPlayFieldState()).toMatchObject({
      status: "unclaimed",
      holderClientId: null,
    });
  });

  it("never reads as holding the table through a broken stream", () => {
    claimPlayField("world-1");
    handlers().next({
      clientId: CLIENT_ID,
      worldId: "world-1",
      claimedAt: "now",
      isMine: true,
    });
    handlers().error("You must be a member of this world");

    // A refusal and a dropped socket arrive identically, and neither means
    // this client is at the table. Saying "holding" through a stream that is
    // not working is the one answer that would be actively harmful.
    expect(getPlayFieldState()).toMatchObject({
      status: "unknown",
      error: "You must be a member of this world",
    });
  });

  it("releases when the caller disposes, and tells anybody watching", () => {
    const dispose = vi.fn();
    subscribe.mockReturnValue(dispose);
    const seen: string[] = [];
    subscribeToPlayFieldState((next) => seen.push(next.status));

    const release = claimPlayField("world-1");
    handlers().next({
      clientId: CLIENT_ID,
      worldId: "world-1",
      claimedAt: "now",
      isMine: true,
    });
    release();

    expect(dispose).toHaveBeenCalledTimes(1);
    expect(getPlayFieldState().status).toBe("unknown");
    expect(seen).toEqual(["unknown", "holding", "unknown"]);
  });

  it("claims with this page load's own id", () => {
    claimPlayField("world-7");
    expect(subscribe).toHaveBeenCalledWith(
      "world-7",
      CLIENT_ID,
      expect.anything(),
    );
  });
});

describe("holding at most one claim", () => {
  beforeEach(() => {
    subscribe.mockReset();
    subscribe.mockReturnValue(() => {});
    resetPlayFieldStateForTests();
  });

  it("closes the previous subscription when the table is taken back", () => {
    const firstDispose = vi.fn();
    const secondDispose = vi.fn();
    subscribe
      .mockReturnValueOnce(firstDispose)
      .mockReturnValueOnce(secondDispose);

    claimPlayField("world-1");
    // What the "take it back" control does: claim again while the first
    // subscription is still open.
    claimPlayField("world-1");

    expect(firstDispose).toHaveBeenCalledTimes(1);
    expect(secondDispose).not.toHaveBeenCalled();
  });

  it("a stale disposer cannot close a claim that replaced it", () => {
    const firstDispose = vi.fn();
    const secondDispose = vi.fn();
    subscribe
      .mockReturnValueOnce(firstDispose)
      .mockReturnValueOnce(secondDispose);

    const releaseFirst = claimPlayField("world-1");
    claimPlayField("world-1");
    releaseFirst();

    // The same identity guard the server applies to a displaced guard: only
    // whoever started the live claim may end it.
    expect(secondDispose).not.toHaveBeenCalled();
  });
});
