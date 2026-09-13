/**
 * Reading the account's shelf (spec 049 FR-040 to FR-043, spec 050 FR-002).
 *
 * `@/api/compendium` owns writing — sending a reviewed book, and taking one
 * off the shelf. This owns reading it, and it lives beside the page because
 * the library is the only surface that asks: there is no world-scoped read of
 * a compendium in this spec, so there is nothing else to share it with.
 *
 * Every query here is account-scoped at the server, with no argument that
 * could name another account's shelf. A person reaches their own library and
 * nobody else's, and that is enforced where it has to be rather than by this
 * module's restraint.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { Compendium } from "@/api/compendium";
import type { ReadValue } from "@/engine/sdk/ReadValue";

/** One book on the shelf, with when it was read in. */
export interface LibraryBook extends Compendium {
  parserVersion: string;
  importedAt: string;
  updatedAt: string;
}

/** One entry, naming the book it came from and the page it was on (FR-043). */
export interface LibraryEntry {
  id: string;
  compendiumId: string;
  bookTitle: string;
  kind: string;
  name: string;
  nameUncertain: boolean;
  page: number;
  /**
   * Each declared field with the certainty it was read at. A field looked for
   * and not found is `{ state: "unread" }` and has no value to show, which is
   * the shape rather than a convention.
   */
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
  suspect: boolean;
}

/** One page of a book's entries, and where the next one starts. */
export interface LibraryEntryPage {
  entries: LibraryEntry[];
  /** `null` when there is no next page. */
  nextCursor: string | null;
  /** Everything matching the filter, not just what this page carries. */
  total: number;
}

const BOOK_FIELDS = `
  id
  bookTitle
  sourceHash
  systemId
  origin
  parserVersion
  pageCount
  silentPageCount
  entryTotal
  entryCounts { kind count }
  importedAt
  updatedAt
`;

/** Every book this account has read in, newest first. */
export async function myLibrary(): Promise<LibraryBook[]> {
  const data = await postGraphQL<{ myLibrary: LibraryBook[] }>(
    `query MyLibrary { myLibrary { ${BOOK_FIELDS} } }`,
  );
  return data.myLibrary;
}

/**
 * One book, or `null` — which is also the answer for a book belonging to
 * somebody else, deliberately and at the server.
 */
export async function book(id: string): Promise<LibraryBook | null> {
  const data = await postGraphQL<{ compendium: LibraryBook | null }>(
    `query Compendium($id: UUID!) { compendium(id: $id) { ${BOOK_FIELDS} } }`,
    { id },
  );
  return data.compendium;
}

/**
 * What came out of one book, a page at a time (FR-042).
 *
 * Paged because a sourcebook produces thousands of entries and a screen shows
 * twenty. `after` is a previous page's `nextCursor` and nothing else.
 */
export async function entriesOf(
  compendiumId: string,
  options: { kind?: string | null; after?: string | null; first?: number } = {},
): Promise<LibraryEntryPage> {
  const data = await postGraphQL<{ compendiumEntries: LibraryEntryPage }>(
    `query CompendiumEntries(
       $compendiumId: UUID!
       $kind: String
       $after: String
       $first: Int
     ) {
       compendiumEntries(
         compendiumId: $compendiumId
         kind: $kind
         after: $after
         first: $first
       ) {
         total
         nextCursor
         entries {
           id
           compendiumId
           bookTitle
           kind
           name
           nameUncertain
           page
           fieldValues
           proseText
           suspect
         }
       }
     }`,
    {
      compendiumId,
      kind: options.kind ?? null,
      after: options.after ?? null,
      first: options.first ?? null,
    },
  );
  return data.compendiumEntries;
}
