import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { ReadValue } from "@/engine/sdk/ReadValue";
import {
  syncBackPlan,
  syncBackToCollection,
  type EntryContentShown,
  type ShelfChange,
  type SyncBackOutcome,
  type SyncBackPlan,
} from "./syncBack";
import type { WorldBook } from "./worldBooks";

/**
 * Syncing this world's changes back to the collection it runs (spec 050
 * FR-100 to FR-105).
 *
 * Only a collection written on the shelf has the control. A book read in says
 * why it has none, here where the control would be (FR-101, FR-102): its
 * entries record what the book says, so a table's changes to it stay at the
 * table.
 *
 * Nothing lands before the plan is on screen and confirmed (FR-103), and what
 * it replaces is kept as the collection's previous version (FR-104).
 */
export function SyncBack({
  worldId,
  book,
  isOwner,
  onSynced,
}: {
  worldId: string;
  book: Pick<WorldBook, "compendiumId" | "bookTitle" | "origin">;
  isOwner: boolean;
  onSynced: () => void;
}) {
  const [plan, setPlan] = useState<SyncBackPlan | null>(null);
  const [outcome, setOutcome] = useState<SyncBackOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refused = (fallback: string) => (cause: unknown) =>
    setError(cause instanceof Error ? cause.message : fallback);

  const ask = useCallback(() => {
    setOutcome(null);
    syncBackPlan(worldId, book.compendiumId)
      .then((shown) => {
        setPlan(shown);
        setError(null);
      })
      .catch(refused("What a sync would change could not be read."));
  }, [worldId, book.compendiumId]);

  const confirm = useCallback(() => {
    if (!plan) return;
    syncBackToCollection(worldId, book.compendiumId, plan.stamp)
      .then((landed) => {
        setPlan(null);
        setOutcome(landed);
        setError(null);
        onSynced();
      })
      .catch(refused("The changes could not be synced."));
  }, [worldId, book.compendiumId, plan, onSynced]);

  if (book.origin === "UPLOADED") {
    return (
      <p
        className="text-sm text-muted-foreground"
        data-testid="sync-back-never"
      >
        {book.bookTitle} was read in from a book, so your changes stay in this
        world: a book&apos;s entries record what the book says, and nothing is
        written back into them.
      </p>
    );
  }

  if (!isOwner) {
    return (
      <p
        className="text-sm text-muted-foreground"
        data-testid="sync-back-owner-only"
      >
        Only the world&apos;s owner syncs its changes back to {book.bookTitle},
        because the collection is on their shelf.
      </p>
    );
  }

  return (
    <div className="grid gap-2" data-testid="sync-back">
      {error && <StatusBadge variant="danger">{error}</StatusBadge>}

      {!plan && (
        <div>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            data-testid="sync-back-ask"
            onClick={ask}
          >
            Sync this world&apos;s changes to the collection
          </Button>
        </div>
      )}

      {plan && plan.changes.length === 0 && (
        <p className="text-sm" data-testid="sync-back-nothing">
          This world has no changes to {plan.collectionTitle} that would sync.
        </p>
      )}

      {plan && plan.changes.length > 0 && (
        <div
          className="grid gap-2 rounded-md border border-amber-500 p-3"
          data-testid="sync-back-plan"
        >
          <p className="text-sm">
            <strong>{plan.collectionTitle}</strong> changes on your shelf, for
            every world running it and every world started from now on. What it
            holds now is kept as version {plan.baseVersion}, so it can be put
            back from your library. {plan.worldName} stops holding these
            changes, because the collection does.
          </p>
          <ul className="grid gap-2">
            {plan.changes.map((change) => (
              <ChangeShown
                key={`${change.kind}/${change.name}`}
                change={change}
              />
            ))}
          </ul>
          {plan.staying.length > 0 && (
            <div className="text-sm" data-testid="sync-back-staying">
              <p>
                These do not fit the collection, so they stay in this world:
              </p>
              <ul className="list-disc pl-5">
                {plan.staying.map((delta) => (
                  <li key={`${delta.kind}/${delta.name}`}>
                    {delta.kind} &ldquo;{delta.name}&rdquo; — {delta.reason}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              data-testid="sync-back-confirm"
              onClick={confirm}
            >
              Sync to version {plan.baseVersion + 1}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setPlan(null)}
            >
              Keep them in this world
            </Button>
          </div>
        </div>
      )}

      {outcome && (
        <div className="grid gap-1 text-sm" data-testid="sync-back-outcome">
          <p>
            {outcome.plan.collectionTitle} is at version {outcome.baseVersion}.
            Version {outcome.plan.baseVersion} is kept in your library.
          </p>
          {outcome.stranded.map((world) => (
            <div key={world.worldId} data-testid="sync-back-stranded">
              <p>
                {world.worldName} has changes that no longer fit, and keeps
                them:
              </p>
              <ul className="list-disc pl-5">
                {world.deltas.map((delta) => (
                  <li key={`${delta.kind}/${delta.name}`}>
                    {delta.kind} &ldquo;{delta.name}&rdquo; — {delta.reason}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function shown(value: ReadValue | undefined): string {
  if (!value) return "—";
  return value.state === "unread" ? "not found" : value.value;
}

/** The fields and prose that differ, before and after. */
function differences(
  before: EntryContentShown | null,
  after: EntryContentShown | null,
): { label: string; was: string; becomes: string }[] {
  const fields = new Set([
    ...Object.keys(before?.fieldValues ?? {}),
    ...Object.keys(after?.fieldValues ?? {}),
  ]);
  const rows = [...fields]
    .sort()
    .map((field) => ({
      label: field,
      was: before ? shown(before.fieldValues[field]) : "—",
      becomes: after ? shown(after.fieldValues[field]) : "—",
    }))
    .filter((row) => row.was !== row.becomes);
  const wasProse = before?.proseText ?? "";
  const becomesProse = after?.proseText ?? "";
  if (wasProse !== becomesProse) {
    rows.push({
      label: "text",
      was: wasProse || "—",
      becomes: becomesProse || "—",
    });
  }
  return rows;
}

function ChangeShown({ change }: { change: ShelfChange }) {
  const verb =
    change.change === "ADDED"
      ? "Added"
      : change.change === "TAKEN_OUT"
        ? "Taken out"
        : "Rewritten";
  const rows = differences(change.before, change.after);
  return (
    <li className="grid gap-1 text-sm" data-testid="sync-back-change">
      <span>
        {verb}: {change.kind} &ldquo;{change.name}&rdquo;
      </span>
      {rows.length > 0 && (
        <ul className="pl-5 text-muted-foreground">
          {rows.map((row) => (
            <li key={row.label}>
              {row.label}: {row.was} → {row.becomes}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}
