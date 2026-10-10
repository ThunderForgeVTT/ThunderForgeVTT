/**
 * Spec 048 T078 (FR-040 to FR-044): the sheets brought onto this character,
 * and the rollbacks, newest first, with who and when. The file is offered to
 * the player who brought it and the GM (`fileAvailable`, which the server
 * decides); rolling back is the GM's alone. Renders nothing until there is
 * history to show.
 */
import { useState } from "react";
import {
  rollBackActor,
  sheetFileUrl,
  type ActorImportRecord,
} from "@/api/sheetImport";
import { Button } from "@/components/ui/button/Button";
import { useSheetImport } from "@/hooks/useSheetImport";

function when(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/** What the row says happened. */
export function historyText(
  record: ActorImportRecord,
  all: ActorImportRecord[],
): string {
  const who = record.appliedBy.displayName;
  if (record.kind === "rollback") {
    const target = all.find((other) => other.id === record.restoredFrom);
    return target?.versionNo
      ? `Rolled back to before version ${target.versionNo} by ${who}`
      : `Rolled back by ${who}`;
  }
  return `Version ${record.versionNo ?? "?"} brought in by ${who}`;
}

export function HistoryRow({
  record,
  all,
  isGm,
  busy,
  confirming,
  onAsk,
  onConfirm,
  onCancel,
}: {
  record: ActorImportRecord;
  all: ActorImportRecord[];
  isGm: boolean;
  busy: boolean;
  confirming: boolean;
  onAsk: () => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <li
      className="flex flex-wrap items-center justify-between gap-2 border-b border-border py-2 last:border-b-0"
      data-testid="import-history-row"
      data-kind={record.kind}
    >
      <div className="grid gap-0.5">
        <span className="text-sm">{historyText(record, all)}</span>
        <time
          className="text-xs text-muted-foreground"
          dateTime={record.appliedAt}
        >
          {when(record.appliedAt)}
        </time>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {record.fileAvailable && record.versionId ? (
          <a
            className="text-sm underline underline-offset-2"
            href={sheetFileUrl(record.versionId)}
            download
            data-testid="import-history-download"
          >
            Download sheet
          </a>
        ) : null}
        {isGm && record.kind === "import" ? (
          confirming ? (
            <>
              <span className="text-xs text-muted-foreground">
                Hit points and other values in play are kept.
              </span>
              <Button
                size="sm"
                variant="danger"
                onClick={onConfirm}
                disabled={busy}
                data-testid="import-history-confirm-rollback"
              >
                Roll back
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={onCancel}
                disabled={busy}
              >
                Cancel
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              variant="secondary"
              onClick={onAsk}
              disabled={busy}
              data-testid="import-history-rollback"
            >
              Roll back to before this
            </Button>
          )
        ) : null}
      </div>
    </li>
  );
}

export function ImportHistory({
  actorId,
  isGm,
}: {
  actorId: string;
  isGm: boolean;
}) {
  const { imports, refetch } = useSheetImport(actorId);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (imports.length === 0) return null;

  const rollBack = async (importId: string) => {
    setBusy(true);
    setError(null);
    try {
      await rollBackActor(actorId, importId);
      setConfirming(null);
      // The page reads the character again on event 42; the list is ours.
      await refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="grid gap-2" data-testid="import-history">
      <h2 className="text-sm font-semibold tracking-widest text-muted-foreground uppercase">
        Sheet history
      </h2>
      <ul className="rounded-lg border border-border px-3">
        {imports.map((record) => (
          <HistoryRow
            key={record.id}
            record={record}
            all={imports}
            isGm={isGm}
            busy={busy}
            confirming={confirming === record.id}
            onAsk={() => setConfirming(record.id)}
            onConfirm={() => void rollBack(record.id)}
            onCancel={() => setConfirming(null)}
          />
        ))}
      </ul>
      {error ? (
        <p
          className="text-sm text-destructive"
          role="alert"
          data-testid="import-history-error"
        >
          {error}
        </p>
      ) : null}
    </section>
  );
}
