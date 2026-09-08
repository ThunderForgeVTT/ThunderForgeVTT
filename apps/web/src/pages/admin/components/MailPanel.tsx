import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  fetchMailAvailability,
  fetchMailOutbox,
  retryOutboxMessage,
  sendTestMail,
  type MailAvailability,
  type OutboxEntry,
} from "@/api/mail";

/**
 * Spec 040 US4: whether mail works, proof that it works, and what this
 * instance has failed to send.
 *
 * # There is no message body on this screen, and there cannot be
 *
 * Not for an administrator, not in a diagnostic, not for the test message
 * (FR-016). The server's outbox type has no body field and asserts its own
 * absence against the generated SDL; this panel has nothing to render even if
 * one appeared. `subject` is shown where the server sends one, which is the
 * instance's own test message alone — a subject about a person is content.
 *
 * # The test message goes through the outbox
 *
 * It is not a shortcut past the queue. It is enqueued, attempted and recorded
 * exactly like every other message, so the path this button proves is the path
 * production uses — a test that exercises a shortcut proves the shortcut. That
 * is also why a failed test leaves a row behind: the failure is findable later
 * by whoever is actually debugging it.
 *
 * # Why the settings are not edited here
 *
 * The seven `mail.*` keys are declared settings like any other, and they are
 * edited in the instance settings panel with the same source, the same
 * refusals and the same change record. Giving mail its own second editor would
 * be a second write path to the same keys.
 */

const STATE_VARIANT: Record<
  OutboxEntry["state"],
  "success" | "warning" | "danger" | "info"
> = {
  SENT: "success",
  QUEUED: "info",
  SENDING: "info",
  BLOCKED: "warning",
  FAILED: "danger",
};

export function MailPanel() {
  const [availability, setAvailability] = useState<MailAvailability | null>(
    null,
  );
  const [outbox, setOutbox] = useState<OutboxEntry[]>([]);
  const [to, setTo] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [resultOk, setResultOk] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    () =>
      Promise.all([fetchMailAvailability(), fetchMailOutbox()])
        .then(([nextAvailability, nextOutbox]) => {
          setAvailability(nextAvailability);
          setOutbox(nextOutbox);
          setError(null);
        })
        .catch((cause: unknown) => {
          setError(
            cause instanceof Error
              ? cause.message
              : "The mail configuration could not be read.",
          );
        }),
    [],
  );

  useEffect(() => {
    void load();
    // Availability and the outbox are read together because a test message
    // changes both: the row it leaves behind, and — for the first message a
    // freshly configured instance sends — whether mail is reported ready.
  }, [load]);

  const handleTest = async () => {
    setBusy(true);
    setResult(null);
    try {
      const outcome = await sendTestMail(to);
      setResultOk(outcome.delivered);
      setResult(
        outcome.delivered
          ? `Delivered to ${to}. If it does not arrive, the fault is past this instance.`
          : (outcome.reason ??
              "It was not delivered, and the server gave no reason."),
      );
      await load();
    } catch (caught) {
      setResultOk(false);
      setResult(
        caught instanceof Error
          ? caught.message
          : "The test message could not be sent.",
      );
    } finally {
      setBusy(false);
    }
  };

  const handleRetry = async (id: string) => {
    setBusy(true);
    try {
      const updated = await retryOutboxMessage(id);
      setOutbox((current) =>
        current.map((entry) => (entry.id === id ? updated : entry)),
      );
    } catch (caught) {
      setError(
        caught instanceof Error ? caught.message : "The retry was refused.",
      );
    } finally {
      setBusy(false);
    }
  };

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  if (!availability) {
    return <p className="text-muted-foreground">Reading mail settings...</p>;
  }

  return (
    <div className="grid gap-4" data-testid="mail-panel">
      <div
        data-testid="mail-availability"
        data-ready={availability.ready ? "true" : "false"}
      >
        <StatusBadge variant={availability.ready ? "success" : "warning"}>
          {availability.ready
            ? "This instance can send mail."
            : "This instance cannot send mail yet."}
        </StatusBadge>
      </div>

      {availability.missing.length ? (
        <div className="grid gap-1 rounded-lg border border-border p-4">
          <p className="font-semibold">Set these first</p>
          <ul className="grid gap-1 text-sm text-muted-foreground">
            {availability.missing.map((key) => (
              <li key={key} data-testid={`mail-missing-${key}`}>
                {key}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {availability.limitedFeatures.length ? (
        <div className="grid gap-1 rounded-lg border border-border p-4">
          <p className="font-semibold">What that costs, until it is set</p>
          <ul className="grid gap-1 text-sm text-muted-foreground">
            {availability.limitedFeatures.map((limit) => (
              <li key={limit}>{limit}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className="grid gap-2 rounded-lg border border-border bg-secondary/40 p-4">
        <Field
          label="Send a test message to"
          htmlFor="mail-test-to"
          hint="It goes through the outbox like everything else, so whatever happens is recorded below."
        >
          <Input
            id="mail-test-to"
            data-testid="mail-test-to"
            type="email"
            autoComplete="off"
            value={to}
            onChange={(event) => setTo(event.target.value)}
          />
        </Field>
        <div>
          <Button
            type="button"
            variant="secondary"
            icon="quill"
            disabled={busy || !to.trim()}
            onClick={() => void handleTest()}
            data-testid="mail-test-send"
          >
            {busy ? "Sending..." : "Send test message"}
          </Button>
        </div>
        {result ? (
          <div
            data-testid="mail-test-result"
            data-delivered={resultOk ? "true" : "false"}
          >
            <StatusBadge variant={resultOk ? "success" : "danger"}>
              {result}
            </StatusBadge>
          </div>
        ) : null}
      </div>

      <div className="grid gap-2">
        <h4 className="text-sm font-semibold tracking-wider text-muted-foreground uppercase">
          Outbox
        </h4>
        {outbox.length === 0 ? (
          <p className="text-muted-foreground">
            This instance has not tried to send anything.
          </p>
        ) : (
          outbox.map((entry) => (
            <div
              key={entry.id}
              className="grid gap-1 rounded-lg border border-border p-3 text-sm"
              data-testid={`outbox-entry-${entry.id}`}
              data-state={entry.state}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-semibold">{entry.toAddress}</span>
                <StatusBadge variant={STATE_VARIANT[entry.state]}>
                  {entry.state}
                </StatusBadge>
              </div>
              <p className="text-muted-foreground">
                {entry.purpose}
                {entry.subject ? ` · ${entry.subject}` : ""} ·{" "}
                {new Date(entry.createdAt).toLocaleString()} · attempts{" "}
                {entry.attempts}
              </p>
              {entry.lastFailureReason ? (
                <p className="text-destructive">{entry.lastFailureReason}</p>
              ) : null}
              {entry.state === "FAILED" || entry.state === "BLOCKED" ? (
                <div>
                  <Button
                    type="button"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => void handleRetry(entry.id)}
                    data-testid={`outbox-retry-${entry.id}`}
                  >
                    Try again now
                  </Button>
                </div>
              ) : null}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
