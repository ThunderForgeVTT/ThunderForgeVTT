/**
 * Sending a reviewed book, and what a Game Master sees while it goes (spec
 * 049 US2, FR-030 to FR-035).
 *
 * # Why this does not use `postGraphQL`
 *
 * Everything else in `src/api` sends a small body and only cares about the
 * reply. This sends some megabytes of entries over whatever connection the
 * Game Master has, and FR-030 asks for progress that reflects **real work
 * sent** rather than an animation. `fetch` cannot report how much of a request
 * body has gone; `XMLHttpRequest.upload` can, and that is the only reason it
 * is here. The transport's other lessons — CSRF, non-JSON replies, every
 * error rather than the first — are carried over deliberately below.
 *
 * # What the bar actually means
 *
 * Reaching the end means **the server now has it and is applying it**, not
 * that it is applied. The commit is one transaction (FR-032), and a
 * transaction has no progress to report: it either happened or it did not. So
 * the last thing progress says is [`ImportPhase.Applying`], and only the
 * resolved promise says the book is on the shelf. Anything that animated a
 * bar to 100% and then sat there would be lying in the most ordinary way a
 * progress bar can.
 *
 * # Abandoning
 *
 * FR-034. An `AbortSignal` stops the upload, and there is nothing to clean up
 * afterwards: the server applies the whole payload in one transaction, so a
 * request that never arrived complete was never applied at all.
 */
import { withCsrf } from "@/api/auth";
import { postGraphQL } from "@/api/graphqlClient";
import type { ContentEntry } from "@/engine/sdk/ContentEntry";

/** The endpoint every GraphQL call in the app uses. */
const GRAPHQL_ENDPOINT = "/api/graphql";

/**
 * The most entries one import may carry (FR-035).
 *
 * Refused here **and** on the server, with the same number and the same
 * reason — the browser refusing first saves a Game Master a long upload, and
 * the server refusing again is what makes it a rule rather than a courtesy.
 */
export const MAX_ENTRIES_PER_IMPORT = 10_000;

/** What a book review approved, ready to send. */
export interface ApprovedBook {
  title: string;
  sha256: string;
  pages: number;
  silentPages: number;
  systemId: string;
  parserVersion: string;
  entries: ContentEntry[];
  /** FR-047: the book on the shelf this re-read replaces, if any. */
  replacesCompendiumId?: string;
}

/** One book, as the shelf holds it. */
export interface Compendium {
  id: string;
  bookTitle: string;
  /** `null` for a collection, which was read from no file. */
  sourceHash: string | null;
  systemId: string;
  origin: "AUTHORED" | "UPLOADED";
  pageCount: number;
  silentPageCount: number;
  entryTotal: number;
  entryCounts: { kind: string; count: number }[];
  /** Which reading of the book is in force: 1, and one more per re-import. */
  baseVersion: number;
}

/** One world's changes a re-read of its book no longer takes (050 FR-027). */
export interface WorldUnattachedDeltas {
  worldId: string;
  worldName: string;
  deltas: { kind: string; name: string; form: string; reason: string }[];
}

/** What a removal would take, or did (FR-044 to FR-046). */
export interface RemovalReport {
  compendiumId: string;
  bookTitle: string;
  entryCount: number;
  inUse: { worldId: string; worldName: string; entryNames: string[] }[];
  handEdited: string[];
  /**
   * Per world, what removal does to that table's own work: changes and hides
   * are lost, additions are kept (spec 050 decision 5). Includes worlds that
   * have switched the book off but kept an addition.
   */
  deltasByWorld: {
    worldId: string;
    worldName: string;
    lost: string[];
    kept: string[];
  }[];
  removed: boolean;
}

/**
 * How far a submission has got, and what it is carrying.
 *
 * `sending` counts bytes the browser has actually handed to the network;
 * `applying` is the wait for one transaction, which has no halfway.
 */
export type ImportProgress =
  | {
      phase: "sending";
      sentBytes: number;
      totalBytes: number;
      /** What is being sent, in words (FR-031). */
      what: string;
    }
  | { phase: "applying"; totalBytes: number; what: string };

/**
 * What is being sent, said rather than counted (FR-031).
 *
 * "3.1 MB" tells a Game Master nothing about their book. "412 creatures and
 * 38 spells from Monster Manual" tells them whether the thing being sent is
 * the thing they reviewed.
 */
export function describeBook(book: ApprovedBook): string {
  const counts = new Map<string, number>();
  for (const entry of book.entries) {
    counts.set(entry.kind, (counts.get(entry.kind) ?? 0) + 1);
  }
  const parts = [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([kind, count]) => `${count} ${kind}${count === 1 ? "" : "s"}`);
  return parts.length === 0
    ? `nothing from ${book.title}`
    : `${parts.join(", ")} from ${book.title}`;
}

/** Thrown when a submission does not become a compendium (FR-033). */
export class ImportFailed extends Error {
  /** Every message the server returned, not only the first. */
  readonly reasons: string[];

  constructor(message: string, reasons: string[] = []) {
    super(message);
    this.name = "ImportFailed";
    this.reasons = reasons.length > 0 ? reasons : [message];
  }
}

/** Thrown when the Game Master abandoned the import (FR-034). */
export class ImportAbandoned extends Error {
  constructor() {
    super("The import was abandoned before it was sent.");
    this.name = "ImportAbandoned";
  }
}

const COMPENDIUM_FIELDS = `
  id
  bookTitle
  sourceHash
  systemId
  origin
  pageCount
  silentPageCount
  entryTotal
  entryCounts { kind count }
  baseVersion
`;

const CREATE_MUTATION = `
  mutation CreateCompendiumFromImport($input: CreateCompendiumFromImportInput!) {
    createCompendiumFromImport(input: $input) {
      ${COMPENDIUM_FIELDS}
    }
  }
`;

/**
 * Turn approved entries into what the mutation takes.
 *
 * `values` and `extras` travel as they are. A field the reader looked for and
 * did not find is `{ state: "unread" }` with no value key, and rebuilding it
 * here is exactly how it would acquire one — so nothing here rebuilds it.
 */
function payloadFrom(book: ApprovedBook): Record<string, unknown> {
  return {
    bookTitle: book.title,
    sourceHash: book.sha256,
    systemId: book.systemId,
    parserVersion: book.parserVersion,
    pageCount: book.pages,
    silentPageCount: book.silentPages,
    replacesCompendiumId: book.replacesCompendiumId ?? null,
    entries: book.entries.map((entry) => ({
      kind: entry.kind,
      name: entry.name,
      nameUncertain: entry.nameState === "uncertain",
      page: entry.page,
      values: entry.values,
      text: entry.text,
      suspect: entry.suspect,
      extras: entry.extras ?? null,
    })),
  };
}

type GraphQLReply = {
  data?: { createCompendiumFromImport?: Compendium };
  errors?: { message?: string }[];
};

function messagesFrom(errors: { message?: string }[] | undefined): string[] {
  return (errors ?? [])
    .map((error) => error.message)
    .filter((message): message is string => Boolean(message));
}

/**
 * Send an approved book, reporting what has actually left the machine.
 *
 * Resolves with the compendium once the server has applied it; rejects with
 * [`ImportFailed`] naming what went wrong (FR-033), or [`ImportAbandoned`] if
 * the caller aborted.
 */
export function submitImport(
  book: ApprovedBook,
  onProgress?: (progress: ImportProgress) => void,
  signal?: AbortSignal,
): Promise<Compendium> {
  if (book.entries.length > MAX_ENTRIES_PER_IMPORT) {
    // Refused before a byte is sent, with the bound named (FR-035). The
    // server refuses the same payload for the same reason.
    return Promise.reject(
      new ImportFailed(
        `This book produced ${book.entries.length} entries, and an import may carry at most ` +
          `${MAX_ENTRIES_PER_IMPORT}. That usually means the system's patterns matched far more ` +
          `than they should rather than that the book is very large.`,
      ),
    );
  }

  const what = describeBook(book);
  const body = JSON.stringify({
    query: CREATE_MUTATION,
    variables: { input: payloadFrom(book) },
  });
  // The body is sent as UTF-8, so its byte length — not its character count —
  // is what the progress events are measured against.
  const totalBytes = new TextEncoder().encode(body).byteLength;

  return new Promise<Compendium>((resolve, reject) => {
    if (signal?.aborted) {
      reject(new ImportAbandoned());
      return;
    }

    const request = new XMLHttpRequest();
    request.open("POST", GRAPHQL_ENDPOINT);
    request.withCredentials = true;
    const headers = withCsrf({ "Content-Type": "application/json" }) as Record<
      string,
      string
    >;
    for (const [name, value] of Object.entries(headers)) {
      request.setRequestHeader(name, value);
    }

    // No timeout, deliberately. A large book over a slow link is legitimately
    // slow, and cutting it off partway would be worse than waiting — the same
    // reasoning the multipart uploader already applies to images.
    request.timeout = 0;

    request.upload.onprogress = (event) => {
      onProgress?.({
        phase: "sending",
        // `event.total` can be 0 when the length is not computable; the body
        // is a string we measured ourselves, so ours is the honest number.
        sentBytes: Math.min(event.loaded, totalBytes),
        totalBytes,
        what,
      });
    };

    request.upload.onload = () => {
      // Everything is sent and the server is inside one transaction. This is
      // the honest end of the bar: it has it, and it is applying it.
      onProgress?.({ phase: "applying", totalBytes, what });
    };

    const abandon = () => {
      request.abort();
      reject(new ImportAbandoned());
    };
    signal?.addEventListener("abort", abandon, { once: true });
    const done = () => signal?.removeEventListener("abort", abandon);

    request.onerror = () => {
      done();
      reject(
        new ImportFailed("Could not reach the server. Check your connection."),
      );
    };

    request.onabort = () => {
      done();
      // The rejection has already been made by `abandon` when the caller
      // aborted; this covers an abort from anywhere else.
      reject(new ImportAbandoned());
    };

    request.onload = () => {
      done();
      let reply: GraphQLReply;
      try {
        reply = JSON.parse(request.responseText) as GraphQLReply;
      } catch {
        // A proxy's HTML error page, or an empty body. The status is the only
        // real information there is, and it is more use than a parse error.
        reject(
          new ImportFailed(
            `The server returned an unreadable response (HTTP ${request.status}).`,
          ),
        );
        return;
      }

      const reasons = messagesFrom(reply.errors);
      const compendium = reply.data?.createCompendiumFromImport;
      if (reasons.length > 0 || !compendium) {
        reject(
          new ImportFailed(
            reasons.length > 0
              ? reasons.join("; ")
              : `The import did not happen (HTTP ${request.status}).`,
            reasons,
          ),
        );
        return;
      }

      resolve(compendium);
    };

    request.send(body);
  });
}

/**
 * Does this account already hold the book in this file? (FR-047, spec 047
 * FR-070 to FR-072.)
 *
 * Asked with a hash and nothing else, **before** the book is read or sent: a
 * hash is not content, and a Game Master who already has this book should
 * learn it in a moment rather than after a long read and a long upload. The
 * answer is about their own library and no other account's.
 *
 * A near-identical file — the same work re-saved — hashes differently and
 * comes back as no match, which is the wanted behaviour rather than a
 * limitation: claiming a false match would overwrite one book with another
 * (spec 047 FR-075).
 */
export async function findBookByHash(
  sha256: string,
): Promise<Compendium | null> {
  const data = await postGraphQL<{
    compendiumForFileHash: Compendium | null;
  }>(
    `query CompendiumForFileHash($sourceHash: String!) {
       compendiumForFileHash(sourceHash: $sourceHash) {
         ${COMPENDIUM_FIELDS}
       }
     }`,
    { sourceHash: sha256 },
  );
  return data.compendiumForFileHash;
}

/**
 * The changes your worlds hold over this book that its reading in force no
 * longer takes, per world (050 FR-027).
 *
 * Asked after a re-import, so the person who re-read the book hears what the
 * re-read stranded instead of each table finding out alone. Nothing is
 * removed: each change stays with its world until somebody there restores it.
 */
export async function unattachedDeltasOf(
  compendiumId: string,
): Promise<WorldUnattachedDeltas[]> {
  const data = await postGraphQL<{
    compendiumUnattachedDeltas: WorldUnattachedDeltas[];
  }>(
    `query CompendiumUnattachedDeltas($id: UUID!) {
       compendiumUnattachedDeltas(id: $id) {
         worldId
         worldName
         deltas { kind name form reason }
       }
     }`,
    { id: compendiumId },
  );
  return data.compendiumUnattachedDeltas;
}

/**
 * Ask what removing a book would take with it. Changes nothing (FR-045).
 *
 * Separate from [`confirmRemoveCompendium`] because naming what is in use
 * *before* it is confirmed cannot be done in one call: a removal that reports
 * what would break and then does it anyway is not a confirmation.
 */
export function previewRemoveCompendium(id: string): Promise<RemovalReport> {
  return removeCompendium(id, false);
}

/** Remove a book and everything that import contributed (FR-044). */
export function confirmRemoveCompendium(id: string): Promise<RemovalReport> {
  return removeCompendium(id, true);
}

async function removeCompendium(
  id: string,
  confirm: boolean,
): Promise<RemovalReport> {
  const data = await postGraphQL<{ removeCompendium: RemovalReport }>(
    `mutation RemoveCompendium($id: UUID!, $confirm: Boolean!) {
       removeCompendium(id: $id, confirm: $confirm) {
         compendiumId
         bookTitle
         entryCount
         inUse { worldId worldName entryNames }
         handEdited
         deltasByWorld { worldId worldName lost kept }
         removed
       }
     }`,
    { id, confirm },
  );
  return data.removeCompendium;
}
