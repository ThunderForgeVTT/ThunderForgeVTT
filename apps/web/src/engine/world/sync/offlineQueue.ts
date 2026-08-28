/**
 * The offline loop, end to end (spec 028 US7).
 *
 * Queueing lives in the engine's wasm (`outbox.rs`), the rules live in
 * `cache-core`, and the adjudication lives on the server. This is the piece
 * that joins them: it decides *when* an edit is queued rather than sent, and
 * *what happens* to the queue when the connection comes back.
 *
 * # Why an edit is queued rather than attempted
 *
 * Optimistically firing a mutation into a dead socket and queueing on failure
 * would be simpler, and wrong in a way that shows: every offline edit would
 * cost a timeout before the user saw their token move. Asking the connection
 * state first is what keeps offline play feeling like play.
 */

import { submitQueuedChanges } from "@/api/reconcile";
import {
  forgetReconciledChanges,
  queueOfflineChange,
  readQueuedChanges,
} from "@/engine/bevy";
import { isDisconnected } from "./subscriptionClient";
import {
  matchOutcomes,
  remainingAfterInterruption,
  type ReconcileOutcome,
  type SubmittedChange,
} from "./reconcile";
import { offlineEditVerdict, type OfflineEditKind } from "../facets/tokenControl";

/** What a caller learns when it tries to make an edit with no connection. */
export interface QueueAttempt {
  /** The edit was stored and may be applied locally. */
  queued: boolean;
  /** Present when it was not: a sentence to show the user. */
  explanation?: string;
}

/**
 * Queue one edit made while disconnected, if the rules allow it.
 *
 * Two gates, in this order, and the order matters. FR-035a comes first —
 * asking the outbox to store a deletion and *then* refusing it would leave an
 * entry the reconnect has to clean up. Then the write, which is awaited,
 * because an edit reported as accepted and not actually stored is the loss
 * this whole subsystem exists to prevent (FR-037).
 */
export async function queueEdit(input: {
  worldId: string;
  localId: string;
  kind: OfflineEditKind;
  command: unknown;
  isGameMaster: boolean;
}): Promise<QueueAttempt> {
  const verdict = offlineEditVerdict(input.kind);
  if (!verdict.permitted) {
    return { queued: false, explanation: verdict.explanation };
  }

  const stored = await queueOfflineChange(
    input.worldId,
    input.localId,
    input.command,
    input.isGameMaster,
  );
  if (!stored) {
    return {
      queued: false,
      explanation:
        "That change couldn't be saved on this device, so it hasn't been made. Try again once you're back online.",
    };
  }
  return { queued: true };
}

/** Whether an edit right now should go to the outbox rather than the wire. */
export function shouldQueue(): boolean {
  return isDisconnected();
}

/** What a reconnect did with the queue. */
export interface ReconcileReport {
  applied: SubmittedChange[];
  rejected: { change: SubmittedChange; outcome: ReconcileOutcome }[];
  unanswered: SubmittedChange[];
  /** Still queued afterwards — the unanswered, kept for the next reconnect. */
  stillQueued: SubmittedChange[];
}

/** The token a queued command edits, for matching outcomes back to subjects. */
function tokenIdOf(command: unknown): string {
  const token = (command as { token?: { id?: unknown } } | undefined)?.token;
  return typeof token?.id === "string" ? token.id : "";
}

/**
 * Replay everything queued for a world, and report what became of it.
 *
 * Returns `null` when there was nothing queued — the overwhelmingly common
 * case, and one that must not put a report on screen.
 *
 * # Interruption is not detected, it is survived
 *
 * A second disconnection part-way through leaves some changes unanswered.
 * Nothing here tries to notice that; instead a change is dropped from the
 * outbox only once the server has spoken about it, so whatever went
 * unanswered is simply still queued next time. Re-sending an applied change
 * is safe because the server gives exactly one outcome per submission
 * (T078), while dropping one is unrecoverable — the asymmetry is what
 * decides the design.
 */
export async function reconcileWorld(worldId: string): Promise<ReconcileReport | null> {
  const queued = await readQueuedChanges(worldId);
  if (queued.length === 0) return null;

  const submitted: SubmittedChange[] = queued.map((change) => ({
    localId: change.localId,
    tokenId: tokenIdOf(change.command),
  }));

  let outcomes: ReconcileOutcome[] = [];
  try {
    outcomes = await submitQueuedChanges(worldId, queued);
  } catch {
    // The call itself failed — a second disconnection, most likely. Nothing
    // was answered, so nothing is dropped, and the whole queue goes again on
    // the next reconnect. Deliberately not an error to the user: they have
    // already been told they are offline, and there is nothing for them to do.
    return {
      applied: [],
      rejected: [],
      unanswered: submitted,
      stillQueued: submitted,
    };
  }

  const matched = matchOutcomes(submitted, outcomes);
  await forgetReconciledChanges(
    outcomes.map((outcome) => ({ localId: outcome.localId, applied: outcome.applied })),
  );

  return {
    ...matched,
    stillQueued: remainingAfterInterruption(submitted, outcomes),
  };
}
