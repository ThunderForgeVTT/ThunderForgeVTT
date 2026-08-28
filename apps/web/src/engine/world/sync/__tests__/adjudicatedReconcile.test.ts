import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Resubmitting peer-adjudicated changes when the server comes back
 * (spec 028 US7, T103/T104).
 *
 * Peer adjudication is provisional: the server re-authorizes every change and
 * its decision is final (FR-062). That is the same sentence the offline
 * outbox already lives by, so this exercises the *same* drain — what is new is
 * that the submitter may not be the author, that a refusal has to take the
 * change back off the map, and that the server returning mid-adjudication
 * must neither apply a change twice nor drop one.
 */

const submitQueuedChanges = vi.fn();
vi.mock("@/api/reconcile", () => ({
  submitQueuedChanges: (...args: unknown[]) => submitQueuedChanges(...args),
}));

const readQueuedChanges = vi.fn();
const queueOfflineChange = vi.fn();
const forgetReconciledChanges = vi.fn();
vi.mock("@/engine/bevy", () => ({
  readQueuedChanges: (...args: unknown[]) => readQueuedChanges(...args),
  queueOfflineChange: (...args: unknown[]) => queueOfflineChange(...args),
  forgetReconciledChanges: (...args: unknown[]) => forgetReconciledChanges(...args),
}));

vi.mock("../heartbeat", () => ({ isHeartbeatOffline: () => true }));

const { queueAdjudicatedChange, reconcileWorld } = await import("../offlineQueue");
const { readAdjudication } = await import("../reconcile");

const WORLD = "11111111-1111-4111-8111-111111111111";
const TOKEN_A = "22222222-2222-4222-8222-222222222222";
const TOKEN_B = "33333333-3333-4333-8333-333333333333";
const GM = "gm-user";
const PLAYER = "player-user";

const move = (tokenId: string) => ({ type: "upsert_token", token: { id: tokenId, x: 1, y: 2 } });
const queued = (localId: string, tokenId: string, command: unknown = move(tokenId)) => ({
  localId,
  command,
});

beforeEach(() => {
  vi.clearAllMocks();
  forgetReconciledChanges.mockResolvedValue(0);
  queueOfflineChange.mockResolvedValue(true);
});

describe("queueAdjudicatedChange", () => {
  /**
   * The point of T103: adjudicated changes go into the outbox everything else
   * goes into. A second queue would need its own answer to interruption and
   * its own drain, and the two would disagree in the one situation neither is
   * tested in.
   */
  it("stores an adjudicated change in the offline outbox, not a queue of its own", async () => {
    const attempt = await queueAdjudicatedChange({
      worldId: WORLD,
      localId: "a",
      kind: "move",
      command: move(TOKEN_A),
      originatorUserId: PLAYER,
    });

    expect(attempt.queued).toBe(true);
    expect(queueOfflineChange).toHaveBeenCalledTimes(1);
    const [worldId, localId, command, isGameMaster] = queueOfflineChange.mock.calls[0];
    expect(worldId).toBe(WORLD);
    expect(localId).toBe("a");
    expect(isGameMaster).toBe(true);
    expect(readAdjudication(command)).toEqual({ originatorUserId: PLAYER });
  });

  /**
   * Attribution is the only record of who made the change — the server sees
   * the GM's credentials and nothing else — so it has to survive a page
   * reload, which means living in the stored command rather than in a map.
   */
  it("leaves the command replayable, with attribution alongside rather than wrapping it", async () => {
    await queueAdjudicatedChange({
      worldId: WORLD,
      localId: "a",
      kind: "move",
      command: move(TOKEN_A),
      originatorUserId: PLAYER,
    });

    const command = queueOfflineChange.mock.calls[0][2] as Record<string, unknown>;
    expect(command.type).toBe("upsert_token");
    expect(command.token).toEqual({ id: TOKEN_A, x: 1, y: 2 });
  });

  /** FR-035a is not relaxed by adjudication: a creation is still refused. */
  it("refuses a kind that may not be made away from the server at all", async () => {
    const attempt = await queueAdjudicatedChange({
      worldId: WORLD,
      localId: "a",
      kind: "create",
      command: move(TOKEN_A),
      originatorUserId: PLAYER,
    });

    expect(attempt.queued).toBe(false);
    expect(queueOfflineChange).not.toHaveBeenCalled();
  });
});

describe("reconcileWorld", () => {
  it("reports nothing at all when the outbox is empty", async () => {
    readQueuedChanges.mockResolvedValue([]);
    expect(await reconcileWorld(WORLD)).toBeNull();
  });

  /**
   * FR-062. The server's decision is final, so a refused change stops being
   * shown — and the client cannot reconstruct what to show instead, because
   * offline play may have moved the token repeatedly and an adjudicated change
   * may have originated on another machine. The revert is a re-read of the
   * server's state, asked for once per token.
   */
  it("returns the refused change's token to the server's state", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A), queued("b", TOKEN_A)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([
      { localId: "a", applied: false, reason: "PERMISSION_DENIED" },
      { localId: "b", applied: true },
    ]);
    const revert = vi.fn().mockResolvedValue(undefined);

    const report = await reconcileWorld(WORLD, { revert });

    expect(revert).toHaveBeenCalledWith([TOKEN_A]);
    expect(report?.reverted).toEqual([TOKEN_A]);
  });

  /**
   * A revert that failed leaves the map still showing what the server refused.
   * Reporting it as reverted anyway would make the report lie about the single
   * thing it exists to say.
   */
  it("does not claim a revert that did not happen", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([
      { localId: "a", applied: false, reason: "GONE_AWAY" },
    ]);

    const report = await reconcileWorld(WORLD, {
      revert: () => Promise.reject(new Error("offline again")),
    });

    expect(report?.reverted).toEqual([]);
    expect(report?.rejected).toHaveLength(1);
  });

  /**
   * Nothing is reverted when nothing was refused — an applied change is the
   * overwhelmingly common case, and refetching the scene after every ordinary
   * reconnect would undo the point of holding a cache.
   */
  it("leaves the map alone when the server accepted everything", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([{ localId: "a", applied: true }]);
    const revert = vi.fn();

    await reconcileWorld(WORLD, { revert });

    expect(revert).not.toHaveBeenCalled();
  });

  /**
   * FR-062's "inform the originator": the Game Master submitted somebody
   * else's move, so the refusal is news they have to carry, and the report has
   * to name whose work it was rather than report an anonymous failure they
   * cannot place.
   */
  it("separates a refusal of somebody else's change from a refusal of your own", async () => {
    const theirs = queued("a", TOKEN_A, {
      ...move(TOKEN_A),
      adjudication: { originator_user_id: PLAYER },
    });
    readQueuedChanges.mockResolvedValueOnce([theirs, queued("b", TOKEN_B)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([
      { localId: "a", applied: false, reason: "PERMISSION_DENIED" },
      { localId: "b", applied: false, reason: "INVALID" },
    ]);

    const report = await reconcileWorld(WORLD, { selfUserId: GM });

    expect(report?.onBehalf.map((entry) => entry.change.localId)).toEqual(["a"]);
    expect(report?.rejected).toHaveLength(2);
  });

  /**
   * T104. `WorldPage` reconciles both when the heartbeat recovers and when the
   * socket goes live, and an ordinary reconnect fires both. Submitting the same
   * queue twice applies every change twice, and the second call's outcomes name
   * local ids the first already drained.
   */
  it("does not submit the same queue twice when two callers reconnect at once", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A)]);
    readQueuedChanges.mockResolvedValue([]);
    let release: (value: unknown) => void = () => {};
    submitQueuedChanges.mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );

    const first = reconcileWorld(WORLD);
    const second = reconcileWorld(WORLD);
    release([{ localId: "a", applied: true }]);
    const [reportA, reportB] = await Promise.all([first, second]);

    expect(submitQueuedChanges).toHaveBeenCalledTimes(1);
    expect(reportA).toBe(reportB);
  });

  /**
   * T104, the other half, and the one that loses work rather than duplicating
   * it. The connection returning does not stop the user playing: an edit made
   * between the queue being read and the outbox being drained lands *after*
   * the drain went past it. Submitted once, it would sit in the outbox until
   * the next disconnection — which on a stable connection is never.
   */
  it("picks up a change queued while the submission was in flight", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A)]);
    readQueuedChanges.mockResolvedValueOnce([queued("b", TOKEN_B)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges
      .mockResolvedValueOnce([{ localId: "a", applied: true }])
      .mockResolvedValueOnce([{ localId: "b", applied: true }]);

    const report = await reconcileWorld(WORLD);

    expect(submitQueuedChanges).toHaveBeenCalledTimes(2);
    expect(report?.applied.map((change) => change.localId)).toEqual(["a", "b"]);
  });

  /**
   * The drain must not re-send what it has already sent. The outbox is drained
   * asynchronously, so a change can still be sitting there on the next read —
   * sending it again inside one reconcile would apply it twice for no reason.
   */
  it("never sends the same change twice within one reconcile", async () => {
    readQueuedChanges.mockResolvedValue([queued("a", TOKEN_A)]);
    submitQueuedChanges.mockResolvedValue([{ localId: "a", applied: true }]);

    await reconcileWorld(WORLD);

    expect(submitQueuedChanges).toHaveBeenCalledTimes(1);
  });

  /**
   * Silence is not refusal. A change the server did not answer for stays
   * queued and is not reported to the user as handled (FR-041) — the same rule
   * that makes an interrupted submission survivable rather than detectable.
   */
  it("keeps an unanswered change queued rather than reporting it as decided", async () => {
    readQueuedChanges.mockResolvedValueOnce([queued("a", TOKEN_A), queued("b", TOKEN_B)]);
    readQueuedChanges.mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([{ localId: "a", applied: true }]);

    const report = await reconcileWorld(WORLD);

    expect(report?.unanswered.map((change) => change.localId)).toEqual(["b"]);
    expect(report?.stillQueued.map((change) => change.localId)).toEqual(["b"]);
    expect(report?.rejected).toEqual([]);
  });

  /**
   * A second disconnection part-way through is not detected, it is survived:
   * nothing was answered, so nothing is dropped and nothing is forgotten from
   * the outbox.
   */
  it("drops nothing when the submission itself fails", async () => {
    readQueuedChanges.mockResolvedValue([queued("a", TOKEN_A)]);
    submitQueuedChanges.mockRejectedValue(new Error("socket closed"));

    const report = await reconcileWorld(WORLD);

    expect(report?.stillQueued.map((change) => change.localId)).toEqual(["a"]);
    expect(forgetReconciledChanges).not.toHaveBeenCalled();
    // And it stops rather than looping against a connection that is gone.
    expect(submitQueuedChanges).toHaveBeenCalledTimes(1);
  });
});
