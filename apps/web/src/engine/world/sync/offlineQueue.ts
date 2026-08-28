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
import { noteDiscrepancy } from "./discrepancies";
import {
  forgetReconciledChanges,
  queueOfflineChange,
  readQueuedChanges,
} from "@/engine/bevy";
import { isHeartbeatOffline } from "./heartbeat";
import {
  attributeCommand,
  matchOutcomes,
  noticesFor,
  readAdjudication,
  remainingAfterInterruption,
  tokensToRevert,
  type ReconcileOutcome,
  type RejectedChange,
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

/**
 * Queue a change the Game Master's client adjudicated while server-isolated
 * (spec 028 US7, T103, FR-062).
 *
 * # This is the offline outbox, not a second one
 *
 * Peer adjudication is provisional: on reconnection every adjudicated change
 * is resubmitted, re-authorized against current permissions, and may be
 * refused, with the server's decision final. That is the same sentence
 * FR-042 already writes about an offline edit, so it is the same mechanism —
 * one durable outbox, one submission path, one place where "the server has
 * spoken" is decided. A parallel queue for adjudicated changes would need its
 * own answer to interruption, its own drain, and its own conflict order, and
 * the two would drift apart in exactly the situation neither is tested in.
 *
 * The one thing adjudication adds is that the submitter may not be the
 * author. `attributeCommand` carries the author in the stored command, which
 * is the only part of the change that survives a page reload.
 */
export async function queueAdjudicatedChange(input: {
  worldId: string;
  localId: string;
  kind: OfflineEditKind;
  command: unknown;
  /** The user whose action this was. The GM's own id when they made it. */
  originatorUserId: string;
}): Promise<QueueAttempt> {
  return queueEdit({
    worldId: input.worldId,
    localId: input.localId,
    kind: input.kind,
    command: attributeCommand(input.command, input.originatorUserId),
    // Only a Game Master's client adjudicates (FR-059, no election), so a
    // change arriving here was settled with GM precedence and reconciles with
    // it. Passing anything else would let a player's client claim adjudicated
    // changes lose to their own ordinary offline edits.
    isGameMaster: true,
  });
}

/**
 * Whether an edit right now should go to the outbox rather than the wire.
 *
 * Keyed on the **heartbeat**, not the WebSocket. `graphql-ws` is lazy and
 * drops its connection when nothing is subscribed, so socket liveness answers
 * "is anything listening" rather than "can this client reach the server" —
 * and using it produced both mistakes: queueing edits during an idle moment
 * when the server was perfectly reachable, and reporting a live connection
 * while holding no socket at all, which sent offline edits into a void.
 *
 * A heartbeat either arrived or it did not.
 */
export function shouldQueue(): boolean {
  return isHeartbeatOffline();
}

/** What a reconnect did with the queue. */
export interface ReconcileReport {
  applied: SubmittedChange[];
  rejected: RejectedChange[];
  unanswered: SubmittedChange[];
  /** Still queued afterwards — the unanswered, kept for the next reconnect. */
  stillQueued: SubmittedChange[];
  /**
   * Refusals of changes this client submitted for somebody else — the
   * peer-adjudicated case (T103). A subset of `rejected`, not a separate list
   * of changes: the report has to name whose work was refused, and the GM is
   * the only one holding that name.
   */
  onBehalf: RejectedChange[];
  /**
   * Tokens whose local view was returned to the server's state because a
   * change touching them was refused (FR-062). Empty when nothing was
   * refused, and empty when no revert was wired in.
   */
  reverted: string[];
}

/** What a caller can hand the reconcile to let it clean up after a refusal. */
export interface ReconcileOptions {
  /**
   * Return the local view of these tokens to the server's state.
   *
   * A callback rather than a direct refetch here because this module knows
   * nothing about the world store or which scene is open, and should not
   * learn: the caller that owns the store already reloads it on reconnect,
   * and this is that same reload, asked for at the one moment it is
   * *required* rather than merely usual.
   */
  revert?: (tokenIds: string[]) => Promise<void> | void;
  /**
   * The signed-in user's id, for telling "my change was refused" from "a
   * change I submitted for someone else was refused". Omitted means no
   * attribution is claimed at all — naming the wrong person is worse than
   * naming nobody.
   */
  selfUserId?: string;
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
export async function reconcileWorld(
  worldId: string,
  options: ReconcileOptions = {},
): Promise<ReconcileReport | null> {
  // T104, first half: two callers race to reconcile the same world.
  //
  // `WorldPage` reconciles from both the heartbeat recovering and the live
  // socket going live, and on an ordinary reconnect both fire. Each would read
  // the same queue and submit it, so the same change would be applied twice —
  // and the second call's outcomes would name local ids the first had already
  // drained. Joining the call in flight makes the second caller a reader of
  // the first's answer, which is what it actually wanted.
  const running = inFlight.get(worldId);
  if (running) return running;

  const attempt = drainQueue(worldId, options).finally(() => {
    inFlight.delete(worldId);
  });
  inFlight.set(worldId, attempt);
  return attempt;
}

/** Reconciles in progress, one per world. See `reconcileWorld`. */
const inFlight = new Map<string, Promise<ReconcileReport | null>>();

/**
 * How many times a single reconcile will go back for newly queued work.
 *
 * Bounded because the loop's exit condition is "nothing new appeared", and a
 * client that queues faster than it submits would otherwise never leave. Three
 * passes covers the race it exists for — edits made in the moments either side
 * of the connection returning — and anything beyond that is the next
 * reconnect's business, still queued, still safe.
 */
const MAX_DRAIN_PASSES = 3;

async function drainQueue(
  worldId: string,
  options: ReconcileOptions,
): Promise<ReconcileReport | null> {
  const report: ReconcileReport = {
    applied: [],
    rejected: [],
    unanswered: [],
    stillQueued: [],
    onBehalf: [],
    reverted: [],
  };
  const submittedAlready = new Set<string>();
  let sentAnything = false;

  for (let pass = 0; pass < MAX_DRAIN_PASSES; pass += 1) {
    // T104, second half: an edit made *while* this reconcile is running.
    //
    // The connection returning does not stop the user playing. An edit made
    // between the queue being read and the outbox being drained decided to
    // queue — `shouldQueue` was true when it was asked — and lands after the
    // drain has gone past it. Submitting once would leave it sitting in the
    // outbox until the *next* disconnection, which on a stable connection is
    // never: not double-applied, but dropped, which is the worse of the two.
    // So the queue is re-read until it stops producing work this run has not
    // already sent.
    const queued = (await readQueuedChanges(worldId)).filter(
      (change) => !submittedAlready.has(change.localId),
    );
    if (queued.length === 0) break;

    const submitted: SubmittedChange[] = queued.map((change) => {
      const adjudication = readAdjudication(change.command);
      return {
        localId: change.localId,
        tokenId: tokenIdOf(change.command),
        ...(adjudication ? { originatorUserId: adjudication.originatorUserId } : {}),
      };
    });
    for (const change of queued) submittedAlready.add(change.localId);
    sentAnything = true;

    let outcomes: ReconcileOutcome[];
    try {
      outcomes = await submitQueuedChanges(
        worldId,
        queued.map((change) => {
          const adjudication = readAdjudication(change.command);
          return {
            localId: change.localId,
            command: change.command,
            // Lifted out of the command and onto the input, because that is
            // where the server reads it. Stamping it inside the command was
            // enough to carry it across a page reload — the outbox is the
            // only durable thing in the path — but the command is opaque to
            // the server, so attribution left in there is attribution the
            // role check never sees.
            ...(adjudication
              ? { attributedToUserId: adjudication.originatorUserId }
              : {}),
          };
        }),
      );
    } catch {
      // The call itself failed — a second disconnection, most likely. Nothing
      // was answered, so nothing is dropped, and this pass's changes go again
      // on the next reconnect. Deliberately not an error to the user: they
      // have already been told they are offline, and there is nothing for
      // them to do. No further passes: the connection is gone.
      report.unanswered.push(...submitted);
      report.stillQueued.push(...submitted);
      return report;
    }

    // Disclosure is remembered before anything else is done with the
    // outcomes, because it is independent of whether the change applied —
    // the server flags and applies (FR-066), so a discrepancy must survive
    // both the applied and the rejected path.
    for (const outcome of outcomes) {
      if (outcome.discrepancy) {
        noteDiscrepancy(outcome.discrepancy.recordId, outcome.discrepancy);
      }
    }

    const matched = matchOutcomes(submitted, outcomes);
    await forgetReconciledChanges(
      outcomes.map((outcome) => ({ localId: outcome.localId, applied: outcome.applied })),
    );

    report.applied.push(...matched.applied);
    report.rejected.push(...matched.rejected);
    report.unanswered.push(...matched.unanswered);
    report.stillQueued.push(...remainingAfterInterruption(submitted, outcomes));
  }

  if (!sentAnything) return null;

  if (report.rejected.length > 0) {
    // FR-062: the server refused it, so the local view must stop showing it.
    // Done before the report reaches the user, so the sentence telling them a
    // change did not stand is not read next to a map still showing it.
    const tokens = tokensToRevert(report.rejected);
    if (tokens.length > 0 && options.revert) {
      try {
        await options.revert(tokens);
        report.reverted = tokens;
      } catch {
        // The refetch failed, so the local view is still ahead of the server.
        // Reported as not reverted rather than as reverted: the next scene
        // load corrects it, and claiming a revert that did not happen would
        // make the report lie about the one thing it exists to say.
      }
    }
    if (options.selfUserId) {
      report.onBehalf = noticesFor(report.rejected, options.selfUserId).onBehalf;
    }
  }

  return report;
}
