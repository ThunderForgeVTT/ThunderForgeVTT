import { expect, test, type Page } from "./fixtures/test";
import {
  register,
  freshCredentials,
  registerAndCreateWorld,
} from "./fixtures/helpers";

/**
 * The account's shelf, end to end (spec 049 US3 T050, spec 050 US1).
 *
 * What the server tests prove is that the library reads correctly and that one
 * account cannot reach another's. What only a browser can prove is the rest of
 * the claim: that a Game Master reaches the importer **from their own shelf**,
 * that two books become two compendiums with their own counts, that a file
 * they already hold is recognised **from a different world**, and that a
 * removal names what is in use before it takes anything.
 *
 * The documents are built here rather than shipped as fixtures, following
 * `book-import-review.spec.ts`: a real book is copyrighted, and a binary
 * fixture tells nobody what it contains. These say exactly what the readers
 * are being asked to find, and they are written to match what the bundled
 * dnd5e pack declares — the creature pattern anchors on "Armor Class", the
 * spell pattern on "Casting Time".
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

/** A different book entirely: one spell, by the spell pattern's anchor. */
const SPELLS = pdfOf([
  "BT /F1 18 Tf 72 720 Td (FIREBALL) Tj ET",
  "BT /F1 9 Tf 72 700 Td (Casting Time 1 action) Tj ET",
  "BT /F1 9 Tf 72 686 Td (Range 150 feet) Tj ET",
  "BT /F1 9 Tf 72 672 Td (Components V, S, M) Tj ET",
  "BT /F1 9 Tf 72 658 Td (Duration Instantaneous) Tj ET",
]);

function asFile(name: string, pdf: string) {
  return {
    name,
    mimeType: "application/pdf",
    buffer: Buffer.from(pdf, "latin1"),
  };
}

/** Choose the system, hand over a file, and wait for the read to finish. */
async function offer(page: Page, name: string, pdf: string) {
  await page.getByTestId("import-system").selectOption("dnd5e");
  await page.getByTestId("import-file").setInputFiles(asFile(name, pdf));
}

/** Read a book in from the shelf, all the way to the shelf again. */
async function readIn(page: Page, name: string, pdf: string) {
  await offer(page, name, pdf);
  await expect(page.getByTestId("found-total")).toBeVisible({
    timeout: 120_000,
  });
  await page.getByTestId("submit-import").click();
  await expect(page.getByTestId("import-review")).toHaveCount(0, {
    timeout: 120_000,
  });
  await expect(page.getByTestId("library-shelf")).toContainText(name, {
    timeout: 60_000,
  });
}

test.describe.configure({ mode: "serial" });

test.describe("The account's library (spec 049 US3, spec 050 US1)", () => {
  test("two books, a duplicate caught from another world, and a removal that says what it takes", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    // A Game Master with a world. The world matters only to prove it does
    // not: nothing about the shelf is scoped to it.
    const firstWorld = await registerAndCreateWorld(
      page,
      "Crypt of the Cartographer",
      "library",
    );

    await page.goto("/library");
    await expect(page.getByTestId("library-empty")).toBeVisible();

    await readIn(page, "Monster Manual.pdf", MONSTERS);
    await readIn(page, "Player's Handbook.pdf", SPELLS);

    // 049 US3 acceptance 1: two imports are two compendiums, each with its
    // own name and its own counts — not one pile.
    const books = page.getByTestId("library-shelf").locator("li");
    await expect(books).toHaveCount(2);
    const manual = books.filter({ hasText: "Monster Manual.pdf" });
    const handbook = books.filter({ hasText: "Player's Handbook.pdf" });
    await expect(manual.getByTestId("book-counts")).toContainText("2 creature");
    await expect(handbook.getByTestId("book-counts")).toContainText("1 spell");
    // FR-051, and what the shelf must say about it: this came out of a
    // document, and it stays here.
    await expect(manual.getByTestId("book-origin")).toContainText(
      "stays with this account",
    );

    // Kept before the shelf is left, since the page it links to is where the
    // rest of this scenario returns.
    const bookHref = await manual.getByTestId("open-book").getAttribute("href");
    expect(bookHref).toBeTruthy();

    // 049 US3 acceptance 2 and 3: open one and see only what came out of it,
    // grouped by kind, with every entry naming its book and page (FR-043).
    await manual.getByTestId("open-book").click();
    await expect(page.getByTestId("book-browser")).toBeVisible();
    await expect(page.getByTestId("entry-count")).toContainText("of 2");
    const dragon = page
      .locator("[data-testid^='entry-']")
      .filter({ hasText: "ADULT RED DRAGON" });
    await expect(dragon).toContainText("Monster Manual.pdf");
    await expect(dragon).toContainText("page 1");
    await dragon.locator("summary").click();
    await expect(dragon.locator("[data-field='armorClass']")).toHaveText("22");
    // A field looked for and not found shows no value, because it has none.
    await expect(dragon.locator("[data-field='challenge']")).toHaveAttribute(
      "data-read-state",
      "unread",
    );

    await page.getByTestId("kind-creature").click();
    await expect(page.getByTestId("entry-count")).toContainText("of 2");
    await expect(
      page.locator("[data-testid^='entry-']").filter({ hasText: "FIREBALL" }),
    ).toHaveCount(0, { timeout: 15_000 });

    // The point of hashing against the account: a second world, and the same
    // file is still recognised. Under a world-scoped check this would be the
    // eighth import of one Monster Manual.
    await page.goto("/worlds/create");
    await page.locator("#world-name").fill("The Second Table");
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 30_000 });
    const secondWorld = /\/world\/([^/]+)\/staging$/.exec(
      new URL(page.url()).pathname,
    )?.[1];
    expect(secondWorld).not.toBe(firstWorld);

    await page.goto("/library");
    await offer(page, "Monster Manual.pdf", MONSTERS);
    await expect(page.getByTestId("import-duplicate")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("import-duplicate")).toContainText(
      "Monster Manual.pdf",
    );
    await page.getByTestId("import-keep").click();
    await expect(page.getByTestId("library-shelf").locator("li")).toHaveCount(
      2,
    );

    // FR-045: what removal would take is named **before** it is confirmed,
    // and asking changes nothing.
    await page
      .getByTestId("library-shelf")
      .locator("li")
      .filter({ hasText: "Player's Handbook.pdf" })
      .getByTestId("remove-book")
      .click();
    await expect(page.getByTestId("removal-report")).toContainText(
      "Player's Handbook.pdf",
    );
    await expect(page.getByTestId("removal-in-use")).toBeVisible();
    await expect(page.getByTestId("library-shelf").locator("li")).toHaveCount(
      2,
    );

    // FR-044: and confirming takes that import's contribution and nothing
    // else — the other book is untouched.
    await page.getByTestId("remove-confirm").click();
    await expect(page.getByTestId("library-shelf").locator("li")).toHaveCount(
      1,
    );
    await expect(page.getByTestId("library-shelf")).toContainText(
      "Monster Manual.pdf",
    );
    expect(bookHref).toBeTruthy();
    await page.goto(bookHref as string);
    await expect(page.getByTestId("entry-count")).toContainText("of 2");
  });

  /**
   * 049 FR-028 as it lands on an account-level surface (T032), and FR-055a:
   * a person reaches their own library and nobody else's. The id is a real
   * one, held by somebody else, and it answers as though there were no such
   * book — the two must be indistinguishable, or an id space can be walked.
   */
  test("somebody else's shelf is not reachable, even with the id", async ({
    browser,
  }) => {
    test.setTimeout(300_000);

    const owner = await browser.newContext();
    const ownerPage = await owner.newPage();
    await registerAndCreateWorld(ownerPage, "A Private Table", "libowner");
    await ownerPage.goto("/library");
    await readIn(ownerPage, "Monster Manual.pdf", MONSTERS);
    const href = await ownerPage
      .getByTestId("library-shelf")
      .getByTestId("open-book")
      .first()
      .getAttribute("href");
    expect(href).toBeTruthy();

    const stranger = await browser.newContext();
    const strangerPage = await stranger.newPage();
    await register(strangerPage, freshCredentials("libstranger"));

    await strangerPage.goto("/library");
    await expect(strangerPage.getByTestId("library-empty")).toBeVisible();
    await expect(strangerPage.getByTestId("library-shelf")).toHaveCount(0);

    await strangerPage.goto(href as string);
    await expect(strangerPage.getByTestId("book-not-yours")).toBeVisible();
    await expect(strangerPage.getByTestId("book-browser")).toHaveCount(0);
    // Nothing of the book — not its title, not an entry — reaches a page it
    // does not belong to.
    await expect(strangerPage.locator("body")).not.toContainText(
      "ADULT RED DRAGON",
    );

    await owner.close();
    await stranger.close();
  });
});
