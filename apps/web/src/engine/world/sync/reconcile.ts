/**
 * Replaying what was changed offline, and noticing when it is later overridden
 * (spec 028 US7, T079/T082).
 *
 * The submission itself is one GraphQL call. What needs care is everything
 * around it: a reconnection can be interrupted by another disconnection, and a
 * change that *did* apply can still be taken away afterwards by a Game Master
 * who reconnects later. Both are ordinary outcomes of the specified rules, and
 * both lose someone's work if nobody is watching for them.
 *
 * # `Applied → Superseded`, the sharpest edge in the feature
 *
 * A player reconnects, their queued move applies, and the UI honestly reports
 * success. Minutes later the GM reconnects with a conflicting offline edit and
 * takes precedence (FR-040). The player is long gone from that reconcile call
 * — there is no response to put the news in — so the only way they learn is
 * the ordinary `world_events` subscription they already have.
 *
 * That is why this module remembers what it applied. Without the memory, a
 * reconciled token event is indistinguishable from someone at the table simply
 * moving a token, and the player is left believing an edit stands that does
 * not. FR-041 calls silent loss prohibited; this is the shape it would take.
 *
 * The memory is per world and deliberately short-lived — see
 * `SUPERSESSION_WINDOW_MS`.
 */

import type { WorldEventLike } from "./subscriptionClient";

/** Mirrors `RejectionReason` in `mutations_reconcile.rs`. */
export type RejectionReason =
  | "PERMISSION_DENIED"
  | "SUPERSEDED"
  | "GONE_AWAY"
  | "INVALID";

/** One change's fate, as the server reported it. */
export interface ReconcileOutcome {
  localId: string;
  applied: boolean;
  reason?: RejectionReason | null;
  /** Set when `reason` is `SUPERSEDED`: who won. */
  supersededByRole?: string | null;
  /**
   * Present only when the server independently determined this outcome and
   * got a different answer (FR-064). It is disclosure, never a refusal —
   * `applied` is decided separately and is unaffected by this field's
   * presence, which is why it sits beside that flag rather than inside it.
   */
  discrepancy?: {
    userId: string;
    recordId: string;
    reportedValue: number;
    determinedValue: number;
  } | null;
}

/** A change this client submitted, paired with what it touched. */
export interface SubmittedChange {
  localId: string;
  /** The token the command edits, for matching later supersession events. */
  tokenId: string;
  /**
   * Set only for a peer-adjudicated change made by somebody else (T103).
   * Absent means the submitter made it — the ordinary offline case.
   */
  originatorUserId?: string;
}

/**
 * How long an applied change stays eligible to be recognised as superseded.
 *
 * The `Applied → Superseded` window is "until every participant has
 * reconnected" (data-model.md), which is not a duration the client can know.
 * Twenty minutes is chosen to comfortably cover a session's worth of stragglers
 * while not making a token someone moves an hour later read as an override of
 * work the user has long since forgotten making.
 *
 * Erring long is the safer direction: the cost of a stale entry is one
 * notification that says a GM changed a token the user had also changed, which
 * is *true* and merely late. The cost of erring short is silence about lost
 * work, which is the thing FR-041 forbids.
 */
export const SUPERSESSION_WINDOW_MS = 20 * 60 * 1000;

/** What this client applied at its own reconnect, and is still watching. */
export interface AppliedChange extends SubmittedChange {
  /** When the outcome came back, for expiry only — never for conflict order. */
  appliedAt: number;
}

/**
 * Pair each submitted change with its outcome.
 *
 * Returns `unanswered` separately rather than folding it into failures,
 * because a change the server said nothing about is a different thing from one
 * it refused: a refusal is a decision the user can be told about, and silence
 * is a contract violation (FR-041) whose entries must stay queued rather than
 * be reported as handled.
 *
 * Pure, so the accounting can be tested without a server — and this is
 * precisely the accounting that must not be got wrong.
 */
export function matchOutcomes(
  submitted: SubmittedChange[],
  outcomes: ReconcileOutcome[],
): {
  applied: SubmittedChange[];
  rejected: { change: SubmittedChange; outcome: ReconcileOutcome }[];
  unanswered: SubmittedChange[];
} {
  const byLocalId = new Map(outcomes.map((outcome) => [outcome.localId, outcome]));
  const applied: SubmittedChange[] = [];
  const rejected: { change: SubmittedChange; outcome: ReconcileOutcome }[] = [];
  const unanswered: SubmittedChange[] = [];

  for (const change of submitted) {
    const outcome = byLocalId.get(change.localId);
    if (!outcome) {
      unanswered.push(change);
      continue;
    }
    if (outcome.applied) {
      applied.push(change);
    } else {
      rejected.push({ change, outcome });
    }
  }

  return { applied, rejected, unanswered };
}

/**
 * Drop entries whose window has passed.
 *
 * Exported and pure so expiry is testable without waiting twenty minutes.
 */
export function pruneApplied(
  applied: AppliedChange[],
  now: number,
  windowMs: number = SUPERSESSION_WINDOW_MS,
): AppliedChange[] {
  return applied.filter((change) => now - change.appliedAt < windowMs);
}

/** A reconciled token event, once it has been understood. */
export interface ReconciledEvent {
  tokenId: string;
  byUser: string;
  byRole: string;
}

/**
 * Read a world event as a reconciled change, or `null` if it is not one.
 *
 * Ordinary live edits carry no `reconciled` marker, so they are not candidates
 * for supersession — someone moving a token at the table is not overriding
 * anybody's queued work, it is just play.
 */
export function parseReconciledEvent(event: WorldEventLike): ReconciledEvent | null {
  const payload = ((event as { token_event?: unknown; tokenEvent?: unknown }).token_event ??
    (event as { tokenEvent?: unknown }).tokenEvent) as
    | {
        token_id?: string;
        tokenId?: string;
        reconciled?: boolean;
        by_user?: string;
        by_role?: string;
      }
    | undefined;

  if (!payload?.reconciled) return null;
  const tokenId = payload.token_id ?? payload.tokenId;
  if (!tokenId || !payload.by_user || !payload.by_role) return null;

  return { tokenId, byUser: payload.by_user, byRole: payload.by_role };
}

/**
 * Decide whether a reconciled event overrides something this client applied.
 *
 * Two conditions, and both matter. The event must touch a token this client
 * changed at its own reconnect — otherwise it is unrelated table activity. And
 * it must come from **somebody else**: this client's own reconcile emits these
 * events too, and treating them as supersession would have every user told
 * their work was overridden by themselves the moment they reconnected.
 */
export function supersededBy(
  event: ReconciledEvent,
  applied: AppliedChange[],
  selfUserId: string,
): AppliedChange[] {
  if (event.byUser === selfUserId) return [];
  return applied.filter((change) => change.tokenId === event.tokenId);
}

/**
 * Split a submission into what was sent and what still has to be.
 *
 * T082: a reconnection can be interrupted by another disconnection part-way
 * through. The rule that keeps that safe is that a change is only dropped from
 * the outbox once the server has spoken about it — so an interrupted
 * submission leaves the unanswered remainder queued, and the next reconnect
 * sends it again.
 *
 * Re-sending an already-applied change is safe by design: the server gives
 * exactly one outcome per submitted change (T078), and a second submission
 * simply earns a second outcome. Dropping one is not recoverable, so the
 * asymmetry decides the behaviour — when in doubt, keep it queued.
 */
export function remainingAfterInterruption(
  submitted: SubmittedChange[],
  outcomes: ReconcileOutcome[],
): SubmittedChange[] {
  const answered = new Set(outcomes.map((outcome) => outcome.localId));
  return submitted.filter((change) => !answered.has(change.localId));
}

// ---------------------------------------------------------------------------
// Peer adjudication (spec 028 US7, T103/T104)
// ---------------------------------------------------------------------------

/**
 * Who a peer-adjudicated change belongs to.
 *
 * While server-isolated, the Game Master's client adjudicates and — per
 * `contracts/peer-protocol.md` — the submission rides the GM's authenticated
 * session on reconnection. So the client holding the change in its outbox is
 * often *not* the person who made it, and there is no other record of who did:
 * the server sees the GM's credentials and nothing else.
 *
 * FR-062 requires the **originating** user be told when the server refuses one
 * of these, so attribution has to survive everything the change survives —
 * including a page reload, which throws away every in-memory map. The outbox
 * is the only durable store in the path, so attribution is carried inside the
 * stored command itself.
 */
export interface Adjudication {
  /** The user whose action this was, which may not be the submitter. */
  originatorUserId: string;
}

/**
 * Where attribution rides in the stored command.
 *
 * Snake case to match the rest of the command wire shape, and a key the
 * server's `UpsertTokenCommand` does not read: the command is replayed
 * verbatim through the ordinary mutation path, which parses `type` and
 * `token` and ignores everything else. So an attributed command is accepted
 * by today's server exactly as an unattributed one is, and T102's role check
 * has the field waiting for it when it lands.
 */
const ADJUDICATION_KEY = "adjudication";

/**
 * Stamp a command with the user it originated from.
 *
 * Returns the command unchanged if it is not an object — there is nothing to
 * attribute, and inventing a wrapper would change the shape the server
 * replays.
 */
export function attributeCommand(command: unknown, originatorUserId: string): unknown {
  if (typeof command !== "object" || command === null || Array.isArray(command)) {
    return command;
  }
  if (originatorUserId.length === 0) return command;
  return {
    ...(command as Record<string, unknown>),
    [ADJUDICATION_KEY]: { originator_user_id: originatorUserId },
  };
}

/**
 * Read attribution back off a stored command, or `null` when there is none.
 *
 * Deliberately strict, and `null` on anything doubtful. The value decides
 * whose name appears next to a refusal, and naming the wrong person is worse
 * than naming nobody — the same reasoning FR-067a applies to discrepancies.
 */
export function readAdjudication(command: unknown): Adjudication | null {
  if (typeof command !== "object" || command === null) return null;
  const raw = (command as Record<string, unknown>)[ADJUDICATION_KEY];
  if (typeof raw !== "object" || raw === null) return null;
  const originator = (raw as { originator_user_id?: unknown }).originator_user_id;
  if (typeof originator !== "string" || originator.length === 0) return null;
  return { originatorUserId: originator };
}

/** A change the server refused, with the reason it gave. */
export interface RejectedChange {
  change: SubmittedChange;
  outcome: ReconcileOutcome;
}

/**
 * Split refusals by who has to be told.
 *
 * `own` is the ordinary Phase 9 case — this user made the edit, this user
 * hears about it. `onBehalf` is the peer-adjudicated one: a Game Master
 * submitted somebody else's change, so the refusal is news the GM has to
 * carry, and the report names whose change it was rather than reporting an
 * anonymous failure the GM cannot place.
 *
 * A change with no attribution counts as `own`. Attribution is only ever
 * written for adjudicated changes, so its absence means the submitter made it.
 */
export function noticesFor(
  rejected: RejectedChange[],
  selfUserId: string,
): { own: RejectedChange[]; onBehalf: RejectedChange[] } {
  const own: RejectedChange[] = [];
  const onBehalf: RejectedChange[] = [];
  for (const entry of rejected) {
    const originator = entry.change.originatorUserId;
    if (!originator || originator === selfUserId) own.push(entry);
    else onBehalf.push(entry);
  }
  return { own, onBehalf };
}

/**
 * Which tokens a refusal leaves showing something the server did not agree to.
 *
 * FR-062 calls this "reverting locally", and the revert is a **re-read of the
 * server's state**, not an undo of the client's own edit. The client cannot
 * reconstruct what the token looked like before — offline play may have moved
 * it several times, and a peer-adjudicated change may have originated on
 * another machine entirely. What it can do is stop showing a position the
 * server refused, and the only authority on what to show instead is the
 * server.
 */
export function tokensToRevert(rejected: RejectedChange[]): string[] {
  const seen = new Set<string>();
  for (const entry of rejected) {
    if (entry.change.tokenId.length > 0) seen.add(entry.change.tokenId);
  }
  return [...seen];
}
