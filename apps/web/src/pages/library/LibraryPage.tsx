import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Container } from "@/components/ui/container/Container";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  confirmRemoveCompendium,
  previewRemoveCompendium,
  type RemovalReport,
} from "@/api/compendium";
import {
  downloadShelfCollection,
  saveFile,
  type ShelfCollectionDownload,
} from "@/api/shelfCollections";
import type { SeoConfig } from "@/types/seo";
import { BookBrowser } from "./BookBrowser";
import { ImportBook } from "./ImportBook";
import { NewCollection } from "./NewCollection";
import { myLibrary, type LibraryBook } from "./library";

/**
 * The account's shelf (spec 050 US1, spec 049 US3).
 *
 * # Why this is a new surface rather than a tab on the world Compendium
 *
 * A compendium belongs to an **account** and never to a world (049 FR-040):
 * a Game Master with eight worlds and one Monster Manual owns one Monster
 * Manual. The world's Compendium portal answers "what can this table use",
 * which is a different question and, until spec 050's book list exists, one
 * this page cannot answer at all — a world cannot reach a compendium yet.
 * Research §1 records the decision; putting the shelf inside a world would
 * have been the retrofit it exists to avoid.
 *
 * # Who reaches it
 *
 * Whoever is signed in, and only their own shelf. There is no account id in
 * the route, no account id in any query this page sends, and nothing on the
 * server that would answer one — 049 FR-028's "only a Game Master, from their
 * own panel" lands here as "a person's library is theirs", which is the form
 * that requirement can actually take on an account-level surface.
 */
export const libraryPageSeo: SeoConfig = {
  title: "Your library",
  description: "The books you have read in, and what came out of them.",
  canonicalPath: "/library",
  noindex: true,
};

export function LibraryPage() {
  const { compendiumId } = useParams<{ compendiumId: string }>();
  const [shelf, setShelf] = useState<LibraryBook[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<RemovalReport | null>(null);
  const [downloaded, setDownloaded] = useState<ShelfCollectionDownload | null>(
    null,
  );

  const read = useCallback(() => {
    myLibrary()
      .then((books) => {
        setShelf(books);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "Your library could not be read.",
        );
      });
  }, []);

  useEffect(read, [read]);

  if (compendiumId) {
    return (
      <>
        <SEO {...libraryPageSeo} />
        <Container>
          <div className="grid gap-6 py-8">
            <BookBrowser compendiumId={compendiumId} />
          </div>
        </Container>
      </>
    );
  }

  return (
    <>
      <SEO {...libraryPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Your library</h1>
            <p className="text-sm text-muted-foreground">
              Books you have read in, and collections you have written. They
              belong to this account, not to any one world, and they go when the
              account goes.
            </p>
          </header>

          <ImportBook onImported={read} />
          <NewCollection onCreated={read} />

          {downloaded && (
            <p
              className="text-sm"
              role="status"
              data-testid="collection-downloaded"
            >
              Saved {downloaded.fileName} with {downloaded.entryCount}{" "}
              {downloaded.entryCount === 1 ? "entry" : "entries"}.
              {downloaded.excluded.length === 0
                ? " Nothing was left out."
                : ` Left out: ${downloaded.excluded
                    .map(
                      (excluded) =>
                        `${excluded.kind} “${excluded.name}” — ${excluded.reason}`,
                    )
                    .join("; ")}`}
            </p>
          )}

          {error && <StatusBadge variant="danger">{error}</StatusBadge>}

          {shelf === null && !error && (
            <p className="text-muted-foreground">Reading your shelf...</p>
          )}

          {shelf !== null && shelf.length === 0 && (
            <p className="text-muted-foreground" data-testid="library-empty">
              You have not read any books in yet.
            </p>
          )}

          {shelf !== null && shelf.length > 0 && (
            <ul className="grid gap-3" data-testid="library-shelf">
              {shelf.map((held) => (
                <li
                  key={held.id}
                  className="grid gap-2 rounded-md border p-4"
                  data-testid={`library-book-${held.id}`}
                  data-origin={held.origin}
                >
                  <div className="flex flex-wrap items-baseline justify-between gap-2">
                    <Link
                      className="text-lg font-medium underline"
                      to={`/library/${held.id}`}
                      data-testid="open-book"
                    >
                      {held.bookTitle}
                    </Link>
                    <span className="text-sm text-muted-foreground">
                      {held.systemId} —{" "}
                      {held.origin === "AUTHORED" ? "started" : "read in"} on{" "}
                      {new Date(held.importedAt).toLocaleDateString()}
                    </span>
                  </div>

                  <p className="text-sm" data-testid="book-counts">
                    {held.entryCounts.length === 0
                      ? held.origin === "AUTHORED"
                        ? "Nothing written in it yet"
                        : "Nothing was kept from this book."
                      : held.entryCounts
                          .map((count) => `${count.count} ${count.kind}`)
                          .join(", ")}
                    {held.origin === "UPLOADED" && (
                      <>
                        {" "}
                        — {held.pageCount} pages
                        {held.silentPageCount > 0 &&
                          `, ${held.silentPageCount} of them images with no text`}
                      </>
                    )}
                    .
                  </p>

                  <p
                    className="text-xs text-muted-foreground"
                    data-testid="book-origin"
                  >
                    {held.origin === "UPLOADED"
                      ? "Read out of a document you supplied. It stays with this account: it cannot be shared, published or exported."
                      : "A collection, written in ThunderForge. It is yours to take with you."}
                  </p>

                  <div className="flex flex-wrap gap-2">
                    {/* FR-009a and FR-009b: a collection downloads; a book
                        read in offers no way to, and the server refuses one
                        asked for anyway. */}
                    {held.origin === "AUTHORED" && (
                      <Button
                        type="button"
                        size="sm"
                        variant="secondary"
                        data-testid="download-collection"
                        onClick={() => {
                          downloadShelfCollection(held.id)
                            .then((file) => {
                              saveFile(file.fileName, file.contents);
                              setDownloaded(file);
                            })
                            .catch(() =>
                              setError("That collection could not be saved."),
                            );
                        }}
                      >
                        Download as JSON
                      </Button>
                    )}
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      data-testid="remove-book"
                      onClick={() => {
                        // FR-045: what is in use is named **before** anything
                        // is confirmed, which is why this asks first and the
                        // confirmation is a second call.
                        previewRemoveCompendium(held.id)
                          .then(setRemoving)
                          .catch(() =>
                            setError("That book could not be checked."),
                          );
                      }}
                    >
                      Remove
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}

          {removing && (
            <div
              className="grid gap-2 rounded-md border p-4"
              data-testid="removal-report"
            >
              <p className="text-sm">
                Removing <strong>{removing.bookTitle}</strong> deletes the book
                itself: its {removing.entryCount} entries leave your shelf and
                every world running it, and it cannot be switched back on
                without reading the book in again.
              </p>
              <p className="text-sm" data-testid="removal-in-use">
                {removing.inUse.length === 0
                  ? "No world is using anything from this book."
                  : removing.inUse
                      .map(
                        (use) =>
                          `${use.worldName} is using ${use.entryNames.join(", ")}`,
                      )
                      .join("; ")}
              </p>
              {/* Spec 050 decision 5: a table's changes and hides go with the
                  book; what it added beside the book stays, and the person
                  removing the book is told both, per world, before they
                  decide. */}
              {removing.deltasByWorld.map((world) => (
                <div
                  key={world.worldId}
                  className="grid gap-1 text-sm"
                  data-testid={`removal-world-${world.worldId}`}
                >
                  <p className="font-medium">{world.worldName}</p>
                  {world.lost.length > 0 && (
                    <p data-testid="removal-deltas-lost">
                      Its changes to this book go too: {world.lost.join(", ")}.
                    </p>
                  )}
                  {world.kept.length > 0 && (
                    <p data-testid="removal-additions-kept">
                      What it added beside this book stays in that world, as its
                      own writing: {world.kept.join(", ")}.
                    </p>
                  )}
                </div>
              ))}
              <div className="flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="danger"
                  data-testid="remove-confirm"
                  onClick={() => {
                    confirmRemoveCompendium(removing.compendiumId)
                      .then(() => {
                        setRemoving(null);
                        read();
                      })
                      .catch(() => setError("That book could not be removed."));
                  }}
                >
                  Remove it
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  data-testid="remove-cancel"
                  onClick={() => setRemoving(null)}
                >
                  Keep it
                </Button>
              </div>
            </div>
          )}
        </div>
      </Container>
    </>
  );
}

export default LibraryPage;
