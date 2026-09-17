import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { ReadValue } from "@/engine/sdk/ReadValue";
import { removeShelfCollectionEntry } from "@/api/shelfCollections";
import { CollectionEntryForm } from "./CollectionEntryForm";
import {
  book as loadBook,
  entriesOf,
  type LibraryBook,
  type LibraryEntry,
} from "./library";

/**
 * Everything that came out of one book, by kind (spec 049 FR-042, FR-043).
 *
 * A page at a time, because a sourcebook produces thousands of entries and a
 * screen shows a few dozen. The cursor is the server's; this never builds one.
 *
 * A book that is not this account's reads here as a book that is not there —
 * the same answer, deliberately, so that an id cannot be used to find out
 * what somebody else has on their shelf.
 */
export interface BookBrowserProps {
  compendiumId: string;
}

/** What a field reads as, when it reads as anything. */
function shown(value: ReadValue): string {
  return value.state === "unread" ? "not found" : value.value;
}

export function BookBrowser({ compendiumId }: BookBrowserProps) {
  const [book, setBook] = useState<LibraryBook | null | "missing">(null);
  const [kind, setKind] = useState<string | null>(null);
  const [entries, setEntries] = useState<LibraryEntry[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  // Moves on whenever a collection is written, so the book and its page are
  // read again from the server rather than patched here.
  const [version, setVersion] = useState(0);
  const written = useCallback(() => setVersion((held) => held + 1), []);

  useEffect(() => {
    let live = true;
    loadBook(compendiumId)
      .then((found) => {
        if (live) setBook(found ?? "missing");
      })
      .catch(() => {
        if (live) setError("That book could not be read.");
      });
    return () => {
      live = false;
    };
  }, [compendiumId, version]);

  // A change of kind starts the browse again rather than appending to what
  // was already on screen, which is the bug a shared "load more" list makes
  // easy to write.
  useEffect(() => {
    let live = true;
    entriesOf(compendiumId, { kind })
      .then((page) => {
        if (!live) return;
        setEntries(page.entries);
        setCursor(page.nextCursor);
        setTotal(page.total);
      })
      .catch(() => {
        if (live) setError("Those entries could not be read.");
      });
    return () => {
      live = false;
    };
  }, [compendiumId, kind, version]);

  const more = useCallback(() => {
    if (!cursor) return;
    entriesOf(compendiumId, { kind, after: cursor })
      .then((page) => {
        setEntries((held) => [...held, ...page.entries]);
        setCursor(page.nextCursor);
      })
      .catch(() => setError("Those entries could not be read."));
  }, [compendiumId, cursor, kind]);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }
  if (book === null) {
    return <p className="text-muted-foreground">Opening the book...</p>;
  }
  if (book === "missing") {
    return (
      <div className="grid gap-3" data-testid="book-not-yours">
        <p className="text-sm">
          There is no such book on your shelf. A library holds what one account
          has read in, and nobody else&apos;s.
        </p>
        <Link className="text-sm underline" to="/library">
          Back to your library
        </Link>
      </div>
    );
  }

  return (
    <div className="grid gap-4" data-testid="book-browser">
      <header className="grid gap-1">
        <Link className="text-sm underline" to="/library">
          Back to your library
        </Link>
        <h1 className="text-2xl font-semibold">{book.bookTitle}</h1>
        {book.origin === "AUTHORED" ? (
          <p className="text-sm text-muted-foreground">
            A collection for {book.systemId} — {book.entryTotal} entries.
          </p>
        ) : (
          <p className="text-sm text-muted-foreground">
            Read as {book.systemId} on{" "}
            {new Date(book.importedAt).toLocaleDateString()} — {book.entryTotal}{" "}
            entries from {book.pageCount} pages.
          </p>
        )}
      </header>

      {book.origin === "AUTHORED" && (
        <CollectionEntryForm
          collectionId={book.id}
          systemId={book.systemId}
          onWritten={written}
        />
      )}

      <nav className="flex flex-wrap gap-2" data-testid="kind-filter">
        <Button
          type="button"
          size="sm"
          variant={kind === null ? "primary" : "secondary"}
          onClick={() => setKind(null)}
          data-testid="kind-all"
        >
          Everything ({book.entryTotal})
        </Button>
        {book.entryCounts.map((count) => (
          <Button
            key={count.kind}
            type="button"
            size="sm"
            variant={kind === count.kind ? "primary" : "secondary"}
            onClick={() => setKind(count.kind)}
            data-testid={`kind-${count.kind}`}
          >
            {count.kind} ({count.count})
          </Button>
        ))}
      </nav>

      <p className="text-sm text-muted-foreground" data-testid="entry-count">
        Showing {entries.length} of {total}
      </p>

      <ul className="grid gap-2">
        {entries.map((entry) => (
          <li
            key={entry.id}
            className="rounded-md border p-3"
            data-testid={`entry-${entry.id}`}
            data-kind={entry.kind}
          >
            <details>
              <summary className="cursor-pointer text-sm">
                <span className="font-medium">{entry.name}</span>{" "}
                <span className="text-muted-foreground">
                  {/* FR-043: the book it came from and the page it was on,
                      on the entry itself rather than only in the header. */}
                  — {entry.kind}, {entry.bookTitle}
                  {entry.page !== null && `, page ${entry.page}`}
                </span>
                {entry.nameUncertain && (
                  <span className="ml-2 text-xs text-amber-600">
                    name uncertain
                  </span>
                )}
                {entry.suspect && (
                  <span className="ml-2 text-xs text-amber-600">
                    read from lines we did not trust
                  </span>
                )}
              </summary>
              <dl className="mt-2 grid grid-cols-2 gap-1 text-sm">
                {Object.entries(entry.fieldValues).map(([field, value]) => (
                  <div key={field} className="contents">
                    <dt className="text-muted-foreground">{field}</dt>
                    <dd
                      data-field={field}
                      data-read-state={value.state}
                      className={
                        value.state === "clear" ? "" : "text-muted-foreground"
                      }
                    >
                      {shown(value)}
                    </dd>
                  </div>
                ))}
              </dl>
              {entry.proseText && (
                <p className="mt-2 text-sm whitespace-pre-wrap">
                  {entry.proseText}
                </p>
              )}
              {book.origin === "AUTHORED" && (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="mt-2"
                  data-testid="remove-collection-entry"
                  onClick={() => {
                    removeShelfCollectionEntry(book.id, entry.id)
                      .then(written)
                      .catch(() =>
                        setError("That entry could not be removed."),
                      );
                  }}
                >
                  Take it out
                </Button>
              )}
            </details>
          </li>
        ))}
      </ul>

      {cursor && (
        <div>
          <Button
            type="button"
            variant="secondary"
            onClick={more}
            data-testid="more-entries"
          >
            Show more
          </Button>
        </div>
      )}
    </div>
  );
}
