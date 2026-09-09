import { useCallback, useEffect, useState } from "react";
import {
  abandonFeedbackDelivery,
  getUndeliveredFeedback,
  resumeFeedbackDelivery,
  type UndeliveredFeedback,
} from "@/api/feedback";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 037 FR-021: what this instance has not managed to send on.
 *
 * # No message, ever
 *
 * This is a diagnostic surface. It shows how many attempts, when the next one
 * is, and why the last one failed — and it deliberately does **not** show what
 * the person wrote. The server does not send it: `UndeliveredFeedback` has no
 * message field, no attachment and no host text. An operator debugging their
 * own credentials does not need to read somebody's bug report to do it, and a
 * screen that showed it would make every delivery outage a privacy question.
 *
 * # Why the failure reason is a closed vocabulary
 *
 * `lastFailureReason` is one of a fixed set, never the host's own words. A
 * host's error text is the most likely place for a token to surface, and
 * "whatever GitHub said" is not something this product can promise never
 * contains one.
 *
 * # Abandon is not delete
 *
 * Abandoning stops the retries. The submission stays — FR-018 makes this
 * instance the record, and a report that was received is received whether or
 * not it was ever forwarded. Deleting it because it could not be forwarded
 * would destroy the thing the requirement exists to protect.
 */

const REASON_LABEL: Record<string, string> = {
  NOT_CONFIGURED: "No destination is configured",
  DESTINATION_NOT_FOUND: "The destination answered 404",
  CREDENTIALS_REJECTED: "The credentials were refused",
  RATE_LIMITED: "The host is rate limiting this instance",
  ATTACHMENTS_EXPIRED: "The evidence expired before it could be sent",
  TRANSPORT: "The host could not be reached",
  HOST_ERROR: "The host failed",
};

export function UndeliveredFeedbackPanel() {
  const [rows, setRows] = useState<UndeliveredFeedback[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(() => {
    getUndeliveredFeedback()
      .then((next) => {
        setRows(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "The undelivered queue could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  const act = async (id: string, what: "abandon" | "resume") => {
    setBusyId(id);
    try {
      await (what === "abandon"
        ? abandonFeedbackDelivery(id)
        : resumeFeedbackDelivery(id));
      load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "That was refused.");
    } finally {
      setBusyId(null);
    }
  };

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }
  if (!rows) {
    return <p className="text-muted-foreground">Reading the queue...</p>;
  }

  return (
    <div className="grid gap-3" data-testid="undelivered-feedback-panel">
      <p className="text-sm text-muted-foreground">
        What this instance has not managed to send on. Everything here was
        received and is kept — sending it on is a separate thing that can fail.
        What people wrote is deliberately not shown.
      </p>

      {rows.length === 0 ? (
        <p
          className="text-muted-foreground"
          data-testid="undelivered-feedback-empty"
        >
          Nothing is waiting.
        </p>
      ) : (
        <ul className="grid gap-3">
          {rows.map((row) => (
            <li
              key={row.id}
              className="grid gap-1.5 rounded-lg border border-border bg-secondary/40 p-4 text-sm"
              data-testid={`undelivered-${row.id}`}
              data-state={row.deliveryState}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <strong>{row.kind}</strong>
                <StatusBadge
                  variant={
                    row.deliveryState === "ABANDONED" ? "warning" : "info"
                  }
                >
                  {row.deliveryState}
                </StatusBadge>
              </div>
              <p className="text-muted-foreground">
                Received {new Date(row.createdAt).toLocaleString()} ·{" "}
                {row.attemptCount} attempt
                {row.attemptCount === 1 ? "" : "s"}
                {row.nextAttemptAfter
                  ? ` · next ${new Date(row.nextAttemptAfter).toLocaleString()}`
                  : ""}
              </p>
              {row.lastFailureReason ? (
                <p data-testid="undelivered-reason">
                  {REASON_LABEL[row.lastFailureReason] ?? row.lastFailureReason}
                </p>
              ) : null}
              <p className="text-muted-foreground">
                Evidence kept until{" "}
                {new Date(row.attachmentsExpireAt).toLocaleDateString()} — after
                that this can no longer be sent on.
              </p>
              <div className="flex flex-wrap gap-2">
                {row.deliveryState === "ABANDONED" ? (
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={busyId === row.id}
                    onClick={() => void act(row.id, "resume")}
                    data-testid={`undelivered-resume-${row.id}`}
                  >
                    Try again
                  </Button>
                ) : (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={busyId === row.id}
                    onClick={() => void act(row.id, "abandon")}
                    data-testid={`undelivered-abandon-${row.id}`}
                  >
                    Stop retrying
                  </Button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
