import { test, expect } from "@playwright/test";

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
