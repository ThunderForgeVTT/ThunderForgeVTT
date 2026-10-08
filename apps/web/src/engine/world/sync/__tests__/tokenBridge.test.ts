import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * What the token mutation bridge knows about, and when it learned it
 * (spec 028 US7, T083).
 *
 * The bridge decides between "update this token" and "this is a token I have
 * never seen, create it" from an in-memory map, and that decision is what
 * FR-035a keys the whole offline path on: a move is queued, a creation is
 * refused. So a token missing from the map does not fail loudly — it is
 * quietly reclassified as a creation, and offline that means the edit is
 * dropped with no error anywhere. These tests pin the map to the tokens that
 * actually exist, whenever the client found out about them.
 */

const createToken = vi.fn();
const updateToken = vi.fn();
const moveOwnToken = vi.fn();
const deleteToken = vi.fn();
const getTokens = vi.fn();

vi.mock("@/api/tokens", () => ({
  createToken: (...args: unknown[]) => createToken(...args),
  updateToken: (...args: unknown[]) => updateToken(...args),
  moveOwnToken: (...args: unknown[]) => moveOwnToken(...args),
  deleteToken: (...args: unknown[]) => deleteToken(...args),
  getTokens: (...args: unknown[]) => getTokens(...args),
}));

const queueEdit = vi.fn();
const shouldQueue = vi.fn();

vi.mock("../offlineQueue", () => ({
  queueEdit: (...args: unknown[]) => queueEdit(...args),
  shouldQueue: () => shouldQueue(),
}));

const settleGroup = vi.fn();
vi.mock("../groupMoves", () => ({
  settleGroup: (...args: unknown[]) => settleGroup(...args),
}));

const toastWarning = vi.fn();
vi.mock("sonner", () => ({
  toast: { warning: (...args: unknown[]) => toastWarning(...args) },
}));

const { createWorldStore } = await import("../../store");
const { startTokenMutationBridge } = await import("../tokens");

const SCENE_ID = "11111111-1111-4111-8111-111111111111";
const WORLD_ID = "22222222-2222-4222-8222-222222222222";
const TOKEN_ID = "33333333-3333-4333-8333-333333333333";

/** Let the bridge's `getTokens` seed promise and the dispatch chain settle. */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

function bridgeOnStore(isSceneOwner = true) {
  const store = createWorldStore({ worldId: WORLD_ID });
  const stop = startTokenMutationBridge(
    store,
    SCENE_ID,
    isSceneOwner,
    WORLD_ID,
  );
  return { store, stop };
}

const tokenAt = (x: number, y: number) => ({ id: TOKEN_ID, x, y, z: 0 });

beforeEach(() => {
  vi.clearAllMocks();
  // The scene had no tokens when the bridge started — the case that matters,
  // since a token created afterwards is the one the map used to miss.
  getTokens.mockResolvedValue([]);
  shouldQueue.mockReturnValue(false);
  queueEdit.mockResolvedValue({ queued: true });
  updateToken.mockResolvedValue(undefined);
  moveOwnToken.mockResolvedValue(true);
  createToken.mockResolvedValue({ tokenId: TOKEN_ID });
});

describe("startTokenMutationBridge", () => {
  it("treats a token it has only ever seen arrive as an update, not a creation", async () => {
    const { store, stop } = bridgeOnStore();
    await settle();

    // How the client learns about a token created by the token panel, or by
    // another player: a `world_events` refetch dispatched as `sync`.
    store.dispatch({ type: "upsert_token", token: tokenAt(0, 0) }, "sync");
    store.dispatch({ type: "upsert_token", token: tokenAt(180, -120) }, "ui");
    await settle();

    expect(updateToken).toHaveBeenCalledWith(TOKEN_ID, { x: 180, y: -120 });
    expect(createToken).not.toHaveBeenCalled();
    stop();
  });

  it("queues a drag of such a token while offline, rather than reading it as a creation", async () => {
    shouldQueue.mockReturnValue(true);
    const { store, stop } = bridgeOnStore();
    await settle();

    store.dispatch({ type: "upsert_token", token: tokenAt(0, 0) }, "sync");
    store.dispatch({ type: "upsert_token", token: tokenAt(180, -120) }, "ui");
    await settle();

    expect(queueEdit).toHaveBeenCalledTimes(1);
    expect(queueEdit.mock.calls[0][0]).toMatchObject({
      worldId: WORLD_ID,
      kind: "move",
      command: {
        type: "upsert_token",
        token: { id: TOKEN_ID, x: 180, y: -120 },
      },
    });
    expect(updateToken).not.toHaveBeenCalled();
    stop();
  });

  it("still refuses to queue a genuinely new token offline (FR-035a)", async () => {
    shouldQueue.mockReturnValue(true);
    const { store, stop } = bridgeOnStore();
    await settle();

    store.dispatch({ type: "upsert_token", token: tokenAt(10, 10) }, "ui");
    await settle();

    expect(queueEdit).not.toHaveBeenCalled();
    expect(createToken).not.toHaveBeenCalled();
    stop();
  });

  it("forgets a token deleted elsewhere, so a stale id is never updated", async () => {
    const { store, stop } = bridgeOnStore();
    await settle();

    store.dispatch({ type: "upsert_token", token: tokenAt(0, 0) }, "sync");
    store.dispatch({ type: "remove_token", tokenId: TOKEN_ID }, "sync");
    store.dispatch({ type: "remove_token", tokenId: TOKEN_ID }, "ui");
    await settle();

    expect(deleteToken).not.toHaveBeenCalled();
    stop();
  });
});

describe("a group move's tokens (spec 085)", () => {
  const group = { id: "g-2", size: 2 };

  it("settles each answer to a grouped move", async () => {
    const { store, stop } = bridgeOnStore();
    await settle();
    store.dispatch({ type: "upsert_token", token: tokenAt(0, 0) }, "sync");
    store.dispatch(
      { type: "upsert_token", token: tokenAt(50, 0), group },
      "bevy",
    );
    await settle();
    expect(settleGroup).toHaveBeenCalledWith(group, true, "moved");
    stop();
  });

  it("puts a refused grouped token back from the server, with no toast of its own", async () => {
    getTokens.mockResolvedValue([{ tokenId: TOKEN_ID, x: 0, y: 0, z: 0 }]);
    moveOwnToken.mockRejectedValueOnce(new Error("A wall is in the way"));
    const { store, stop } = bridgeOnStore(false);
    await settle();
    store.dispatch(
      { type: "upsert_token", token: tokenAt(200, 0), group },
      "bevy",
    );
    await vi.waitFor(() =>
      expect(settleGroup).toHaveBeenCalledWith(group, false, "moved"),
    );
    await vi.waitFor(() =>
      expect(store.getState().tokens[TOKEN_ID]).toMatchObject({ x: 0, y: 0 }),
    );
    expect(toastWarning).not.toHaveBeenCalled();
    stop();
  });

  it("still shows the toast for a refused move that is not a group's", async () => {
    getTokens.mockResolvedValue([{ tokenId: TOKEN_ID, x: 0, y: 0, z: 0 }]);
    moveOwnToken.mockRejectedValueOnce(new Error("A wall is in the way"));
    const { store, stop } = bridgeOnStore(false);
    await settle();
    store.dispatch({ type: "upsert_token", token: tokenAt(200, 0) }, "bevy");
    await vi.waitFor(() =>
      expect(toastWarning).toHaveBeenCalledWith("A wall is in the way"),
    );
    expect(settleGroup).not.toHaveBeenCalledWith(
      expect.anything(),
      false,
      "moved",
    );
    stop();
  });

  it("settles a grouped token delete", async () => {
    deleteToken.mockResolvedValueOnce(true);
    const { store, stop } = bridgeOnStore();
    await settle();
    store.dispatch({ type: "upsert_token", token: tokenAt(0, 0) }, "sync");
    store.dispatch({ type: "remove_token", tokenId: TOKEN_ID, group }, "bevy");
    await vi.waitFor(() =>
      expect(settleGroup).toHaveBeenCalledWith(group, true, "deleted"),
    );
    stop();
  });
});
