import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "./fixtures/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Collections on the shelf, end to end (spec 049 T083 to T088, spec 050
 * FR-007 to FR-009c, FR-089b, SC-012).
 *
 * The claims:
 *
 * 1. **One shelf** (FR-007): a collection started on the library page sits
 *    beside a book read in, and says which it is.
 * 2. **A collection is a book in a world** (FR-008, FR-009): offered only to a
 *    world on its system and refused by another, switched on the same way,
 *    read the same way, and changed by a world the same way — with a world's
 *    change over it still authored and shareable.
 * 3. **It downloads, and a book does not** (FR-009a, FR-009b): the file the
 *    browser saves opens as JSON outside ThunderForge and holds what was
 *    written; a book offers no download and the server refuses one asked for
 *    directly.
 *
 * The book is built here rather than shipped, following
 * `library-book-list.spec.ts`: a real book is copyrighted.
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
const COLLECTION = "Fen Folk";

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

/** Read the book in from the shelf, and return its compendium id. */
async function readInTheBook(page: Page): Promise<string> {
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

/** A world on `system`, made the way the other library specs make one. */
async function aWorldOn(page: Page, name: string, system: string) {
  const made = await graphql<Gql<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name, gameSystemId: system } },
  );
  const worldId = made.data?.createWorld?.id;
  expect(worldId, JSON.stringify(made.errors)).toBeTruthy();
  return worldId as string;
}

async function offeredTo(page: Page, worldId: string): Promise<string[]> {
  const offered = await graphql<
    Gql<{ compendiumsOfferedToWorld: { id: string }[] }>
  >(
    page,
    `
      query O($w: UUID!) {
        compendiumsOfferedToWorld(worldId: $w) {
          id
        }
      }
    `,
    { w: worldId },
  );
  expect(offered.errors, JSON.stringify(offered.errors)).toBeUndefined();
  return (offered.data?.compendiumsOfferedToWorld ?? []).map((book) => book.id);
}

function shelfRow(page: Page, title: string) {
  return page
    .getByTestId("library-shelf")
    .locator('li[data-testid^="library-book-"]')
    .filter({ hasText: title });
}

test("a collection sits on the shelf beside a book, is a book in a world, and downloads as JSON where a book does not", async ({
  page,
}) => {
  test.setTimeout(300_000);
  await register(page, freshCredentials("collections"));
  const bookId = await readInTheBook(page);

  // 1. Started on the library page, beside the book.
  await page.getByTestId("new-collection-title").fill(COLLECTION);
  await page.getByTestId("new-collection-system").selectOption(SYSTEM);
  await page.getByTestId("new-collection-submit").click();
  const collectionRow = shelfRow(page, COLLECTION);
  await expect(collectionRow).toHaveCount(1, { timeout: 15_000 });
  await expect(collectionRow).toHaveAttribute("data-origin", "AUTHORED");
  await expect(collectionRow.getByTestId("book-origin")).toContainText(
    "A collection, written in ThunderForge",
  );
  const bookRow = shelfRow(page, BOOK);
  await expect(bookRow).toHaveAttribute("data-origin", "UPLOADED");
  // FR-009b on the page: a book offers no download at all.
  await expect(bookRow.getByTestId("download-collection")).toHaveCount(0);
  await expect(collectionRow.getByTestId("download-collection")).toHaveCount(1);

  const href = await collectionRow
    .getByTestId("open-book")
    .getAttribute("href");
  const collectionId = /\/library\/([^/]+)$/.exec(href ?? "")?.[1] as string;
  expect(collectionId).toMatch(UUID_PATTERN);
  expect(
    sql(
      `SELECT origin || '|' || coalesce(source_hash, 'none') FROM compendiums WHERE id = '${uuid(collectionId)}';`,
    ),
  ).toBe("Authored|none");

  // Written in, through the page: two entries, one taken out again.
  await collectionRow.getByTestId("open-book").click();
  const form = page.getByTestId("collection-entry-form");
  await expect(form).toBeVisible();
  for (const [name, text] of [
    ["Mire Hag", "Lives in the fen and bargains in teeth."],
    ["Bog Wight", "It was a traveller once."],
  ]) {
    await form.getByTestId("collection-entry-kind").selectOption("creature");
    await form.getByTestId("collection-entry-name").fill(name);
    await form.getByTestId("collection-entry-text").fill(text);
    await form.getByTestId("collection-entry-submit").click();
    await expect(page.getByTestId("book-browser")).toContainText(name, {
      timeout: 15_000,
    });
  }
  const wight = page
    .getByTestId("book-browser")
    .locator('li[data-kind="creature"]')
    .filter({ hasText: "Bog Wight" });
  await wight.locator("summary").click();
  await wight.getByTestId("remove-collection-entry").click();
  await expect(page.getByTestId("book-browser")).not.toContainText(
    "Bog Wight",
    {
      timeout: 15_000,
    },
  );
  await expect(page.getByTestId("entry-count")).toHaveText("Showing 1 of 1");

  // 2. In a world, a book: offered only on its system, refused elsewhere.
  const onDnd = await aWorldOn(page, "The Fen Table", SYSTEM);
  const onGenie = await aWorldOn(page, "The Lamp Table", "genie");
  expect(await offeredTo(page, onDnd)).toContain(collectionId);
  expect(await offeredTo(page, onGenie)).not.toContain(collectionId);
  const refused = await graphql<Gql<unknown>>(
    page,
    `
      mutation On($w: UUID!, $c: UUID!) {
        switchOnCompendium(worldId: $w, compendiumId: $c) {
          compendiumId
        }
      }
    `,
    { w: onGenie, c: collectionId },
  );
  expect(refused.errors?.[0]?.message).toMatch(/read as dnd5e/);

  await page.goto(`/world/${onDnd}/compendium?tab=books`);
  await page
    .getByTestId(`offered-book-${collectionId}`)
    .getByTestId("switch-on-book")
    .click();
  await expect(page.getByTestId("book-list")).toContainText(COLLECTION, {
    timeout: 15_000,
  });
  await page.getByTestId("browse-book").click();
  const browser = page.getByTestId("world-book-browser");
  const hag = browser.locator('li[data-name="Mire Hag"]');
  await expect(hag).toHaveCount(1, { timeout: 15_000 });
  await expect(hag).toHaveAttribute("data-state", "INHERITED");
  await expect(hag.getByTestId("world-entry-origin")).toHaveAttribute(
    "data-may-be-shared",
    "true",
  );

  const changed = await graphql<
    Gql<{
      changeWorldEntry: { state: string; origin: string; mayBeShared: boolean };
    }>
  >(
    page,
    `
      mutation C($w: UUID!, $c: UUID!) {
        changeWorldEntry(
          worldId: $w
          compendiumId: $c
          kind: "creature"
          name: "Mire Hag"
          proseText: "She has moved to the river."
        ) {
          state
          origin
          mayBeShared
        }
      }
    `,
    { w: onDnd, c: collectionId },
  );
  expect(changed.errors, JSON.stringify(changed.errors)).toBeUndefined();
  expect(changed.data?.changeWorldEntry).toEqual({
    state: "CHANGED",
    origin: "AUTHORED",
    mayBeShared: true,
  });

  // 3. Downloaded as JSON, saved by the browser, opened outside ThunderForge.
  await page.goto("/library");
  const saving = page.waitForEvent("download");
  await shelfRow(page, COLLECTION).getByTestId("download-collection").click();
  const download = await saving;
  expect(download.suggestedFilename()).toBe(`${COLLECTION}.json`);
  const opened = JSON.parse(readFileSync(await download.path(), "utf-8"));
  expect(opened).toMatchObject({
    format: "thunderforge.collection",
    formatVersion: 1,
    title: COLLECTION,
    systemId: SYSTEM,
    excluded: [],
  });
  expect(opened.entries).toEqual([
    {
      kind: "creature",
      name: "Mire Hag",
      fieldValues: {},
      proseText: "Lives in the fen and bargains in teeth.",
    },
  ]);
  await expect(page.getByTestId("collection-downloaded")).toContainText(
    "Nothing was left out.",
  );

  // And the book, asked for directly: refused, with nothing of it sent.
  const bookDownload = await graphql<
    Gql<{ downloadShelfCollection: { contents: string } }>
  >(
    page,
    `
      query D($id: UUID!) {
        downloadShelfCollection(id: $id) {
          contents
        }
      }
    `,
    { id: bookId },
  );
  expect(bookDownload.data?.downloadShelfCollection ?? null).toBeNull();
  expect(bookDownload.errors?.[0]?.message).toContain("cannot be downloaded");
});
