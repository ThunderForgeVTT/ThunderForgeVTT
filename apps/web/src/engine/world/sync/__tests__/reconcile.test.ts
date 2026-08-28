import { describe, expect, it } from "vitest";
import {
  matchOutcomes,
  parseReconciledEvent,
  pruneApplied,
  remainingAfterInterruption,
  supersededBy,
  SUPERSESSION_WINDOW_MS,
  type AppliedChange,
  type ReconcileOutcome,
  type SubmittedChange,
} from "../reconcile";

const change = (localId: string, tokenId: string): SubmittedChange => ({ localId, tokenId });
const applied = (localId: string, tokenId: string, at: number): AppliedChange => ({
  localId,
  tokenId,
  appliedAt: at,
});

describe("matchOutcomes", () => {
  it("sorts each submitted change by what the server said", () => {
    const submitted = [change("a", "t1"), change("b", "t2")];
    const outcomes: ReconcileOutcome[] = [
      { localId: "a", applied: true },
      { localId: "b", applied: false, reason: "SUPERSEDED", supersededByRole: "GameMaster" },
    ];

    const result = matchOutcomes(submitted, outcomes);

    expect(result.applied.map((c) => c.localId)).toEqual(["a"]);
    expect(result.rejected.map((r) => r.change.localId)).toEqual(["b"]);
    expect(result.unanswered).toEqual([]);
  });

  /**
   * A change the server said nothing about is not a failure — it is a contract
   * violation (FR-041), and it has to stay queued rather than be reported as
   * handled. Folding it in with rejections would tell the user their work was
   * refused when in fact nobody has decided anything about it.
   */
  it("keeps silence separate from refusal", () => {
    const submitted = [change("a", "t1"), change("ignored", "t2")];
    const outcomes: ReconcileOutcome[] = [{ localId: "a", applied: true }];

    const result = matchOutcomes(submitted, outcomes);

    expect(result.unanswered.map((c) => c.localId)).toEqual(["ignored"]);
    expect(result.rejected).toEqual([]);
  });

  it("ignores an outcome for something never submitted", () => {
    const result = matchOutcomes([change("a", "t1")], [
      { localId: "a", applied: true },
      { localId: "phantom", applied: true },
    ]);

    expect(result.applied).toHaveLength(1);
    expect(result.unanswered).toEqual([]);
  });
});

describe("parseReconciledEvent", () => {
  const event = (payload: unknown) => ({ id: 1, token_event: payload }) as never;

  it("reads a reconciled token change", () => {
    expect(
      parseReconciledEvent(
        event({ token_id: "t1", reconciled: true, by_user: "u1", by_role: "GameMaster" }),
      ),
    ).toEqual({ tokenId: "t1", byUser: "u1", byRole: "GameMaster" });
  });

  /**
   * Someone moving a token at the table is play, not an override of anybody's
   * queued work. Only a replay can supersede, so only a replay is a candidate.
   */
  it("does not treat an ordinary live edit as a replay", () => {
    expect(parseReconciledEvent(event({ token_id: "t1", action: "moved" }))).toBeNull();
  });

  it("refuses a reconciled event that cannot say who made it", () => {
    expect(parseReconciledEvent(event({ token_id: "t1", reconciled: true }))).toBeNull();
    expect(
      parseReconciledEvent(event({ reconciled: true, by_user: "u1", by_role: "Player" })),
    ).toBeNull();
  });
});

describe("supersededBy", () => {
  const mine = [applied("a", "t1", 0), applied("b", "t2", 0)];

  it("recognises someone else's replay overriding this client's change", () => {
    const hit = supersededBy(
      { tokenId: "t1", byUser: "them", byRole: "GameMaster" },
      mine,
      "me",
    );

    expect(hit.map((c) => c.localId)).toEqual(["a"]);
  });

  /**
   * This client's own reconcile emits these events too. Without the check,
   * every user would be told their work had been overridden — by themselves —
   * the moment they reconnected.
   */
  it("does not report a client superseding itself", () => {
    expect(
      supersededBy({ tokenId: "t1", byUser: "me", byRole: "Player" }, mine, "me"),
    ).toEqual([]);
  });

  it("ignores a replay touching a token this client never changed", () => {
    expect(
      supersededBy({ tokenId: "other", byUser: "them", byRole: "GameMaster" }, mine, "me"),
    ).toEqual([]);
  });
});

describe("pruneApplied", () => {
  it("forgets entries once their window has passed", () => {
    const entries = [applied("old", "t1", 0), applied("fresh", "t2", 1_000)];

    const kept = pruneApplied(entries, SUPERSESSION_WINDOW_MS + 500);

    expect(kept.map((c) => c.localId)).toEqual(["fresh"]);
  });

  it("keeps everything inside the window", () => {
    const entries = [applied("a", "t1", 0)];
    expect(pruneApplied(entries, SUPERSESSION_WINDOW_MS - 1)).toHaveLength(1);
  });
});

describe("remainingAfterInterruption", () => {
  /**
   * T082. A change leaves the outbox only once the server has spoken about it,
   * so a submission cut short by a second disconnection leaves the remainder
   * queued for next time. Re-sending an applied change is safe — one outcome
   * per submission, so a second send earns a second outcome — while dropping
   * one is not recoverable.
   */
  it("keeps whatever the interrupted call did not answer", () => {
    const submitted = [change("a", "t1"), change("b", "t2"), change("c", "t3")];
    const partial: ReconcileOutcome[] = [{ localId: "a", applied: true }];

    expect(remainingAfterInterruption(submitted, partial).map((c) => c.localId)).toEqual([
      "b",
      "c",
    ]);
  });

  it("keeps everything when the call answered nothing at all", () => {
    const submitted = [change("a", "t1")];
    expect(remainingAfterInterruption(submitted, [])).toHaveLength(1);
  });

  it("keeps nothing when every change was answered", () => {
    const submitted = [change("a", "t1"), change("b", "t2")];
    const outcomes: ReconcileOutcome[] = [
      { localId: "a", applied: true },
      { localId: "b", applied: false, reason: "GONE_AWAY" },
    ];

    expect(remainingAfterInterruption(submitted, outcomes)).toEqual([]);
  });
});
