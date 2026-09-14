import { test, expect } from "./fixtures/test";

/**
 * Spec 049 US1: a book is read on the Game Master's own machine, and nothing
 * read leaves it (FR-020, FR-024).
 *
 * The crate's own tests cover what the readers get out of text. What this
 * proves is the part only a browser can:
 *
 * 1. the declaration-driven readers — the same Rust the server runs — load and
 *    run as WebAssembly, with no server involved in the reading;
 * 2. **nothing carrying book content crosses the wire**, observed on the
 *    network rather than argued from the code.
 *
 * The second is the one that matters. FR-020 is a promise about *absence*, and
 * absence is not provable by inspection: a telemetry call, an analytics beacon
 * or an eager pre-flight added next year by somebody who never read this spec
 * would all pass a code review and break the promise. Only a test that fails
 * when a request body carries an entry keeps it true.
 *
 * The document is built here rather than shipped as a fixture. A real book is
 * copyrighted, and a binary fixture tells nobody what it contains; a PDF
 * written in the test says exactly what the readers are being asked to find.
 */

/** A one-page PDF holding two creature statblocks, built by hand. */
function bookPdf(): string {
  const lines = [
    "BT /F1 18 Tf 72 720 Td (ADULT RED DRAGON) Tj ET",
    "BT /F1 9 Tf 72 700 Td (Gargantuan dragon, chaotic evil) Tj ET",
    "BT /F1 9 Tf 72 686 Td (Armor Class 22) Tj ET",
    "BT /F1 9 Tf 72 672 Td (Hit Points 546) Tj ET",
    "BT /F1 9 Tf 72 658 Td (Speed 40 ft., fly 80 ft.) Tj ET",
    "BT /F1 18 Tf 72 600 Td (GOBLIN) Tj ET",
    "BT /F1 9 Tf 72 580 Td (Small humanoid, neutral evil) Tj ET",
    "BT /F1 9 Tf 72 566 Td (Armor Class 15) Tj ET",
    "BT /F1 9 Tf 72 552 Td (Hit Points 7) Tj ET",
  ].join("\n");
  const stream = `<< /Length ${lines.length} >>\nstream\n${lines}\nendstream`;

  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    stream,
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

/**
 * A two-page PDF whose second page carries no text at all.
 *
 * What an image scan looks like to the reader: the page exists, it has a
 * content stream, and nothing in it decodes to a word. 51 of the 246 books
 * measured are like this from cover to cover.
 */
function partlyScannedPdf(): string {
  const text = [
    "BT /F1 18 Tf 72 720 Td (GOBLIN) Tj ET",
    "BT /F1 9 Tf 72 700 Td (Armor Class 15) Tj ET",
    "BT /F1 9 Tf 72 686 Td (Hit Points 7) Tj ET",
  ].join("\n");
  const blank = "";

  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 7 0 R >> >> >>",
    `<< /Length ${text.length} >>\nstream\n${text}\nendstream`,
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 6 0 R /Resources << >> >>",
    `<< /Length ${blank.length} >>\nstream\n${blank}\nendstream`,
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

/**
 * A book with no text anywhere in it.
 *
 * What 51 of the 246 measured books are from cover to cover: pages that exist,
 * with nothing on them that decodes to a word. The review must call this
 * unreadable rather than show it as an import that happened to find nothing
 * (FR-005).
 */
function scannedPdf(): string {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << >> >>",
    "<< /Length 0 >>\nstream\n\nendstream",
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

/** A creature pattern, exactly as a system pack declares one. */
const PATTERNS = {
  patterns: [
    {
      kind: "creature",
      shape: "anchored",
      anchor: "Armor Class",
      name: { position: "before", withinLines: 6, prefer: "largest" },
      fields: [
        { key: "armorClass", label: "Armor Class", as: "integer" },
        { key: "hitPoints", label: "Hit Points", as: "integer" },
        { key: "speed", label: "Speed", as: "text" },
        { key: "challenge", label: "Challenge", as: "text" },
      ],
    },
  ],
};

test.describe("Reading a book into entries, in the browser (spec 049 US1)", () => {
  test("the declaration finds a system's content with no server involved", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto("/");

    const result = await page.evaluate(
      async ([source, patterns]) => {
        const reader = (await import(
          /* @vite-ignore */ "/src/services/bookImport.ts"
        )) as typeof import("../src/services/bookImport");

        const bytes = Uint8Array.from(source as string, (character) =>
          character.charCodeAt(0),
        );
        const book = await reader.readBook(
          bytes,
          JSON.parse(patterns as string),
        );
        return {
          names: book.entries.map((entry) => entry.name),
          kinds: [...new Set(book.entries.map((entry) => entry.kind))],
          dragon: book.entries.find((e) => e.name === "ADULT RED DRAGON"),
          hash: await reader.hashOf(bytes),
        };
      },
      [bookPdf(), JSON.stringify(PATTERNS)],
    );

    expect(result.names).toContain("ADULT RED DRAGON");
    expect(result.names).toContain("GOBLIN");
    expect(result.kinds).toEqual(["creature"]);

    // The declared fields, read into values a review can show.
    expect(result.dragon?.values.armorClass).toEqual({
      state: "clear",
      value: "22",
    });
    expect(result.dragon?.values.hitPoints).toEqual({
      state: "clear",
      value: "546",
    });

    // FR-002: a field that was not found carries no value at all. The
    // generated union has no `value` on `unread`, so this cannot drift.
    expect(result.dragon?.values.challenge).toEqual({ state: "unread" });

    // A hash is taken locally and is the one thing allowed to reach the
    // server before submit — it is not content (FR-047, spec 047 FR-072).
    expect(result.hash).toMatch(/^[0-9a-f]{64}$/);
  });

  test("a page that yielded no text is reported, not quietly skipped", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto("/");

    const result = await page.evaluate(
      async ([source, patterns]) => {
        const reader = (await import(
          /* @vite-ignore */ "/src/services/bookImport.ts"
        )) as typeof import("../src/services/bookImport");
        const bytes = Uint8Array.from(source as string, (character) =>
          character.charCodeAt(0),
        );
        const book = await reader.readBook(
          bytes,
          JSON.parse(patterns as string),
        );
        return {
          pages: book.pages,
          silentPages: book.silentPages,
          names: book.entries.map((entry) => entry.name),
          nothingAtAll: reader.yieldedNothing(book),
        };
      },
      [partlyScannedPdf(), JSON.stringify(PATTERNS)],
    );

    // FR-005: an empty page is counted and reported. A book that is a third
    // scans must be able to say so afterwards, not only look thin.
    expect(result.pages).toBe(2);
    expect(
      result.silentPages,
      "the page with no text is counted as silent",
    ).toBe(1);

    // And what could be read still was. A silent page is not a failed read.
    expect(result.names).toContain("GOBLIN");
    expect(
      result.nothingAtAll,
      "a book that yielded something is not reported as unreadable",
    ).toBe(false);
  });

  test("nothing carrying the book's content crosses the wire before submit", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    const bodies: string[] = [];
    page.on("request", (request) => {
      const body = request.postData();
      if (body) bodies.push(body);
    });

    await page.goto("/");
    await page.evaluate(
      async ([source, patterns]) => {
        const reader = (await import(
          /* @vite-ignore */ "/src/services/bookImport.ts"
        )) as typeof import("../src/services/bookImport");
        const bytes = Uint8Array.from(source as string, (character) =>
          character.charCodeAt(0),
        );
        await reader.readBook(bytes, JSON.parse(patterns as string));
      },
      [bookPdf(), JSON.stringify(PATTERNS)],
    );

    // Give anything eager a chance to fire before we declare the wire clean.
    await page.waitForTimeout(1_000);

    const offending = bodies.filter(
      (body) =>
        body.includes("ADULT RED DRAGON") ||
        body.includes("Hit Points 546") ||
        body.includes("Gargantuan dragon"),
    );
    expect(
      offending,
      "reading a book must not put any of it on the network before the Game Master submits (FR-020)",
    ).toEqual([]);
  });
});

/**
 * Spec 049 US1, the other half: the window between the read and the world.
 *
 * The tests above prove what the readers get out of a document. These prove
 * what a Game Master can do about it — see everything that was found, open one
 * of it, refuse part of it, and submit the rest — by rendering the real
 * component and clicking it, because every rule in FR-021 to FR-026 is a rule
 * about what a person can see and do rather than about a function's return.
 *
 * The component is mounted by `e2e/harness/mountImportReview.tsx` rather than
 * reached through a page, and that is deliberate: the account library that
 * will host it is spec 050's, and a world panel would be the wrong home for a
 * thing imported once per account. Nothing can reach it by accident yet.
 */
test.describe("The confirmation window (spec 049 US1)", () => {
  async function openReview(
    page: import("@playwright/test").Page,
    source: string,
  ) {
    await page.goto("/");
    await page.evaluate(
      async ([pdf, patterns]) => {
        // Named through a variable so TypeScript leaves the specifier alone:
        // the path is a URL the dev server resolves, not a module path.
        const url = "/e2e/harness/mountImportReview.tsx";
        const harness = (await import(
          /* @vite-ignore */ url
        )) as typeof import("./harness/mountImportReview");
        harness.mountImportReview(
          pdf as string,
          "Monster Manual.pdf",
          JSON.parse(patterns as string),
        );
      },
      [source, JSON.stringify(PATTERNS)],
    );
    // The read happens here, in wasm, before any of it is shown.
    await expect(page.getByTestId("found-total")).toBeVisible({
      timeout: 120_000,
    });
  }

  test("shows everything found, grouped and counted, with nothing sent", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    const bodies: string[] = [];
    page.on("request", (request) => {
      const body = request.postData();
      if (body) bodies.push(body);
    });

    await openReview(page, bookPdf());

    // FR-021: what was found, per kind and in total.
    await expect(page.getByTestId("found-total")).toContainText("2 found");
    await expect(page.getByTestId("tally-creature")).toHaveText(
      "creature: 2 of 2",
    );

    // FR-022: an entry opens to its fields and the page it came from.
    const dragon = page
      .locator("[data-testid^='entry-']")
      .filter({ hasText: "ADULT RED DRAGON" });
    await expect(dragon).toContainText("page 1");
    await dragon.locator("summary").click();
    await expect(dragon.locator("[data-field='armorClass']")).toHaveText("22");

    // FR-023: read, unsure and unread are three visibly different things, and
    // the unread one shows no value because it has none to show.
    await expect(dragon.locator("[data-field='armorClass']")).toHaveAttribute(
      "data-read-state",
      "clear",
    );
    const challenge = dragon.locator("[data-field='challenge']");
    await expect(challenge).toHaveAttribute("data-read-state", "unread");
    await expect(challenge).toHaveText("not found");

    // FR-020 again, through the component this time: showing a book is not
    // sending one, and submit has not been pressed.
    await page.waitForTimeout(1_000);
    expect(
      bodies.filter((body) => body.includes("ADULT RED DRAGON")),
      "the review must not put the book on the network before submit (FR-020)",
    ).toEqual([]);
    expect(
      await page.evaluate(() => window.importReviewOutcome?.submitted),
      "nothing is submitted until the Game Master presses submit (FR-024)",
    ).toBeFalsy();
  });

  test("submit hands back only what was not excluded", async ({ page }) => {
    test.setTimeout(180_000);
    await openReview(page, bookPdf());

    // FR-026: this one is not wanted.
    const goblin = page
      .locator("[data-testid^='entry-']")
      .filter({ hasText: "GOBLIN" });
    await goblin.locator("[data-testid^='include-entry-']").click();
    await expect(goblin).toHaveAttribute("data-included", "false");
    await expect(page.getByTestId("tally-creature")).toHaveText(
      "creature: 1 of 2",
    );
    await expect(page.getByTestId("submit-count")).toContainText("1 of 2");

    await page.getByTestId("submit-import").click();

    const submitted = await page.evaluate(
      () => window.importReviewOutcome?.submitted,
    );
    // FR-027: what leaves the review is what the review showed — the entry the
    // Game Master kept, and not the one they refused.
    expect(submitted?.entries.map((entry) => entry.name)).toEqual([
      "ADULT RED DRAGON",
    ]);
    expect(submitted?.pages).toBe(1);
    expect(submitted?.sha256).toMatch(/^[0-9a-f]{64}$/);
  });

  test("excluding a whole kind excludes everything in it", async ({ page }) => {
    test.setTimeout(180_000);
    await openReview(page, bookPdf());

    await page.getByTestId("include-kind-creature").click();
    await expect(page.getByTestId("tally-creature")).toHaveText(
      "creature: 0 of 2",
    );
    // Nothing left to import, so there is nothing to press. A review that
    // would send an empty compendium is FR-005's failure by another route.
    await expect(page.getByTestId("submit-import")).toBeDisabled();
  });

  test("closing the window leaves nothing behind", async ({ page }) => {
    test.setTimeout(180_000);
    await openReview(page, bookPdf());

    await page.keyboard.press("Escape");

    // FR-024 and FR-025: closing is not a quiet submit, and the component owns
    // nothing outside itself that could survive being closed.
    const outcome = await page.evaluate(() => window.importReviewOutcome);
    expect(outcome?.submitted).toBeNull();
    expect(outcome?.closes).toBe(1);
  });

  test("a book that is all images is refused, not imported empty", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto("/");
    await page.evaluate(
      async ([pdf, patterns]) => {
        // Named through a variable so TypeScript leaves the specifier alone:
        // the path is a URL the dev server resolves, not a module path.
        const url = "/e2e/harness/mountImportReview.tsx";
        const harness = (await import(
          /* @vite-ignore */ url
        )) as typeof import("./harness/mountImportReview");
        harness.mountImportReview(
          pdf as string,
          "A Scanned Book.pdf",
          JSON.parse(patterns as string),
        );
      },
      [scannedPdf(), JSON.stringify(PATTERNS)],
    );

    // FR-005: this is not an import that found nothing. It was never read.
    await expect(page.getByTestId("book-unreadable")).toBeVisible({
      timeout: 120_000,
    });
    await expect(page.getByTestId("book-unreadable")).toContainText(
      "could not read this book",
    );
    await expect(page.getByTestId("submit-import")).toHaveCount(0);
  });
});
