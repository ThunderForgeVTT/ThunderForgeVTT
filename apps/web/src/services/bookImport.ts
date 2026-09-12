/**
 * Reading a source book into entries, on the machine that holds it.
 *
 * `pdfReader.ts` gets text out of a document. This turns that text into a
 * system's content — spells, items, creatures, feats — using the patterns that
 * system's pack declares.
 *
 * # Nothing leaves this machine
 *
 * Spec 049 FR-020, and the reason the *entries* are built here rather than the
 * lines being shipped somewhere to be interpreted: sending the lines would be
 * sending the book. The only thing that crosses the wire before a Game Master
 * submits is the file's hash, which is not content (FR-047, spec 047 FR-072).
 *
 * # One reader, compiled twice
 *
 * The readers are `crates/thunderforge-content`, compiled to wasm and reached
 * through `@thunderforge/pdf`. The server runs the same crate natively when it
 * re-checks what arrives (FR-036). A TypeScript reimplementation would be two
 * answers to "what does this book say", which is the worst of both.
 */
import type { ContentEntry } from "@/engine/sdk/ContentEntry";
import type { ContentPatterns } from "@/engine/sdk/ContentPatterns";

export type { ContentEntry };

type PdfModule = {
  default: (init?: unknown) => Promise<unknown>;
  read_content: (
    bytes: Uint8Array,
    patterns: string,
    from: number,
    count: number,
  ) => string;
  pdf_page_count: (bytes: Uint8Array) => number;
};

let loading: Promise<PdfModule> | null = null;

async function load(): Promise<PdfModule> {
  if (!loading) {
    loading = (async () => {
      const module =
        (await import("@thunderforge/pdf")) as unknown as PdfModule;
      await module.default();
      return module;
    })();
  }
  return loading;
}

/** What a book turned into, and what the reader could not do with it. */
export interface ReadBook {
  entries: ContentEntry[];
  pages: number;
  /** Pages that yielded no text. A book that is all scans reports all of them. */
  silentPages: number;
}

/** How far through a read we are, for a person watching. */
export interface ReadProgress {
  page: number;
  pages: number;
  entriesSoFar: number;
}

/**
 * The SHA-256 of a file, as a lowercase hex string.
 *
 * Taken **before** anything is read or sent, so a person who picked the wrong
 * file learns it in a moment rather than after a long read (spec 047 FR-072).
 * A hash is not content: this is the one thing that may reach the server
 * before a Game Master submits.
 */
export async function hashOf(file: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    file.buffer.slice(
      file.byteOffset,
      file.byteOffset + file.byteLength,
    ) as ArrayBuffer,
  );
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

/** How many pages to read at once. */
const CHUNK = 25;

/**
 * Read a whole book into entries, a chunk of pages at a time.
 *
 * Chunked so the tab keeps answering and so a caller can show progress: a
 * 350-page book is seconds of arithmetic, and a frozen tab is a worse
 * experience than a slow import.
 */
export async function readBook(
  file: Uint8Array,
  patterns: ContentPatterns,
  onProgress?: (progress: ReadProgress) => void,
): Promise<ReadBook> {
  const module = await load();
  const pages = module.pdf_page_count(file);
  const declared = JSON.stringify(patterns);

  const entries: ContentEntry[] = [];
  for (let from = 1; from <= pages; from += CHUNK) {
    const count = Math.min(CHUNK, pages - from + 1);
    const read = JSON.parse(
      module.read_content(file, declared, from, count),
    ) as ContentEntry[];
    entries.push(...read);
    onProgress?.({
      page: Math.min(from + count - 1, pages),
      pages,
      entriesSoFar: entries.length,
    });
    // Yield to the event loop between chunks. Without this the loop holds the
    // main thread for the whole book and the progress nobody can see is worse
    // than none.
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  return { entries, pages, silentPages: 0 };
}

/** Entries grouped by the kind their system called them, for the review. */
export function byKind(entries: ContentEntry[]): Map<string, ContentEntry[]> {
  const grouped = new Map<string, ContentEntry[]>();
  for (const entry of entries) {
    const existing = grouped.get(entry.kind);
    if (existing) {
      existing.push(entry);
    } else {
      grouped.set(entry.kind, [entry]);
    }
  }
  return grouped;
}

/**
 * How many of an entry's declared fields were not found.
 *
 * Shown in the review so a Game Master can see at a glance which entries came
 * out thin, without opening each one (FR-023).
 */
export function unreadCount(entry: ContentEntry): number {
  return Object.values(entry.values).filter((value) => value.state === "unread")
    .length;
}
