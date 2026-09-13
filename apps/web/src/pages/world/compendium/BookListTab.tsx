import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  booksOffered,
  entriesFrom,
  switchOff,
  switchOn,
  worldBookList,
  type OfferedBook,
  type SwitchOffReport,
  type WorldBook,
  type WorldBookEntry,
} from "./worldBooks";
import type { ReadValue } from "@/engine/sdk/ReadValue";

/**
 * The books this table is running (spec 050 US3, FR-030 to FR-036, FR-042;
 * spec 049 FR-042).
 *
 * # A mod list, and who may edit it
 *
 * Everyone at the table sees the list. Its Owner, Game Masters and Trusted
 * Players change it (spec 050 decision 8), always from the **owner's** shelf
 * (FR-010a), and a player is not shown disabled buttons — there is nothing to
 * switch on or off in what they are rendered, because a control that is
 * present and inert invites a person to wonder what they did wrong.
 *
 * Arranging the list and reading a book are two flags, not one. A Trusted
 * Player is trusted with the first; browsing what a book says stays a Game
 * Master's (049 FR-042), and ADR-099 shows a Trusted Player what a Player is
 * shown wherever no spec says otherwise.
 *
 * # What a player sees
 *
 * FR-036: a book's name, the system it was read as, and how much is in it.
 * Not an entry, not a field, not a line of prose. The server does not send
 * one either — `WorldBook` has nowhere to carry content — so this component
 * could not leak it by an oversight in rendering.
 *
 * # Nothing is copied
 *
 * Switching a book on writes one row. Everything below the list is fetched
 * from the account's shelf each time it is asked for, which is why switching
 * the book off takes the content with it and why the confirmation says so
 * first.
 */
export interface BookListTabProps {
  worldId: string;
  /** Owner, Game Master or Trusted Player: may switch books on and off, and
   * browse a book's entries — nobody can change entries they cannot read. */
  managesBooks: boolean;
  /** Whether the shelf being drawn on is the viewer's own, which is only a
   * matter of wording — the books are the owner's either way. */
  isOwner: boolean;
}

function shown(value: ReadValue): string {
  return value.state === "unread" ? "not found" : value.value;
}

export function BookListTab({
  worldId,
  managesBooks,
  isOwner,
}: BookListTabProps) {
  const [books, setBooks] = useState<WorldBook[] | null>(null);
  const [offered, setOffered] = useState<OfferedBook[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [leaving, setLeaving] = useState<SwitchOffReport | null>(null);
  const [browsing, setBrowsing] = useState<string | null>(null);

  const readList = useCallback(() => {
    worldBookList(worldId)
      .then((list) => {
        setBooks(list);
        setError(null);
      })
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error
            ? cause.message
            : "This world's books could not be read.",
        ),
      );
  }, [worldId]);

  const readOffers = useCallback(() => {
    if (!managesBooks) {
      return;
    }
    booksOffered(worldId)
      .then(setOffered)
      // The list itself is unaffected by a failure here, so it is silent
      // rather than an error banner over a page that is otherwise correct.
      .catch(() => setOffered([]));
  }, [worldId, managesBooks]);

  useEffect(readList, [readList]);
  useEffect(readOffers, [readOffers]);

  const turnOn = useCallback(
    (compendiumId: string) => {
      switchOn(worldId, compendiumId)
        .then((list) => {
          setBooks(list);
          readOffers();
        })
        .catch((cause: unknown) =>
          setError(
            cause instanceof Error
              ? cause.message
              : "That book could not be switched on.",
          ),
        );
    },
    [worldId, readOffers],
  );

  const askToTurnOff = useCallback(
    (compendiumId: string) => {
      switchOff(worldId, compendiumId, false)
        .then(setLeaving)
        .catch((cause: unknown) =>
          setError(
            cause instanceof Error
              ? cause.message
              : "That book could not be switched off.",
          ),
        );
    },
    [worldId],
  );

  const confirmTurnOff = useCallback(() => {
    if (!leaving) return;
    switchOff(worldId, leaving.compendiumId, true)
      .then(() => {
        setLeaving(null);
        setBrowsing((open) => (open === leaving.compendiumId ? null : open));
        readList();
        readOffers();
      })
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error
            ? cause.message
            : "That book could not be switched off.",
        ),
      );
  }, [leaving, worldId, readList, readOffers]);

  return (
    <div className="grid gap-4" data-testid="world-book-list">
      <header className="grid gap-1">
        <h2 className="text-lg font-semibold">Books this table is running</h2>
        <p className="text-sm text-muted-foreground">
          {managesBooks
            ? `Switched on from ${isOwner ? "your library" : "the world owner's library"}. Nothing is copied into this world — the content is read from that shelf, so switching a book off takes it back out again.`
            : "What this table is running. Only its Game Masters and Trusted Players change this list."}
        </p>
      </header>

      {error && <StatusBadge variant="danger">{error}</StatusBadge>}

      {books === null && !error && (
        <p className="text-muted-foreground">Reading the book list...</p>
      )}

      {books !== null && books.length === 0 && (
        <p className="text-muted-foreground" data-testid="book-list-empty">
          {managesBooks
            ? "No books are switched on for this world yet."
            : "This table is not running any books."}
        </p>
      )}

      {books !== null && books.length > 0 && (
        <ul className="grid gap-2" data-testid="book-list">
          {books.map((book) => (
            <li
              key={book.compendiumId}
              className="grid gap-2 rounded-md border p-3"
              data-testid={`world-book-${book.compendiumId}`}
            >
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <span className="font-medium">{book.bookTitle}</span>
                <span className="text-sm text-muted-foreground">
                  {book.systemId} — {book.entryTotal} entries
                </span>
              </div>

              {/* FR-042: the world changed system underneath this book. It
                  stays on the list and says so, and it is served to nobody. */}
              {!book.systemMatches && (
                <StatusBadge variant="warning" data-testid="book-mismatched">
                  This world no longer runs {book.systemId}, so this book is not
                  being served. Switch it off, or put the world back on the
                  system it was read as.
                </StatusBadge>
              )}

              {managesBooks && (
                <div className="flex flex-wrap gap-2">
                  {book.systemMatches && (
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      data-testid="browse-book"
                      onClick={() =>
                        setBrowsing((open) =>
                          open === book.compendiumId ? null : book.compendiumId,
                        )
                      }
                    >
                      {browsing === book.compendiumId
                        ? "Close"
                        : "Browse this book"}
                    </Button>
                  )}
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    data-testid="switch-off-book"
                    onClick={() => askToTurnOff(book.compendiumId)}
                  >
                    Switch off
                  </Button>
                </div>
              )}

              {managesBooks && browsing === book.compendiumId && (
                <BookEntries
                  worldId={worldId}
                  compendiumId={book.compendiumId}
                />
              )}
            </li>
          ))}
        </ul>
      )}

      {/* FR-013 and FR-032: what goes is named before it goes. */}
      {leaving && (
        <div
          className="grid gap-2 rounded-md border border-amber-500 p-3"
          data-testid="switch-off-report"
        >
          <p className="text-sm">
            Switching <strong>{leaving.bookTitle}</strong> off takes{" "}
            {leaving.entryCount} entries out of this world. Nothing was copied
            in, so nothing is left behind — and anything at this table that is
            using them stops working.
          </p>
          {leaving.entryNames.length > 0 && (
            <p className="text-sm text-muted-foreground" data-testid="in-use">
              Including: {leaving.entryNames.join(", ")}
              {leaving.entryCount > leaving.entryNames.length && ", and more"}.
            </p>
          )}
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              data-testid="switch-off-confirm"
              onClick={confirmTurnOff}
            >
              Switch it off
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setLeaving(null)}
            >
              Keep it on
            </Button>
          </div>
        </div>
      )}

      {managesBooks && (
        <section className="grid gap-2" data-testid="books-offered">
          <h3 className="text-sm font-semibold">
            {isOwner ? "From your library" : "From the world owner's library"}
          </h3>
          {offered.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              {isOwner ? (
                <>
                  Nothing on your shelf matches this world&apos;s system that is
                  not already switched on.{" "}
                  <Link className="underline" to="/library">
                    Your library
                  </Link>
                </>
              ) : (
                "Nothing on the owner's shelf matches this world's system that is not already switched on. Only the owner can add books to it."
              )}
            </p>
          ) : (
            <ul className="grid gap-2">
              {offered.map((book) => (
                <li
                  key={book.id}
                  className="flex flex-wrap items-center justify-between gap-2 rounded-md border p-3"
                  data-testid={`offered-book-${book.id}`}
                >
                  <span>
                    {book.bookTitle}{" "}
                    <span className="text-sm text-muted-foreground">
                      — {book.entryTotal} entries
                    </span>
                  </span>
                  <Button
                    type="button"
                    size="sm"
                    data-testid="switch-on-book"
                    onClick={() => turnOn(book.id)}
                  >
                    Switch on
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

/**
 * Browse by compendium, inside the world (spec 049 FR-042).
 *
 * The thing merging the two specs bought: a Game Master reaches what a book
 * says from the table it is switched on for, rather than only from the shelf.
 * Every entry names its book and page, as it does in the library, because
 * FR-043 is a promise to whoever is reading the entry and not to a surface.
 */
function BookEntries({
  worldId,
  compendiumId,
}: {
  worldId: string;
  compendiumId: string;
}) {
  const [entries, setEntries] = useState<WorldBookEntry[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [kind, setKind] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    entriesFrom(worldId, compendiumId, { kind })
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
  }, [worldId, compendiumId, kind]);

  const more = useCallback(() => {
    if (!cursor) return;
    entriesFrom(worldId, compendiumId, { kind, after: cursor })
      .then((page) => {
        setEntries((held) => [...held, ...page.entries]);
        setCursor(page.nextCursor);
      })
      .catch(() => setError("Those entries could not be read."));
  }, [worldId, compendiumId, cursor, kind]);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  const kinds = Array.from(new Set(entries.map((entry) => entry.kind)));

  return (
    <div className="grid gap-2" data-testid="world-book-browser">
      <nav className="flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          variant={kind === null ? "primary" : "secondary"}
          onClick={() => setKind(null)}
          data-testid="world-kind-all"
        >
          Everything
        </Button>
        {kinds.map((each) => (
          <Button
            key={each}
            type="button"
            size="sm"
            variant={kind === each ? "primary" : "secondary"}
            onClick={() => setKind(each)}
            data-testid={`world-kind-${each}`}
          >
            {each}
          </Button>
        ))}
      </nav>

      <p
        className="text-sm text-muted-foreground"
        data-testid="world-entry-count"
      >
        Showing {entries.length} of {total}
      </p>

      <ul className="grid gap-2">
        {entries.map((entry) => (
          <li
            key={entry.id}
            className="rounded-md border p-2"
            data-testid={`world-entry-${entry.id}`}
            data-kind={entry.kind}
          >
            <details>
              <summary className="cursor-pointer text-sm">
                <span className="font-medium">{entry.name}</span>{" "}
                <span className="text-muted-foreground">
                  — {entry.kind}, {entry.bookTitle}, page {entry.page}
                </span>
              </summary>
              <dl className="mt-2 grid grid-cols-2 gap-1 text-sm">
                {Object.entries(entry.fieldValues).map(([field, value]) => (
                  <div key={field} className="contents">
                    <dt className="text-muted-foreground">{field}</dt>
                    <dd data-field={field} data-read-state={value.state}>
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
            </details>
          </li>
        ))}
      </ul>

      {cursor && (
        <div>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={more}
            data-testid="world-more-entries"
          >
            Show more
          </Button>
        </div>
      )}
    </div>
  );
}
