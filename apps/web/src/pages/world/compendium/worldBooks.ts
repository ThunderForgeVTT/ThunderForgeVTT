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
  deltas: string[];
  switchedOff: boolean;
}

/** One fetched entry, naming the book it came from and the page it was on. */
export interface WorldBookEntry {
  id: string;
  compendiumId: string;
  bookTitle: string;
  kind: string;
  name: string;
  nameUncertain: boolean;
  page: number;
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
  suspect: boolean;
}

export interface WorldBookEntryPage {
  entries: WorldBookEntry[];
  nextCursor: string | null;
  total: number;
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
  options: { kind?: string | null; after?: string | null; first?: number } = {},
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
     ) {
       worldCompendiumEntries(
         worldId: $worldId
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
      worldId,
      compendiumId,
      kind: options.kind ?? null,
      after: options.after ?? null,
      first: options.first ?? null,
    },
  );
  return data.worldCompendiumEntries;
}
