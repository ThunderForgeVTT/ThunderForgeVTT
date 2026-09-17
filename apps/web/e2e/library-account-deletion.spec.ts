import { execFileSync } from "node:child_process";
import { expect, test } from "./fixtures/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Deleting an account leaves nothing of its library (spec 049 T091, T092;
 * spec 050 FR-054, FR-062 to FR-064, FR-083, SC-007).
 *
 * Built through the product: a book read in (with the content agreement
 * checked on the way), a collection started on the shelf, a world running the
 * book with a change and an addition over it. Then the account is deleted
 * from the page a person uses.
 *
 * **By inspection, not by assertion.** Postgres is asked, before and after,
 * how many bytes of rows anywhere still name the library — the bases, their
 * entries, the book-list links, and every delta over a book or kept beside
 * one. Before, each is more than zero; after, every one is zero.
 */

function pdfOf(lines: string[]): string {
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
const MONSTERS = pdfOf([
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

const SYSTEM = "dnd5e";
const BOOK = "Monster Manual.pdf";

type Gql<T> = { data?: T; errors?: { message: string }[] };

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function uuid(value: string): string {
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
function sql(statement: string): string {
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
 * Bytes of every row, anywhere, that names one of `books` — or, for a delta,
 * carries one of `deltaNames`, which are unique to this run so that another
 * test's kept addition beside its own Monster Manual is not counted.
 */
function libraryBytes(
  books: string[],
  deltaNames: string[],
): Record<string, number> {
  const ids = books.map((id) => `'${uuid(id)}'`).join(", ");
  const named = deltaNames
    .map((title) => `'${title.replace(/'/g, "''")}'`)
    .join(", ");
  const bytes = (table: string, where: string) =>
    Number(
      sql(
        `SELECT coalesce(sum(pg_column_size(t.*)), 0) FROM ${table} t WHERE ${where};`,
      ),
    );
  return {
    bases: bytes("compendiums", `t.id IN (${ids})`),
    entries: bytes("compendium_entries", `t.compendium_id IN (${ids})`),
    bookLists: bytes("world_books", `t.compendium_id IN (${ids})`),
    deltas: bytes(
      "world_entry_deltas",
      `t.compendium_id IN (${ids}) OR t.name IN (${named})`,
    ),
    // A collection's earlier versions (spec 050 FR-104) hold its entries too.
    versions: bytes(
      "shelf_collection_versions",
      `t.compendium_id IN (${ids})`,
    ),
  };
}

test("deleting an account from its page leaves zero bytes of its library", async ({
  page,
}) => {
  test.setTimeout(300_000);
  const creds = freshCredentials("librarygone");
  await register(page, creds);

  // A book read in, and FR-054's agreement said on the way.
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
  const agreement = page.getByTestId("import-content-agreement");
  await expect(agreement).toContainText("for you and your own games");
  await expect(agreement).toContainText("never shared with another account");
  await expect(agreement).toContainText("No copy is kept anywhere else");
  await page.getByTestId("submit-import").click();
  const bookRow = page
    .getByTestId("library-shelf")
    .locator('li[data-testid^="library-book-"]')
    .filter({ hasText: BOOK });
  await expect(bookRow).toHaveCount(1, { timeout: 60_000 });
  const bookId = /\/library\/([^/]+)$/.exec(
    (await bookRow.getByTestId("open-book").getAttribute("href")) ?? "",
  )?.[1] as string;

  // A collection, started on the page, with an entry in it.
  const collectionTitle = `Fen Folk ${creds.username}`;
  await page.getByTestId("new-collection-title").fill(collectionTitle);
  await page.getByTestId("new-collection-system").selectOption(SYSTEM);
  await page.getByTestId("new-collection-submit").click();
  const collectionRow = page
    .getByTestId("library-shelf")
    .locator('li[data-testid^="library-book-"]')
    .filter({ hasText: collectionTitle });
  await expect(collectionRow).toHaveCount(1, { timeout: 15_000 });
  const collectionId = /\/library\/([^/]+)$/.exec(
    (await collectionRow.getByTestId("open-book").getAttribute("href")) ?? "",
  )?.[1] as string;

  // A world running both, with a change and an addition over the book.
  const made = await graphql<Gql<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: "The Doomed Table", gameSystemId: SYSTEM } },
  );
  const worldId = made.data?.createWorld?.id as string;
  expect(worldId, JSON.stringify(made.errors)).toBeTruthy();
  const bookAddition = `Bog Wight ${creds.username}`;
  for (const [query, variables] of [
    [
      `mutation W($c: UUID!) { writeShelfCollectionEntry(collectionId: $c, kind: "creature", name: "Mire Hag", proseText: "Lives in the fen.") { id } }`,
      { c: collectionId },
    ],
    [
      `mutation On($w: UUID!, $c: UUID!) { switchOnCompendium(worldId: $w, compendiumId: $c) { compendiumId } }`,
      { w: worldId, c: bookId },
    ],
    [
      `mutation On($w: UUID!, $c: UUID!) { switchOnCompendium(worldId: $w, compendiumId: $c) { compendiumId } }`,
      { w: worldId, c: collectionId },
    ],
    [
      `mutation H($w: UUID!, $c: UUID!) { hideWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "GOBLIN") { state } }`,
      { w: worldId, c: bookId },
    ],
    [
      `mutation A($w: UUID!, $c: UUID!, $n: String!) { addWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: $n, proseText: "It was a traveller once.") { state } }`,
      { w: worldId, c: bookId, n: bookAddition },
    ],
  ] as const) {
    const done = await graphql<Gql<unknown>>(page, query, variables);
    expect(done.errors, JSON.stringify(done.errors)).toBeUndefined();
  }

  const books = [bookId, collectionId];
  const deltaNames = [bookAddition];
  const before = libraryBytes(books, deltaNames);
  for (const [table, size] of Object.entries(before)) {
    expect(size, `${table} holds the library before`).toBeGreaterThan(0);
  }

  // Deleted from the page a person uses, which says what it will do.
  await page.goto("/counter");
  await page.getByRole("button", { name: "Delete account" }).click();
  await expect(page.getByTestId("delete-account-consequence")).toContainText(
    "Nothing of your library is kept",
  );
  await page.getByRole("button", { name: "Delete permanently" }).click();
  await page.waitForURL(/\/login/, { timeout: 30_000 });

  expect(
    sql(`SELECT count(*) FROM users WHERE username = '${creds.username}';`),
  ).toBe("0");
  expect(libraryBytes(books, deltaNames)).toEqual({
    bases: 0,
    entries: 0,
    bookLists: 0,
    deltas: 0,
    versions: 0,
  });
});
