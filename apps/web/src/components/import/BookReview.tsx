import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { cn } from "@/lib/utils";
import type { ContentEntry } from "@/engine/sdk/ContentEntry";
import type { ReadValue } from "@/engine/sdk/ReadValue";
import {
  unreadCount,
  yieldedNothing,
  type ReadBook,
} from "@/services/bookImport";
import {
  approved,
  correct,
  correctionFor,
  includeEntry,
  includeKind,
  isIncluded,
  nothingExcluded,
  tally,
  type Review,
} from "./selection";

/**
 * Everything a book gave up, laid out so a Game Master can refuse it.
 *
 * Spec 049 US1. Nothing has been sent by the time this renders and nothing is
 * sent by it: the only way out of here that changes anything is the submit
 * button, and `onSubmit` is the only place it is called (FR-024). There is
 * deliberately no timer, no default action and no submit-on-close — closing is
 * `onClose`, and abandoning a review leaves nothing behind because the review
 * never wrote anything down (FR-025).
 *
 * # Why this is a separate component from the window around it
 *
 * `ImportReview` owns the file, the read and the dialog; this owns what the
 * Game Master decided about a book that has already been read. Splitting them
 * means the decisions can be rendered and asserted without a browser, a wasm
 * module or a PDF — which is how the rules about *what appears* are proved.
 */
export interface BookReviewProps {
  /** What to call the book. The file's name, until somebody renames it. */
  title: string;
  book: ReadBook;
  /** The entries the Game Master approved, corrections already applied. */
  onSubmit: (entries: ContentEntry[]) => void;
  onClose: () => void;
}

/** A group of entries as the review lists them, with their positions kept. */
interface Group {
  kind: string;
  at: number[];
}

function groupsOf(entries: readonly ContentEntry[]): Group[] {
  const groups = new Map<string, Group>();
  entries.forEach((entry, at) => {
    const existing = groups.get(entry.kind) ?? { kind: entry.kind, at: [] };
    existing.at.push(at);
    groups.set(entry.kind, existing);
  });
  return [...groups.values()];
}

export function BookReview({
  title,
  book,
  onSubmit,
  onClose,
}: BookReviewProps) {
  const [review, setReview] = useState<Review>(nothingExcluded);

  const groups = useMemo(() => groupsOf(book.entries), [book.entries]);
  const tallies = useMemo(() => tally(book.entries, review), [book, review]);
  const chosen = useMemo(
    () => approved(book.entries, review),
    [book.entries, review],
  );

  // FR-005, and the first thing a Game Master needs to know: a book that is
  // images from cover to cover has not been imported successfully with nothing
  // in it — it has not been read at all. There is no submit button on this
  // branch, which is a stronger statement than a disabled one.
  if (yieldedNothing(book)) {
    return (
      <section className="space-y-4" data-testid="book-unreadable">
        <header className="space-y-1">
          <h2 className="text-lg font-semibold">{title}</h2>
          <p className="text-sm text-muted-foreground">
            Nothing has been sent.
          </p>
        </header>
        <p className="text-sm">
          We could not read this book. All {book.pages} of its pages are images
          with no text behind them, so there is nothing here to import — this is
          not an import that found nothing.
        </p>
        <div className="flex justify-end">
          <Button variant="secondary" onClick={onClose}>
            Close
          </Button>
        </div>
      </section>
    );
  }

  return (
    <section className="flex max-h-[80vh] flex-col gap-4">
      <header className="space-y-1">
        <h2 className="text-lg font-semibold">{title}</h2>
        <p className="text-sm text-muted-foreground">
          Read on this machine. Nothing has been sent yet.
        </p>
      </header>

      <div className="space-y-2 text-sm">
        <p data-testid="found-total">
          <strong>{book.entries.length}</strong> found across {book.pages}{" "}
          pages.
        </p>
        <ul className="flex flex-wrap gap-2">
          {tallies.map((kind) => (
            <li key={kind.kind}>
              <Badge
                variant="secondary"
                data-testid={`tally-${kind.kind}`}
                title={`${kind.included} of ${kind.found} ${kind.kind} entries will be imported`}
              >
                {kind.kind}: {kind.included} of {kind.found}
              </Badge>
            </li>
          ))}
        </ul>
        {book.silentPages > 0 && (
          <p
            data-testid="silent-pages"
            className="text-amber-600 dark:text-amber-400"
          >
            {book.silentPages} of {book.pages} pages had no text on them and
            could not be read. Whatever is printed on those pages is not below.
          </p>
        )}
        {book.repaired && (
          <p className="text-muted-foreground">
            This file was damaged and had to be repaired before it could be
            opened. Look at what came out of it with that in mind.
          </p>
        )}
      </div>

      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
        {groups.map((group) => (
          <KindSection
            key={group.kind}
            group={group}
            book={book}
            review={review}
            onReview={setReview}
          />
        ))}
      </div>

      <footer className="flex items-center justify-between gap-4 border-t border-border pt-3">
        <p className="text-sm text-muted-foreground" data-testid="submit-count">
          {chosen.length} of {book.entries.length} will be imported.
        </p>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={onClose}>
            Close without importing
          </Button>
          <Button
            data-testid="submit-import"
            disabled={chosen.length === 0}
            onClick={() => onSubmit(chosen)}
          >
            Import {chosen.length}
          </Button>
        </div>
      </footer>
    </section>
  );
}

function KindSection({
  group,
  book,
  review,
  onReview,
}: {
  group: Group;
  book: ReadBook;
  review: Review;
  onReview: (review: Review) => void;
}) {
  const kindIncluded = !review.excludedKinds.has(group.kind);
  return (
    <section className="rounded-lg border border-border">
      <header className="flex items-center gap-2 border-b border-border px-3 py-2">
        <Checkbox
          checked={kindIncluded}
          data-testid={`include-kind-${group.kind}`}
          aria-label={`Import every ${group.kind}`}
          onCheckedChange={(checked) =>
            onReview(includeKind(review, group.kind, checked === true))
          }
        />
        <h3 className="text-sm font-semibold">{group.kind}</h3>
        <span className="text-sm text-muted-foreground">{group.at.length}</span>
      </header>
      <ul className="divide-y divide-border">
        {group.at.map((at) => (
          <EntryRow
            key={at}
            at={at}
            entry={book.entries[at]!}
            included={isIncluded(review, book.entries, at)}
            review={review}
            onReview={onReview}
          />
        ))}
      </ul>
    </section>
  );
}

function EntryRow({
  at,
  entry,
  included,
  review,
  onReview,
}: {
  at: number;
  entry: ContentEntry;
  included: boolean;
  review: Review;
  onReview: (review: Review) => void;
}) {
  const unread = unreadCount(entry);
  return (
    <li
      className={cn("px-3 py-2", !included && "opacity-50")}
      data-testid={`entry-${at}`}
      data-included={included}
    >
      <div className="flex items-start gap-2">
        <Checkbox
          className="mt-1"
          checked={included}
          data-testid={`include-entry-${at}`}
          aria-label={`Import ${entry.name}`}
          onCheckedChange={(checked) =>
            onReview(includeEntry(review, at, checked === true))
          }
        />
        <details className="min-w-0 flex-1">
          <summary className="flex cursor-pointer flex-wrap items-center gap-2">
            <span className="font-medium">{entry.name}</span>
            {/* FR-022: the page it came from, so it can be checked against the
                book on the table rather than taken on trust. */}
            <span className="text-xs text-muted-foreground">
              page {entry.page}
            </span>
            {entry.nameState === "uncertain" && (
              <StatusBadge variant="warning" data-testid={`unsure-name-${at}`}>
                name unsure
              </StatusBadge>
            )}
            {entry.suspect && (
              <StatusBadge variant="danger" data-testid={`suspect-${at}`}>
                from a page we struggled with
              </StatusBadge>
            )}
            {unread > 0 && (
              <StatusBadge variant="warning" data-testid={`unread-${at}`}>
                {unread} not found
              </StatusBadge>
            )}
          </summary>
          <EntryDetail
            at={at}
            entry={entry}
            review={review}
            onReview={onReview}
          />
        </details>
      </div>
    </li>
  );
}

function EntryDetail({
  at,
  entry,
  review,
  onReview,
}: {
  at: number;
  entry: ContentEntry;
  review: Review;
  onReview: (review: Review) => void;
}) {
  if (entry.text !== null) {
    // A prose kind has no fields to be certain about — it is a name, its text
    // and where it came from, and the importer invents nothing else (FR-001b).
    return (
      <p
        className="mt-2 text-sm whitespace-pre-wrap"
        data-testid={`text-${at}`}
      >
        {entry.text}
      </p>
    );
  }

  return (
    <dl className="mt-2 grid grid-cols-[10rem_1fr] gap-x-3 gap-y-1 text-sm">
      {Object.entries(entry.values).map(([field, value]) => (
        <FieldRow
          key={field}
          at={at}
          field={field}
          value={value}
          review={review}
          onReview={onReview}
        />
      ))}
    </dl>
  );
}

/**
 * One field, shown as what it is: read, unsure, or not found at all.
 *
 * FR-023 asks for those three to be visibly different, and the shape of
 * `ReadValue` is what makes the third honest — the `unread` case carries no
 * value, so this cannot render a placeholder for it even by accident. That is
 * also why the switch is written over the state rather than over "is there a
 * value": there is no branch here in which a missing value could acquire one.
 */
function FieldRow({
  at,
  field,
  value,
  review,
  onReview,
}: {
  at: number;
  field: string;
  value: ReadValue;
  review: Review;
  onReview: (review: Review) => void;
}) {
  const corrected = correctionFor(review, at, field);
  return (
    <>
      <dt className="text-muted-foreground">{field}</dt>
      <dd data-field={field} data-read-state={value.state}>
        {value.state === "unread" ? (
          <span className="text-muted-foreground italic">not found</span>
        ) : value.state === "clear" ? (
          <span>{value.value}</span>
        ) : (
          <span className="flex flex-wrap items-center gap-2">
            <Input
              className="h-7 max-w-40 border-amber-500 bg-amber-500/5"
              value={corrected ?? value.value}
              aria-label={`${field}, unsure — correct it if the book says otherwise`}
              data-testid={`correct-${at}-${field}`}
              onChange={(event) =>
                onReview(correct(review, at, field, event.target.value))
              }
            />
            <StatusBadge variant="warning">unsure</StatusBadge>
          </span>
        )}
      </dd>
    </>
  );
}
