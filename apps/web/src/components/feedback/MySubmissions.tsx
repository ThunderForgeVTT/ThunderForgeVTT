import { useCallback, useEffect, useState } from "react";
import { getMySubmissions, type MySubmission } from "@/api/feedback";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 037 US5: what this account has sent, and what became of it.
 *
 * # What a person is shown, and what they are not
 *
 * FR-018 makes the **instance** the record: a submission that reached this
 * server has arrived, whatever a destination later does with it. So delivery
 * shows as pending or delivered and **never as failed**. A person told "your
 * report failed to deliver" would reasonably file it again, producing two
 * reports of one problem and no new information — and the thing that actually
 * needs doing is the operator's, on the operator's screen.
 *
 * # Why the expiry is stated rather than implied
 *
 * The attachments a person approved are kept by this instance only for a
 * window, and that was said before they submitted. Saying it again here, with
 * the date, is what makes it true rather than a sentence in a dialog nobody
 * re-reads. Once purged the row says so plainly — a screenshot that silently
 * stopped existing would look like a bug in this screen.
 */

const KIND_LABEL: Record<string, string> = {
  ISSUE: "Problem",
  IDEA: "Idea",
  QUESTION: "Question",
};

function expiry(iso: string): { expired: boolean; when: string } {
  const at = new Date(iso);
  return { expired: at.getTime() < Date.now(), when: at.toLocaleDateString() };
}

export function MySubmissions() {
  const [rows, setRows] = useState<MySubmission[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getMySubmissions()
      .then((next) => {
        setRows(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "Your submissions could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }
  if (!rows) {
    return (
      <p className="text-muted-foreground">Reading what you have sent...</p>
    );
  }
  if (rows.length === 0) {
    return (
      <p className="text-muted-foreground" data-testid="my-submissions-empty">
        You have not sent any feedback from this account.
      </p>
    );
  }

  return (
    <ul className="grid gap-3" data-testid="my-submissions">
      {rows.map((row) => {
        const attachments = expiry(row.attachmentsExpireAt);
        return (
          <li
            key={row.id}
            className="grid gap-1.5 rounded-lg border border-border bg-secondary/40 p-4"
            data-testid={`my-submission-${row.id}`}
            data-state={row.deliveryState}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <strong>{row.summary || row.message.slice(0, 80)}</strong>
              <StatusBadge
                variant={row.deliveryState === "DELIVERED" ? "success" : "info"}
              >
                {row.deliveryState === "DELIVERED" ? "Sent on" : "Received"}
              </StatusBadge>
            </div>

            <p className="text-sm text-muted-foreground">
              {KIND_LABEL[row.kind] ?? row.kind} ·{" "}
              {new Date(row.createdAt).toLocaleString()}
            </p>

            {row.issueUrl ? (
              <p className="text-sm">
                <a
                  className="underline hover:text-foreground"
                  href={row.issueUrl}
                  target="_blank"
                  rel="noreferrer"
                  data-testid="my-submission-issue-link"
                >
                  Follow it where it was sent
                </a>
                {row.issueState ? ` · ${row.issueState.toLowerCase()}` : ""}
              </p>
            ) : null}

            {row.attachments.length ? (
              <p className="text-sm text-muted-foreground">
                {attachments.expired ? (
                  // Said plainly. A screenshot that silently stopped existing
                  // reads as a bug in this screen rather than as the retention
                  // window doing exactly what it said it would.
                  <span data-testid="my-submission-attachments-expired">
                    {row.attachments.length} attachment
                    {row.attachments.length === 1 ? "" : "s"} — this
                    instance&apos;s copies have expired and been deleted.
                    Anything already sent on is unaffected.
                  </span>
                ) : (
                  <span data-testid="my-submission-attachments">
                    {row.attachments.length} attachment
                    {row.attachments.length === 1 ? "" : "s"} — this
                    instance&apos;s copies are deleted on {attachments.when}.
                  </span>
                )}
              </p>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}
