/**
 * Reading a PDF on this machine, rather than sending it to the server.
 *
 * # Who may, and why that is the whole argument
 *
 * A **source book is read into a world by its Game Master, from their own
 * world panel.** That is what makes parsing here safe: a Game Master can
 * already put anything they like into their own world through the authoring
 * tools, so doing the reading on their machine hands them nothing they did
 * not already have. What it buys is the instance's compute and the round
 * trip — a 70 MB book never leaves the desk it is sitting on.
 *
 * A **player importing their own character sheet** is a different question,
 * and the answer is not "trust the parse". It is spec 048's guards: content
 * the world has not adopted is withheld from the play field, a Game Master
 * can roll an import back, and an attempt to use what was never delivered is
 * refused by the server and reported. Those hold whether the parse happened
 * here or on the server, because none of them ever trusted the parse.
 *
 * # One parser
 *
 * `@thunderforge/pdf` is `crates/thunderforge-pdf` compiled for the browser —
 * the same code the server runs. Two implementations of "what does this book
 * say" would be the worst of both.
 */

export interface PdfLine {
  text: string;
  page: number;
  size: number;
  bold: boolean;
  italic: boolean;
  /** Set larger or bolder than the body around it. */
  heading: boolean;
  /**
   * The text is not to be trusted — letter-spaced, or from a font this build
   * cannot decode. Shown to a person rather than hidden: a value somebody can
   * check is worth more than one quietly presented as fact.
   */
  suspect: boolean;
}

export interface PdfDocument {
  pages: number;
  lines: PdfLine[];
  /** Pages that yielded no text. A book that is entirely scans reports all. */
  silentPages: number;
  /** The cross-reference table had to be rebuilt to open it. */
  repaired: boolean;
}

type PdfModule = {
  default: (init?: unknown) => Promise<unknown>;
  read_pdf: (bytes: Uint8Array, from: number, count: number) => string;
  pdf_page_count: (bytes: Uint8Array) => number;
  pdf_outline: (bytes: Uint8Array) => string;
};

let loading: Promise<PdfModule> | null = null;

/**
 * Load the reader, once.
 *
 * Lazily, and never as part of the application's own bundle: it is half a
 * megabyte that only somebody importing a book ever needs, and a player who
 * never imports anything should not download it to play.
 */
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

/** How many pages, without reading any of them. */
export async function pageCount(file: Uint8Array): Promise<number> {
  const module = await load();
  return module.pdf_page_count(file);
}

/**
 * Read part of a book.
 *
 * Paged on purpose. A 350-page book is seconds of arithmetic, and a browser
 * tab that stops answering is a worse experience than a slow import — so a
 * caller walks it a chunk at a time and can show progress while it does.
 */
export async function readPages(
  file: Uint8Array,
  from: number,
  count: number,
): Promise<PdfDocument> {
  const module = await load();
  const parsed = JSON.parse(module.read_pdf(file, from, count)) as {
    pages: number;
    lines: PdfLine[];
    silent_pages: number;
    repaired: boolean;
  };
  return {
    pages: parsed.pages,
    lines: parsed.lines,
    silentPages: parsed.silent_pages,
    repaired: parsed.repaired,
  };
}

/** A document's own table of contents. Absent in most real books. */
export async function outline(
  file: Uint8Array,
): Promise<{ title: string; level: number; page: number | null }[]> {
  const module = await load();
  return JSON.parse(module.pdf_outline(file)) as {
    title: string;
    level: number;
    page: number | null;
  }[];
}
