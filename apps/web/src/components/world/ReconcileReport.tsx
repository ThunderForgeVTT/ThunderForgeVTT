import { Button } from "@/components/ui/button";
import type {
  RejectionReason,
  ReconcileOutcome,
  SubmittedChange,
} from "@/engine/world/sync/reconcile";

/**
 * What became of the edits made while disconnected (spec 028 US7, T080).
 *
 * # Why `SUPERSEDED` gets its own words
 *
 * It is not an error. A Game Master's conflicting change taking precedence is
 * the specified behaviour of FR-040, working exactly as designed — and if the
 * UI reports it the way it reports a failure, the player learns that
 * reconnecting is unreliable rather than that the GM moved the token. One of
 * those is true and actionable; the other is a bug report waiting to be filed
 * against working software.
 *
 * So supersession says who won. `supersededByRole` exists on the outcome for
 * this sentence and no other reason.
 *
 * # Nothing is dismissed for the user
 *
 * The report stays until acknowledged. An offline session's work disappearing
 * behind a toast that timed out while someone was looking at the map is the
 * silent loss FR-041 prohibits, arriving by a different route.
 */

export interface ReconcileReportProps {
  /** Changes that applied cleanly. */
  applied: SubmittedChange[];
  /** Changes the server refused, with its reason. */
  rejected: { change: SubmittedChange; outcome: ReconcileOutcome }[];
  /**
   * Changes the server said nothing about. A contract violation (FR-041)
   * rather than a decision, so it is worded as work still pending rather than
   * work lost — it stays queued and will be sent again.
   */
  unanswered: SubmittedChange[];
  /** Applied changes a later Game Master reconnect overrode. */
  superseded: { change: SubmittedChange; byRole: string }[];
  /**
   * Refusals of peer-adjudicated changes this client submitted for somebody
   * else (spec 028 US7, T103, FR-062).
   *
   * A subset of `rejected`, listed separately because the sentence is a
   * different one: the reader did not make these edits, another player did,
   * and the Game Master's client submitted them because adjudication rides
   * the GM's session (`contracts/peer-protocol.md`).
   *
   * The honest limit, recorded rather than hidden: this tells the **GM** whose
   * work was refused. The player's own client has no reconcile response to
   * read — it never submitted anything — so what reaches them directly is the
   * revert, arriving as an ordinary token change. Telling them in words needs
   * a server-side channel that does not exist yet, and the GM naming it at the
   * table is what the design leans on meanwhile.
   */
  onBehalf?: { change: SubmittedChange; outcome: ReconcileOutcome }[];
  onDismiss: () => void;
}

function reasonSentence(reason: RejectionReason | null | undefined, byRole?: string | null): string {
  switch (reason) {
    case "SUPERSEDED":
      return `${byRole === "GameMaster" ? "The Game Master" : "Someone else"} changed this while you were offline, and their change takes precedence.`;
    case "PERMISSION_DENIED":
      return "You no longer have permission to make this change.";
    case "GONE_AWAY":
      return "This was deleted while you were offline, so the change no longer applies.";
    case "INVALID":
      return "This change couldn't be replayed.";
    default:
      return "This change didn't apply.";
  }
}

export function ReconcileReport({
  applied,
  rejected,
  unanswered,
  superseded,
  onBehalf = [],
  onDismiss,
}: ReconcileReportProps) {
  const nothingToSay =
    applied.length === 0 &&
    rejected.length === 0 &&
    unanswered.length === 0 &&
    superseded.length === 0;
  if (nothingToSay) return null;

  // `onBehalf` is drawn from `rejected`, so it is taken out here rather than
  // listed twice — once anonymously and once with a name, which would read as
  // twice as much having gone wrong.
  const onBehalfIds = new Set(onBehalf.map((entry) => entry.change.localId));
  const ownRejected = rejected.filter((entry) => !onBehalfIds.has(entry.change.localId));

  return (
    <section
      role="status"
      aria-live="polite"
      className="grid gap-3 rounded-md border bg-background/95 p-4 shadow-md"
      data-testid="reconcile-report"
    >
      <div className="grid gap-1">
        <h2 className="text-sm font-semibold">Your offline changes</h2>
        <p className="text-xs text-muted-foreground">
          {applied.length > 0
            ? `${applied.length} ${applied.length === 1 ? "change" : "changes"} synced.`
            : "Nothing synced."}
        </p>
      </div>

      {superseded.length > 0 && (
        <div className="grid gap-1" data-testid="reconcile-superseded">
          <h3 className="text-xs font-medium">Overridden since</h3>
          <ul className="grid gap-1">
            {superseded.map(({ change, byRole }) => (
              <li key={change.localId} className="text-xs text-muted-foreground">
                {/* The Applied → Superseded case: this synced, and was then
                    overridden when someone else reconnected. Saying so is the
                    whole of FR-041 for this path — the user was already told
                    it worked. */}
                A token you moved was changed again by{" "}
                {byRole === "GameMaster" ? "the Game Master" : "another player"} when they
                reconnected. Theirs is what everyone sees now.
              </li>
            ))}
          </ul>
        </div>
      )}

      {onBehalf.length > 0 && (
        <div className="grid gap-1" data-testid="reconcile-on-behalf">
          <h3 className="text-xs font-medium">Refused for another player</h3>
          <ul className="grid gap-1">
            {onBehalf.map(({ change, outcome }) => (
              <li
                key={change.localId}
                className="text-xs text-muted-foreground"
                data-reason={outcome.reason ?? "UNKNOWN"}
                data-originator={change.originatorUserId ?? ""}
              >
                {/* Peer adjudication is provisional and the server's decision
                    is final (FR-062). It was a player's move, adjudicated at
                    the table and submitted through this connection, and the
                    server has now refused it — so the map no longer shows it,
                    and the person it belonged to should be told. */}
                A move you adjudicated for another player was not accepted.{" "}
                {reasonSentence(outcome.reason, outcome.supersededByRole)} Their token is back
                to where the server has it.
              </li>
            ))}
          </ul>
        </div>
      )}

      {ownRejected.length > 0 && (
        <div className="grid gap-1" data-testid="reconcile-rejected">
          <h3 className="text-xs font-medium">Not applied</h3>
          <ul className="grid gap-1">
            {ownRejected.map(({ change, outcome }) => (
              <li
                key={change.localId}
                className="text-xs text-muted-foreground"
                data-reason={outcome.reason ?? "UNKNOWN"}
              >
                {reasonSentence(outcome.reason, outcome.supersededByRole)}
              </li>
            ))}
          </ul>
        </div>
      )}

      {unanswered.length > 0 && (
        <div className="grid gap-1" data-testid="reconcile-unanswered">
          <h3 className="text-xs font-medium">Still to sync</h3>
          <p className="text-xs text-muted-foreground">
            {unanswered.length} {unanswered.length === 1 ? "change is" : "changes are"} still
            waiting and will be sent again. Nothing has been lost.
          </p>
        </div>
      )}

      <div>
        <Button variant="outline" size="sm" onClick={onDismiss} data-testid="reconcile-dismiss">
          Got it
        </Button>
      </div>
    </section>
  );
}

export default ReconcileReport;
