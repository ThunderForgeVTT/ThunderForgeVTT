/**
 * The book list, from the world's side (spec 050 FR-030 to FR-036, FR-041,
 * FR-042; spec 049 FR-042).
 *
 * `@/pages/library/library` reads an account's shelf. This reads what one
 * **world** is running, which is a different question with a different
 * audience: every member of the world may ask it, players included, and
 * nothing it returns is content.
 *
 * Switching a book on and off is here too, and there is exactly one of each.
 * Ticking books while creating a world and ticking them afterwards call the
 * same mutation (FR-033) — two routes to one state would be two things to
 * keep agreeing.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { ReadValue } from "@/engine/sdk/ReadValue";

/** How many of one kind a book holds. */
export interface BookKindCount {
  kind: string;
  count: number;
}

/**
 * One book this table is running.
 *
 * The name, the system, and how much is in it. Deliberately nothing that
 * amounts to its content (FR-036) — this is what a player is shown.
 */
export interface WorldBook {
  compendiumId: string;
  bookTitle: string;
  systemId: string;
  entryCounts: BookKindCount[];
  entryTotal: number;
  /** False once the world has changed system underneath the book (FR-042). */
  systemMatches: boolean;
  baseParserVersion: string;
  switchedOnAt: string;
}

/** A book on the owner's shelf that this world could switch on. */
export interface OfferedBook {
  id: string;
  bookTitle: string;
  systemId: string;
  entryTotal: number;
  importedAt: string;
}

/** What switching a book off takes out of this world, before it is taken. */
export interface SwitchOffReport {
  compendiumId: string;
  bookTitle: string;
  entryCount: number;
  entryNames: string[];
  /** Changes and hides this world made over the book: these go with it. */
  deltas: string[];
  /** What this world added beside the book: these stay (spec 050 decision 5). */
  additionsKept: string[];
  switchedOff: boolean;
}

/**
 * How an entry stands in this world (spec 050 FR-021): as the book has it,
 * changed here, hidden here, or not in the book at all and written here.
 */
export type WorldEntryState = "INHERITED" | "CHANGED" | "HIDDEN" | "ADDED";

/**
 * Where an entry came from, and so whether it may leave the account.
 *
 * Per entry, not per book (spec 050 FR-052, FR-052a): a change to an uploaded
 * entry is uploaded, and an addition beside it is authored.
 */
export type EntryOrigin = "AUTHORED" | "UPLOADED";

/** One entry as this world reads it: the book with the world's changes over it. */
export interface WorldBookEntry {
  /** The book entry's id, or the addition's own for an entry the book lacks. */
  id: string;
  compendiumId: string;
  bookTitle: string;
  kind: string;
  name: string;
  nameUncertain: boolean;
  /** Null for an addition, which is on no page of any book. */
  page: number | null;
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
  suspect: boolean;
  state: WorldEntryState;
  origin: EntryOrigin;
  mayBeShared: boolean;
  notShareableBecause: string | null;
  /** What the book says, for an entry this world changed (FR-024). */
  before: {
    fieldValues: Record<string, ReadValue>;
    proseText: string | null;
  } | null;
  /** Another entry in the book shares this kind and name, so it cannot be
   * changed (FR-025a). */
  ambiguous: boolean;
}

/** A change this world holds that is not being applied, and why. */
export interface UnattachedDelta {
  kind: string;
  name: string;
  form: string;
  reason: string;
}

export interface WorldBookEntryPage {
  entries: WorldBookEntry[];
  nextCursor: string | null;
  total: number;
  unattached: UnattachedDelta[];
}

const BOOK_FIELDS = `
  compendiumId
  bookTitle
  systemId
  entryTotal
  entryCounts { kind count }
  systemMatches
  baseParserVersion
  switchedOnAt
`;

const ENTRY_FIELDS = `
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
  state
  origin
  mayBeShared
  notShareableBecause
  before { fieldValues proseText }
  ambiguous
`;

/** Which books this world is running (FR-030, FR-035). */
export async function worldBookList(worldId: string): Promise<WorldBook[]> {
  const data = await postGraphQL<{ worldBookList: WorldBook[] }>(
    `query WorldBookList($worldId: UUID!) {
       worldBookList(worldId: $worldId) { ${BOOK_FIELDS} }
     }`,
    { worldId },
  );
  return data.worldBookList;
}

/**
 * What the world's owner could switch on and has not: their own shelf,
 * narrowed to this world's system (FR-030, FR-041).
 *
 * Asked only by a Game Master. A co-Game Master is refused rather than given
 * an empty list, because an empty list would read as "you have no books".
 */
export async function booksOffered(worldId: string): Promise<OfferedBook[]> {
  const data = await postGraphQL<{ compendiumsOfferedToWorld: OfferedBook[] }>(
    `query BooksOffered($worldId: UUID!) {
       compendiumsOfferedToWorld(worldId: $worldId) {
         id
         bookTitle
         systemId
         entryTotal
         importedAt
       }
     }`,
    { worldId },
  );
  return data.compendiumsOfferedToWorld;
}

/**
 * Switch a book on, and get the whole list back.
 *
 * Nothing is copied: the server writes a row saying this world reads that
 * book, and the content is fetched from the shelf whenever it is shown.
 */
export async function switchOn(
  worldId: string,
  compendiumId: string,
): Promise<WorldBook[]> {
  const data = await postGraphQL<{ switchOnCompendium: WorldBook[] }>(
    `mutation SwitchOn($worldId: UUID!, $compendiumId: UUID!) {
       switchOnCompendium(worldId: $worldId, compendiumId: $compendiumId) {
         ${BOOK_FIELDS}
       }
     }`,
    { worldId, compendiumId },
  );
  return data.switchOnCompendium;
}

/**
 * Ask what switching a book off would take, or take it (FR-013, FR-032).
 *
 * `confirm: false` changes nothing. The two calls are not one call with a
 * flag by accident: because content is fetched rather than copied, switching
 * a book off reaches a live table, and that has to be said before it happens
 * rather than discovered by a player whose sword vanished.
 */
export async function switchOff(
  worldId: string,
  compendiumId: string,
  confirm: boolean,
): Promise<SwitchOffReport> {
  const data = await postGraphQL<{ switchOffCompendium: SwitchOffReport }>(
    `mutation SwitchOff(
       $worldId: UUID!
       $compendiumId: UUID!
       $confirm: Boolean!
     ) {
       switchOffCompendium(
         worldId: $worldId
         compendiumId: $compendiumId
         confirm: $confirm
       ) {
         compendiumId
         bookTitle
         entryCount
         entryNames
         deltas
         additionsKept
         switchedOff
       }
     }`,
    { worldId, compendiumId, confirm },
  );
  return data.switchOffCompendium;
}

/**
 * Browse one switched-on book from inside the world (049 FR-042).
 *
 * The fetch itself: these entries are read from the account's shelf at the
 * moment they are asked for. There is no copy in this world to read instead,
 * which is why switching the book off makes them gone.
 */
export async function entriesFrom(
  worldId: string,
  compendiumId: string,
  options: {
    kind?: string | null;
    after?: string | null;
    first?: number;
    showHidden?: boolean;
  } = {},
): Promise<WorldBookEntryPage> {
  const data = await postGraphQL<{
    worldCompendiumEntries: WorldBookEntryPage;
  }>(
    `query WorldCompendiumEntries(
       $worldId: UUID!
       $compendiumId: UUID!
       $kind: String
       $after: String
       $first: Int
       $showHidden: Boolean
     ) {
       worldCompendiumEntries(
         worldId: $worldId
         compendiumId: $compendiumId
         kind: $kind
         after: $after
         first: $first
         showHidden: $showHidden
       ) {
         total
         nextCursor
         entries { ${ENTRY_FIELDS} }
         unattached { kind name form reason }
       }
     }`,
    {
      worldId,
      compendiumId,
      kind: options.kind ?? null,
      after: options.after ?? null,
      first: options.first ?? null,
      showHidden: options.showHidden ?? false,
    },
  );
  return data.worldCompendiumEntries;
}

/** Where in a world a change lands: one book, one entry by kind and name. */
export interface EntryAddress {
  worldId: string;
  compendiumId: string;
  kind: string;
  name: string;
}

/** An entry's content: fields or prose, never both (049 FR-001b). */
export type EntryContent =
  | { fieldValues: Record<string, ReadValue> }
  | { proseText: string };

const ADDRESS_ARGS = `
  $worldId: UUID!
  $compendiumId: UUID!
  $kind: String!
  $name: String!
`;
const ADDRESS = `
  worldId: $worldId
  compendiumId: $compendiumId
  kind: $kind
  name: $name
`;

function contentVariables(content: EntryContent) {
  return "fieldValues" in content
    ? { fieldValues: content.fieldValues, proseText: null }
    : { fieldValues: null, proseText: content.proseText };
}

/**
 * Change what an entry says in this world, and nowhere else (spec 050 FR-020,
 * FR-023). Only the fields named are touched, and the server keeps only what
 * differs from the book.
 */
export async function changeEntry(
  address: EntryAddress,
  content: EntryContent,
): Promise<WorldBookEntry | null> {
  const data = await postGraphQL<{ changeWorldEntry: WorldBookEntry | null }>(
    `mutation ChangeWorldEntry(${ADDRESS_ARGS} $fieldValues: JSON $proseText: String) {
       changeWorldEntry(${ADDRESS} fieldValues: $fieldValues proseText: $proseText) {
         ${ENTRY_FIELDS}
       }
     }`,
    { ...address, ...contentVariables(content) },
  );
  return data.changeWorldEntry;
}

/** Stop showing a book's entry in this world. Every other world keeps it. */
export async function hideEntry(
  address: EntryAddress,
): Promise<WorldBookEntry | null> {
  const data = await postGraphQL<{ hideWorldEntry: WorldBookEntry | null }>(
    `mutation HideWorldEntry(${ADDRESS_ARGS}) {
       hideWorldEntry(${ADDRESS}) { ${ENTRY_FIELDS} }
     }`,
    { ...address },
  );
  return data.hideWorldEntry;
}

/**
 * Write an entry into this world beside the book. It is authored — written
 * here by a person — and so shareable, whatever the book is (FR-052a).
 */
export async function addEntry(
  address: EntryAddress,
  content: EntryContent,
): Promise<WorldBookEntry | null> {
  const data = await postGraphQL<{ addWorldEntry: WorldBookEntry | null }>(
    `mutation AddWorldEntry(${ADDRESS_ARGS} $fieldValues: JSON $proseText: String) {
       addWorldEntry(${ADDRESS} fieldValues: $fieldValues proseText: $proseText) {
         ${ENTRY_FIELDS}
       }
     }`,
    { ...address, ...contentVariables(content) },
  );
  return data.addWorldEntry;
}

/** Put an entry back as the book has it, or take an addition out (FR-024). */
export async function restoreEntry(
  address: EntryAddress,
): Promise<WorldBookEntry | null> {
  const data = await postGraphQL<{ restoreWorldEntry: WorldBookEntry | null }>(
    `mutation RestoreWorldEntry(${ADDRESS_ARGS}) {
       restoreWorldEntry(${ADDRESS}) { ${ENTRY_FIELDS} }
     }`,
    { ...address },
  );
  return data.restoreWorldEntry;
}

/**
 * What this world added beside books it has since switched off (spec 050
 * decision 5). Kept, because the world's own writing never needed the book;
 * each names the book it was written beside in `bookTitle`, and rejoins that
 * book's page if the book is switched back on.
 */
export async function additionsWithoutBook(
  worldId: string,
): Promise<WorldBookEntry[]> {
  const data = await postGraphQL<{
    worldAdditionsWithoutBook: WorldBookEntry[];
  }>(
    `query WorldAdditionsWithoutBook($worldId: UUID!) {
       worldAdditionsWithoutBook(worldId: $worldId) { ${ENTRY_FIELDS} }
     }`,
    { worldId },
  );
  return data.worldAdditionsWithoutBook;
}
