import { useCallback, useEffect, useState } from "react";
import {
  executeTermination,
  getAccountStanding,
  resolveAppeal,
  type Standing,
} from "@/api/standing";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

interface TerminationPanelProps {
  accountId: string;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/**
 * Spec 039 T080: a flagged account is a decision somebody can act on, not an
 * id in a list. Its window, its appeal in the person's own words, and the two
 * decisions only an administrator makes — deciding the appeal, and carrying
 * out a window that waits for a person.
 *
 * Deliberately no "disable" button and no strike editor: disablement is a
 * consequence of the counting, and strikes change by resolving cases.
 */
export function TerminationPanel({ accountId }: TerminationPanelProps) {
  const [standing, setStanding] = useState<Standing | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [deleted, setDeleted] = useState(false);

  const load = useCallback(() => {
    let active = true;
    getAccountStanding(accountId)
      .then((result) => {
        if (active) {
          setStanding(result);
        }
      })
      .catch((cause) => {
        if (active) {
          setError(
            cause instanceof Error ? cause.message : "Failed to load standing",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [accountId]);

  useEffect(() => load(), [load]);

  const act = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "The action failed");
    } finally {
      setBusy(false);
    }
  };

  if (deleted) {
    return (
      <p className="text-sm" data-testid="termination-deleted">
        The account was deleted. Its moderation cases and its agreements, with
        the name removed, remain.
      </p>
    );
  }
  if (!standing) {
    return error ? (
      <StatusBadge variant="danger">{error}</StatusBadge>
    ) : (
      <Loader label="Loading standing" />
    );
  }

  const openWindow = standing.termination;
  return (
    <div
      className="grid gap-2 rounded border p-3"
      data-testid="termination-panel"
    >
      <p className="text-sm">
        {standing.strikeCount} strikes counting.{" "}
        {openWindow
          ? openWindow.disablesAccount
            ? `Disabled; deletion due ${formatDate(openWindow.deletionDueAt)}${
                openWindow.requiresHuman
                  ? " and waits for an administrator"
                  : ""
              }.`
            : "This is the instance's last administrator: the window is recorded, and the account is not disabled."
          : "No window is open."}
      </p>

      {openWindow?.appealState === "open" ? (
        <div className="grid gap-2" data-testid="termination-appeal">
          <p className="text-sm font-semibold">Their appeal</p>
          <blockquote className="border-l-2 pl-3 text-sm whitespace-pre-line">
            {openWindow.appealStatement}
          </blockquote>
          <Input
            aria-label="Note to the person"
            placeholder="Note to the person (optional)"
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              disabled={busy}
              onClick={() =>
                void act(() => resolveAppeal(accountId, true, note || null))
              }
            >
              Uphold — restore the account
            </Button>
            <Button
              size="sm"
              variant="danger"
              disabled={busy}
              onClick={() =>
                void act(() => resolveAppeal(accountId, false, note || null))
              }
            >
              Reject
            </Button>
          </div>
        </div>
      ) : null}

      {openWindow?.due &&
      openWindow.requiresHuman &&
      openWindow.disablesAccount ? (
        <Button
          size="sm"
          variant="danger"
          className="justify-self-start"
          disabled={busy}
          data-testid="termination-execute"
          onClick={() =>
            void act(async () => {
              if (
                window.confirm(
                  "Delete this account permanently? This cannot be undone.",
                )
              ) {
                await executeTermination(accountId);
                setDeleted(true);
              }
            })
          }
        >
          Delete the account — the window has ended
        </Button>
      ) : null}

      {error ? <p className="text-xs text-destructive">{error}</p> : null}
    </div>
  );
}
