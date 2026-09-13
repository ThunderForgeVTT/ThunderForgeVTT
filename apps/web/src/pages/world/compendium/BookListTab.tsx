import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  addEntry,
  booksOffered,
  changeEntry,
  entriesFrom,
  hideEntry,
  restoreEntry,
  switchOff,
  switchOn,
  worldBookList,
  type EntryAddress,
  type OfferedBook,
  type SwitchOffReport,
  type UnattachedDelta,
  type WorldBook,
  type WorldBookEntry,
} from "./worldBooks";
import { KeptAdditions } from "./KeptAdditions";
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
  // Bumped whenever a book goes on or off, so what the table wrote beside a
  // book is re-read at the moment it may have been left without one.
  const [listVersion, setListVersion] = useState(0);

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
          setListVersion((seen) => seen + 1);
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
        setListVersion((seen) => seen + 1);
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
          {/* 050 FR-013: this world's changes and hides go with the book, so
              each is named, uncapped. */}
          {leaving.deltas.length > 0 && (
            <div className="text-sm" data-testid="deltas-lost">
              <p>This world&apos;s changes to it go too:</p>
              <ul className="list-disc pl-5">
                {leaving.deltas.map((delta) => (
                  <li key={delta}>{delta}</li>
                ))}
              </ul>
            </div>
          )}
          {/* Decision 5: what the table added beside the book stays, and the
              person switching it off is told so before they decide. */}
          {leaving.additionsKept.length > 0 && (
            <div className="text-sm" data-testid="additions-kept">
              <p>What this table added beside it stays, as its own writing:</p>
              <ul className="list-disc pl-5">
                {leaving.additionsKept.map((addition) => (
                  <li key={addition}>{addition}</li>
                ))}
              </ul>
            </div>
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
        <KeptAdditions worldId={worldId} version={listVersion} />
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
 * Browse by compendium, inside the world (spec 049 FR-042), as this world
 * reads it (spec 050 FR-022).
 *
 * The thing merging the two specs bought: a Game Master reaches what a book
 * says from the table it is switched on for, rather than only from the shelf.
 * Every entry names its book and page, as it does in the library, because
 * FR-043 is a promise to whoever is reading the entry and not to a surface.
 *
 * # Changing what the world inherited
 *
 * Only the people who manage a table's books see this component at all
 * (FR-020a), so its controls are theirs. Every change lands in this world
 * only; the book underneath and every other world are untouched, and each
 * changed entry shows what the book says beside what the world says (FR-024).
 *
 * # Two badges that look alike and mean opposite things
 *
 * A changed entry and an added one sit side by side here, and one may be
 * shared while the other may not (FR-052, FR-052a). The origin is shown on
 * every entry, in words, from what the server says — this component never
 * works it out from the state, because working it out is how it gets worked
 * out wrongly.
 */
function BookEntries({
  worldId,
  compendiumId,
}: {
  worldId: string;
  compendiumId: string;
}) {
  const [entries, setEntries] = useState<WorldBookEntry[]>([]);
  const [unattached, setUnattached] = useState<UnattachedDelta[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [kind, setKind] = useState<string | null>(null);
  const [showHidden, setShowHidden] = useState(false);
  const [version, setVersion] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    entriesFrom(worldId, compendiumId, { kind, showHidden })
      .then((page) => {
        if (!live) return;
        setEntries(page.entries);
        setUnattached(page.unattached);
        setCursor(page.nextCursor);
        setTotal(page.total);
      })
      .catch(() => {
        if (live) setError("Those entries could not be read.");
      });
    return () => {
      live = false;
    };
  }, [worldId, compendiumId, kind, showHidden, version]);

  const more = useCallback(() => {
    if (!cursor) return;
    entriesFrom(worldId, compendiumId, { kind, after: cursor, showHidden })
      .then((page) => {
        setEntries((held) => [...held, ...page.entries]);
        setCursor(page.nextCursor);
      })
      .catch(() => setError("Those entries could not be read."));
  }, [worldId, compendiumId, cursor, kind, showHidden]);

  /** Run a change, then read the book again rather than patching the list —
   * a client that patches its own copy is a client whose copy can be wrong. */
  const act = useCallback((change: Promise<unknown>) => {
    change
      .then(() => {
        setRefusal(null);
        setVersion((seen) => seen + 1);
      })
      .catch((cause: unknown) =>
        setRefusal(
          cause instanceof Error ? cause.message : "That change was refused.",
        ),
      );
  }, []);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  const kinds = Array.from(new Set(entries.map((entry) => entry.kind)));
  const address = (entry: WorldBookEntry): EntryAddress => ({
    worldId,
    compendiumId,
    kind: entry.kind,
    name: entry.name,
  });

  return (
    <div className="grid gap-2" data-testid="world-book-browser">
      <nav className="flex flex-wrap items-center gap-2">
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
        <label className="flex items-center gap-1 text-sm">
          <input
            type="checkbox"
            checked={showHidden}
            onChange={(event) => setShowHidden(event.target.checked)}
            data-testid="world-show-hidden"
          />
          Show what this world hides
        </label>
      </nav>

      <p
        className="text-sm text-muted-foreground"
        data-testid="world-entry-count"
      >
        Showing {entries.length} of {total}
      </p>

      {refusal && (
        <StatusBadge variant="danger" data-testid="entry-refusal">
          {refusal}
        </StatusBadge>
      )}

      {/* FR-025a, FR-027: a change this world holds and cannot apply is
          said, not dropped. */}
      {unattached.length > 0 && (
        <div
          className="grid gap-1 rounded-md border border-amber-500 p-2 text-sm"
          data-testid="unattached-deltas"
        >
          <p>Changes this world holds that are not being applied:</p>
          <ul className="list-disc pl-5">
            {unattached.map((delta) => (
              <li key={`${delta.kind}/${delta.name}`}>
                {delta.form} {delta.kind} &ldquo;{delta.name}&rdquo; —{" "}
                {delta.reason}
              </li>
            ))}
          </ul>
        </div>
      )}

      <ul className="grid gap-2">
        {entries.map((entry) => (
          <li
            key={entry.id}
            className="rounded-md border p-2"
            data-testid={`world-entry-${entry.id}`}
            data-kind={entry.kind}
            data-name={entry.name}
            data-state={entry.state}
          >
            <WorldEntryRow
              entry={entry}
              onChange={(content) => act(changeEntry(address(entry), content))}
              onHide={() => act(hideEntry(address(entry)))}
              onRestore={() => act(restoreEntry(address(entry)))}
            />
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

      <AddEntryForm
        onAdd={(kindName, name, proseText) =>
          act(
            addEntry(
              { worldId, compendiumId, kind: kindName, name },
              { proseText },
            ),
          )
        }
      />
    </div>
  );
}

const STATE_WORDS: Record<WorldBookEntry["state"], string | null> = {
  INHERITED: null,
  CHANGED: "Changed in this world",
  HIDDEN: "Hidden in this world",
  ADDED: "Added in this world",
};

function WorldEntryRow({
  entry,
  onChange,
  onHide,
  onRestore,
}: {
  entry: WorldBookEntry;
  onChange: (
    content: { fieldValues: Record<string, ReadValue> } | { proseText: string },
  ) => void;
  onHide: () => void;
  onRestore: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [prose, setProse] = useState("");
  const stateWord = STATE_WORDS[entry.state];
  const isProse = entry.proseText !== null;

  const startEditing = () => {
    setDraft(
      Object.fromEntries(
        Object.entries(entry.fieldValues).map(([field, value]) => [
          field,
          value.state === "unread" ? "" : value.value,
        ]),
      ),
    );
    setProse(entry.proseText ?? "");
    setEditing(true);
  };

  const save = () => {
    if (isProse) {
      onChange({ proseText: prose });
    } else {
      // Only the fields a person actually altered are sent; the server keeps
      // only what differs from the book either way (FR-023).
      const altered = Object.fromEntries(
        Object.entries(draft)
          .filter(([field, text]) => {
            const was = entry.fieldValues[field];
            const wasText = was?.state === "unread" ? "" : (was?.value ?? "");
            return text !== wasText;
          })
          .map(([field, text]): [string, ReadValue] => [
            field,
            text === "" ? { state: "unread" } : { state: "clear", value: text },
          ]),
      );
      onChange({ fieldValues: altered });
    }
    setEditing(false);
  };

  return (
    <details>
      <summary className="cursor-pointer text-sm">
        <span className="font-medium">{entry.name}</span>{" "}
        <span className="text-muted-foreground">
          — {entry.kind}, {entry.bookTitle}
          {entry.page === null ? ", not in the book" : `, page ${entry.page}`}
        </span>{" "}
        {stateWord && (
          <StatusBadge
            variant={entry.state === "HIDDEN" ? "warning" : "info"}
            data-testid="world-entry-state"
          >
            {stateWord}
          </StatusBadge>
        )}{" "}
        <span
          className="text-xs text-muted-foreground"
          data-testid="world-entry-origin"
          data-origin={entry.origin}
          data-may-be-shared={entry.mayBeShared ? "true" : "false"}
          title={entry.notShareableBecause ?? undefined}
        >
          {entry.mayBeShared
            ? "Authored here — may be shared"
            : "Uploaded — stays with this account"}
        </span>
      </summary>

      {editing ? (
        <div className="mt-2 grid gap-2">
          {isProse ? (
            <textarea
              className="rounded-md border p-2 text-sm"
              value={prose}
              onChange={(event) => setProse(event.target.value)}
              data-testid="edit-entry-prose"
            />
          ) : (
            <div className="grid grid-cols-2 gap-1 text-sm">
              {Object.keys(draft).map((field) => (
                <label key={field} className="contents">
                  <span className="text-muted-foreground">{field}</span>
                  <input
                    className="rounded-md border px-2"
                    value={draft[field]}
                    onChange={(event) =>
                      setDraft((held) => ({
                        ...held,
                        [field]: event.target.value,
                      }))
                    }
                    data-edit-field={field}
                  />
                </label>
              ))}
            </div>
          )}
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              onClick={save}
              data-testid="save-entry"
            >
              Save for this world
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setEditing(false)}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <>
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
        </>
      )}

      {/* FR-024: what the book says, beside what this world says. */}
      {entry.before && (
        <div
          className="mt-2 rounded-md bg-muted p-2 text-sm"
          data-testid="entry-before"
        >
          <p className="font-medium">Before this world changed it</p>
          <dl className="grid grid-cols-2 gap-1">
            {Object.entries(entry.before.fieldValues).map(([field, value]) => (
              <div key={field} className="contents">
                <dt className="text-muted-foreground">{field}</dt>
                <dd data-before-field={field}>{shown(value)}</dd>
              </div>
            ))}
          </dl>
          {entry.before.proseText && (
            <p className="whitespace-pre-wrap">{entry.before.proseText}</p>
          )}
        </div>
      )}

      {entry.ambiguous ? (
        <p
          className="mt-2 text-sm text-muted-foreground"
          data-testid="entry-ambiguous"
        >
          This book has more than one {entry.kind} named &ldquo;{entry.name}
          &rdquo;, so this world cannot tell them apart to change either.
        </p>
      ) : (
        <div className="mt-2 flex flex-wrap gap-2">
          {!editing && entry.state !== "HIDDEN" && (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={startEditing}
              data-testid="edit-entry"
            >
              Change for this world
            </Button>
          )}
          {(entry.state === "INHERITED" || entry.state === "CHANGED") && (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={onHide}
              data-testid="hide-entry"
            >
              Hide in this world
            </Button>
          )}
          {entry.state !== "INHERITED" && (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={onRestore}
              data-testid="restore-entry"
            >
              {entry.state === "ADDED"
                ? "Remove"
                : "Put back as the book has it"}
            </Button>
          )}
        </div>
      )}
    </details>
  );
}

/**
 * Write an entry into this world beside the book (FR-021, FR-052a).
 *
 * Prose only here: a written note or house rule is what a table adds by hand
 * most, and an entry with declared fields needs the system's own sheet to be
 * written well rather than a row of free-text boxes.
 */
function AddEntryForm({
  onAdd,
}: {
  onAdd: (kind: string, name: string, proseText: string) => void;
}) {
  const [kind, setKind] = useState("");
  const [name, setName] = useState("");
  const [text, setText] = useState("");

  return (
    <form
      className="grid gap-1 rounded-md border p-2 text-sm"
      data-testid="add-entry"
      onSubmit={(event) => {
        event.preventDefault();
        if (!kind.trim() || !name.trim()) return;
        onAdd(kind.trim(), name.trim(), text);
        setName("");
        setText("");
      }}
    >
      <p className="font-medium">Add an entry to this world</p>
      <p className="text-muted-foreground">
        It sits beside the book in this world only, and it is yours: written
        here, so it may be shared.
      </p>
      <input
        className="rounded-md border px-2"
        placeholder="Kind"
        value={kind}
        onChange={(event) => setKind(event.target.value)}
        data-testid="add-entry-kind"
      />
      <input
        className="rounded-md border px-2"
        placeholder="Name"
        value={name}
        onChange={(event) => setName(event.target.value)}
        data-testid="add-entry-name"
      />
      <textarea
        className="rounded-md border p-2"
        placeholder="What it says"
        value={text}
        onChange={(event) => setText(event.target.value)}
        data-testid="add-entry-text"
      />
      <div>
        <Button type="submit" size="sm" data-testid="add-entry-submit">
          Add to this world
        </Button>
      </div>
    </form>
  );
}
