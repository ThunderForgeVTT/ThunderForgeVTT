import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  additionsWithoutBook,
  removeKeptAddition,
  type WorldBookEntry,
} from "./worldBooks";

/**
 * What this table wrote beside books it has since switched off (spec 050
 * decision 5).
 *
 * Changes and hides go with a book, because they mean nothing without the
 * entries they modify. Additions stay, because they never needed it — they
 * are the table's own writing, authored and shareable (FR-052a) — and a
 * person who switched a book off must be able to find them again.
 *
 * # Why a section of its own, and not under the book
 *
 * Showing a kept addition inside a switched-off book would make the book look
 * partly on, and a player-facing list that says a table is running a book it
 * is not is worse than one more heading. So they are listed here, each naming
 * the book it was written beside. One whose book is only switched off rejoins
 * that book's page — as the same entry, not a copy — if the book is switched
 * back on. One whose book was removed from the shelf stays here for good,
 * labelled with the title the book had: a re-import is a different book.
 *
 * Only removal is offered here. Changing a kept addition happens on its
 * book's page, which is where it was written and where it will be read again.
 */
export function KeptAdditions({
  worldId,
  version,
}: {
  worldId: string;
  /** Bumped by the book list whenever a book goes on or off. */
  version: number;
}) {
  const [kept, setKept] = useState<WorldBookEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [removed, setRemoved] = useState(0);

  useEffect(() => {
    let live = true;
    additionsWithoutBook(worldId)
      .then((found) => {
        if (live) setKept(found);
      })
      .catch(() => {
        if (live) setError("This table's own additions could not be read.");
      });
    return () => {
      live = false;
    };
  }, [worldId, version, removed]);

  const remove = useCallback(
    (entry: WorldBookEntry) => {
      removeKeptAddition(worldId, entry.id)
        .then(() => setRemoved((count) => count + 1))
        .catch((cause: unknown) =>
          setError(
            cause instanceof Error
              ? cause.message
              : "That addition could not be removed.",
          ),
        );
    },
    [worldId],
  );

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }
  if (kept.length === 0) {
    return null;
  }

  return (
    <section className="grid gap-2" data-testid="kept-additions">
      <h3 className="text-sm font-semibold">Written at this table</h3>
      <p className="text-sm text-muted-foreground">
        Added beside books this world has since switched off or that have left
        the shelf. They stay because they are this table&apos;s own writing. One
        whose book is only switched off returns to that book&apos;s page if it
        is switched back on; one whose book was removed stays here.
      </p>
      <ul className="grid gap-2">
        {kept.map((entry) => (
          <li
            key={entry.id}
            className="grid gap-1 rounded-md border p-2 text-sm"
            data-testid={`kept-addition-${entry.id}`}
            data-name={entry.name}
            data-state={entry.state}
          >
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <span>
                <span className="font-medium">{entry.name}</span>{" "}
                <span className="text-muted-foreground">
                  — {entry.kind}, written beside {entry.bookTitle}
                  {entry.compendiumId === null
                    ? ", which has left the shelf"
                    : ", which is switched off"}
                </span>
              </span>
              <span
                className="text-xs text-muted-foreground"
                data-testid="world-entry-origin"
                data-origin={entry.origin}
                data-may-be-shared={entry.mayBeShared ? "true" : "false"}
              >
                {entry.mayBeShared
                  ? "Authored here — may be shared"
                  : "Uploaded — stays with this account"}
              </span>
            </div>
            {entry.proseText && (
              <p className="whitespace-pre-wrap">{entry.proseText}</p>
            )}
            <div>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => remove(entry)}
                data-testid="remove-kept-addition"
              >
                Remove
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
