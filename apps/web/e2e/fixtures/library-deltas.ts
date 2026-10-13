import { execFileSync } from "node:child_process";
import { expect, type Page } from "./test";
import { graphql } from "./helpers";

/**
 * The book, the database reads and the world helpers shared by the
 * `library-deltas-*` specs (spec 050 US2).
 *
 * The book is built here rather than shipped, following
 * `library-book-list.spec.ts`: a real book is copyrighted, and a binary
 * fixture tells nobody what it contains.
 */

export function pdfOf(lines: string[]): string {
  const content = lines.join("\n");
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  ];

  let pdf = "%PDF-1.4\n";
  const offsets: number[] = [];
  objects.forEach((body, index) => {
    offsets.push(pdf.length);
    pdf += `${index + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets) {
    pdf += `${offset.toString().padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return pdf;
}

/** Two creatures, as the dnd5e creature pattern declares them. */
export const MONSTERS = pdfOf([
  "BT /F1 18 Tf 72 720 Td (ADULT RED DRAGON) Tj ET",
  "BT /F1 9 Tf 72 700 Td (Gargantuan dragon, chaotic evil) Tj ET",
  "BT /F1 9 Tf 72 686 Td (Armor Class 22) Tj ET",
  "BT /F1 9 Tf 72 672 Td (Hit Points 546) Tj ET",
  "BT /F1 9 Tf 72 658 Td (Speed 40 ft., fly 80 ft.) Tj ET",
  "BT /F1 18 Tf 72 600 Td (GOBLIN) Tj ET",
  "BT /F1 9 Tf 72 580 Td (Small humanoid, neutral evil) Tj ET",
  "BT /F1 9 Tf 72 566 Td (Armor Class 15) Tj ET",
  "BT /F1 9 Tf 72 552 Td (Hit Points 7) Tj ET",
]);

export const SYSTEM = "dnd5e";
export const BOOK = "Monster Manual.pdf";
export const ADDED = "Mire Hag";

export type Gql<T> = { data?: T; errors?: { message: string }[] };

export type ReadValue = { state: string; value?: string };

export interface Entry {
  id: string;
  kind: string;
  name: string;
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
  state: string;
  origin: string;
  mayBeShared: boolean;
  before: { fieldValues: Record<string, ReadValue> } | null;
}

export const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function uuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`Refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/**
 * Reads this shard's database, following `library-book-list.spec.ts`.
 *
 * **Read-only, and only to measure.** Everything this spec changes it changes
 * through the product. Whether the base is byte-identical is the evidence the
 * central claim rests on, and asking the product to vouch for its own storage
 * would be asking the thing under test to mark its own work.
 */
export function sql(statement: string): string {
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-U",
      dbUser,
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
      "-t",
      "-A",
    ],
    { input: statement, encoding: "utf-8", stdio: ["pipe", "pipe", "inherit"] },
  ).trim();
}

/**
 * Every stored byte of a book's entries, in a stable order, as Postgres's own
 * text rendering of each whole row — id, name, fields, prose, page, timestamps
 * and all. Equal strings are the same bytes; any write to any column of any
 * entry changes it.
 */
export function baseAsStored(compendiumId: string): string {
  return sql(
    `SELECT string_agg(e::text, E'\\n' ORDER BY e.id)
       FROM compendium_entries e
      WHERE e.compendium_id = '${uuid(compendiumId)}';`,
  );
}

/** The origin column of each delta a world holds, by entry name. */
export function deltaOrigins(worldId: string): Record<string, string> {
  const rows = sql(
    `SELECT name || '|' || form || '|' || origin FROM world_entry_deltas
      WHERE world_id = '${uuid(worldId)}' ORDER BY name;`,
  );
  return Object.fromEntries(
    rows
      .split("\n")
      .filter(Boolean)
      .map((row) => {
        const [name, form, origin] = row.split("|");
        return [name, `${form} ${origin}`];
      }),
  );
}

export function deltaCount(worldId: string): number {
  return Number(
    sql(
      `SELECT count(*) FROM world_entry_deltas WHERE world_id = '${uuid(worldId)}';`,
    ),
  );
}

/** Read the book in from the shelf, and return its compendium id. */
export async function readInTheBook(page: Page): Promise<string> {
  await page.goto("/library");
  await page.getByTestId("import-system").selectOption(SYSTEM);
  await page.getByTestId("import-file").setInputFiles({
    name: BOOK,
    mimeType: "application/pdf",
    buffer: Buffer.from(MONSTERS, "latin1"),
  });
  await expect(page.getByTestId("found-total")).toBeVisible({
    timeout: 120_000,
  });
  await page.getByTestId("submit-import").click();
  await expect(page.getByTestId("library-shelf")).toContainText(BOOK, {
    timeout: 60_000,
  });

  const href = await page
    .getByTestId("library-shelf")
    .getByTestId("open-book")
    .first()
    .getAttribute("href");
  const id = /\/library\/([^/]+)$/.exec(href ?? "")?.[1];
  expect(id, "the shelf should link to the book").toBeTruthy();
  return id as string;
}

/** A world on the book's system with the book switched on. */
export async function aWorldRunning(
  page: Page,
  name: string,
  compendiumId: string,
): Promise<string> {
  const made = await graphql<Gql<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name, gameSystemId: SYSTEM } },
  );
  const worldId = made.data?.createWorld?.id;
  expect(worldId, JSON.stringify(made.errors)).toBeTruthy();

  const on = await graphql<Gql<unknown>>(
    page,
    `
      mutation On($w: UUID!, $c: UUID!) {
        switchOnCompendium(worldId: $w, compendiumId: $c) {
          compendiumId
        }
      }
    `,
    { w: worldId, c: compendiumId },
  );
  expect(on.errors, JSON.stringify(on.errors)).toBeUndefined();
  return worldId as string;
}

/** What a world reads from the book, through the product, hidden included. */
export async function worldReads(
  page: Page,
  worldId: string,
  compendiumId: string,
): Promise<Entry[]> {
  const read = await graphql<
    Gql<{ worldCompendiumEntries: { entries: Entry[] } }>
  >(
    page,
    `
      query E($w: UUID!, $c: UUID!) {
        worldCompendiumEntries(
          worldId: $w
          compendiumId: $c
          showHidden: true
        ) {
          entries {
            id
            kind
            name
            fieldValues
            proseText
            state
            origin
            mayBeShared
            before {
              fieldValues
            }
          }
        }
      }
    `,
    { w: worldId, c: compendiumId },
  );
  expect(read.errors, JSON.stringify(read.errors)).toBeUndefined();
  return read.data?.worldCompendiumEntries.entries ?? [];
}

export function named(entries: Entry[], name: string): Entry | undefined {
  return entries.find((entry) => entry.name === name);
}

/** The first declared field the reader found a value for on this entry. */
export function aReadField(entry: Entry): [string, string] {
  const found = Object.entries(entry.fieldValues).find(
    ([, value]) => value.state !== "unread" && value.value,
  );
  expect(found, JSON.stringify(entry.fieldValues)).toBeTruthy();
  const [field, value] = found as [string, ReadValue];
  return [field, value.value as string];
}

export async function browseTheBook(page: Page, worldId: string) {
  await page.goto(`/world/${worldId}/compendium?tab=books`);
  await expect(page.getByTestId("book-list")).toContainText(BOOK, {
    timeout: 30_000,
  });
  await page.getByTestId("browse-book").click();
  await expect(page.getByTestId("world-book-browser")).toBeVisible();
}

/** One entry's row in the browser, opened. */
export async function openEntry(page: Page, name: string) {
  const row = page
    .getByTestId("world-book-browser")
    .locator(`li[data-name="${name}"]`);
  await expect(row).toHaveCount(1, { timeout: 15_000 });
  const details = row.locator("details");
  if ((await details.getAttribute("open")) === null) {
    await row.locator("summary").click();
  }
  return row;
}
