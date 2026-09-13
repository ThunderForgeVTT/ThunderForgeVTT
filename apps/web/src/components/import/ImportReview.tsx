import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button/Button";
import type { ContentEntry } from "@/engine/sdk/ContentEntry";
import type { ContentPatterns } from "@/engine/sdk/ContentPatterns";
import {
  hashOf,
  readBook,
  type ReadBook,
  type ReadProgress,
} from "@/services/bookImport";
import { BookReview } from "./BookReview";

/**
 * The window between "the reader has finished" and "the world has changed".
 *
 * Spec 049 US1, FR-020 to FR-026. A Game Master hands this a file off their
 * own disk and the patterns their system declares; it reads the book **here**,
 * shows what came out of it, and hands back what they approved. It sends
 * nothing. It knows no world, no account and no mutation: a caller receives
 * entries and decides what to do with them, which is what keeps FR-025 true by
 * construction — abandoning this leaves nothing behind because there is
 * nothing outside it to leave behind.
 *
 * # Why it is not wired into a page yet
 *
 * The account library that will host it is spec 050's, and the only other
 * plausible home — a world panel — would be the wrong one: a book is imported
 * once per account, not once per world. A component with no route into it is a
 * component nobody can reach by mistake.
 */
export interface ApprovedImport {
  /** What to call the compendium this becomes. The file's own name. */
  title: string;
  /** Taken here, before anything is read, so a wrong file is caught at once. */
  sha256: string;
  pages: number;
  silentPages: number;
  /** Only what the Game Master approved, with their corrections applied. */
  entries: ContentEntry[];
}

export interface ImportReviewProps {
  file: File;
  patterns: ContentPatterns;
  /**
   * Called once, and only from the submit button. There is no other route out
   * of this component that produces entries (FR-024).
   */
  onSubmit: (approved: ApprovedImport) => void;
  onClose: () => void;
}

/**
 * Where the read has got to.
 *
 * A union rather than a bag of optional fields, so that "finished, and here
 * are no entries" and "still reading" cannot be confused for one another, and
 * so the review cannot be rendered before there is a book to render.
 */
type Reading =
  | { phase: "reading"; progress: ReadProgress | null }
  | { phase: "failed"; reason: string }
  | { phase: "read"; book: ReadBook; sha256: string };

export function ImportReview({
  file,
  patterns,
  onSubmit,
  onClose,
}: ImportReviewProps) {
  const [reading, setReading] = useState<Reading>({
    phase: "reading",
    progress: null,
  });

  useEffect(() => {
    // Guarded rather than fire-and-forget: a Game Master who picks a second
    // file while the first is still being read would otherwise be shown
    // whichever read happened to finish last.
    let cancelled = false;
    (async () => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const sha256 = await hashOf(bytes);
      const book = await readBook(bytes, patterns, (progress) => {
        if (!cancelled) setReading({ phase: "reading", progress });
      });
      if (!cancelled) setReading({ phase: "read", book, sha256 });
    })().catch((error: unknown) => {
      if (cancelled) return;
      // Named, not swallowed. A file that is not a PDF, a document the reader
      // cannot open and a browser that ran out of memory are different
      // problems, and only one of them is worth trying again.
      setReading({
        phase: "failed",
        reason: error instanceof Error ? error.message : String(error),
      });
    });
    return () => {
      cancelled = true;
    };
  }, [file, patterns]);

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        // Closing is closing. It does not submit, and it does not ask whether
        // the Game Master meant it — there is nothing yet to lose (FR-024,
        // FR-025).
        if (!open) onClose();
      }}
    >
      <DialogContent
        className="sm:max-w-3xl"
        data-testid="import-review"
        aria-describedby={undefined}
      >
        <DialogTitle className="sr-only">
          What we found in {file.name}
        </DialogTitle>
        {reading.phase === "reading" && (
          <div className="space-y-2" data-testid="book-reading">
            <p className="text-sm font-medium">Reading {file.name}</p>
            <DialogDescription>
              {reading.progress
                ? `Page ${reading.progress.page} of ${reading.progress.pages} — ${reading.progress.entriesSoFar} found so far.`
                : "Opening the book."}
            </DialogDescription>
          </div>
        )}
        {reading.phase === "failed" && (
          <div className="space-y-4" data-testid="book-failed">
            <p className="text-sm font-medium">
              We could not read {file.name}.
            </p>
            <p className="text-sm text-muted-foreground">{reading.reason}</p>
            <div className="flex justify-end">
              <Button variant="secondary" onClick={onClose}>
                Close
              </Button>
            </div>
          </div>
        )}
        {reading.phase === "read" && (
          <BookReview
            title={file.name}
            book={reading.book}
            onClose={onClose}
            onSubmit={(entries) =>
              onSubmit({
                title: file.name,
                sha256: reading.sha256,
                pages: reading.book.pages,
                silentPages: reading.book.silentPages,
                entries,
              })
            }
          />
        )}
      </DialogContent>
    </Dialog>
  );
}
