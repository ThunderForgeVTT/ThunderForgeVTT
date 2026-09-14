import { test, expect } from "./fixtures/test";

/**
 * Spec 047/048: a PDF is read on the machine that holds it.
 *
 * The crate's own tests cover what it gets out of a document. What this
 * proves is the part only a browser can: that the same Rust, compiled to
 * WebAssembly, loads and runs there — with no server involved in the reading
 * at all.
 *
 * Why that is safe is a matter of *who*: a source book is read into a world by
 * its Game Master, from their own world panel, and a Game Master can already
 * put anything into their own world through the authoring tools. A player's
 * character sheet is guarded differently, by spec 048's adoption gate, and
 * those guards never trusted the parse in the first place.
 *
 * The document is built here rather than shipped as a fixture. A real book is
 * copyrighted and a binary fixture tells nobody what it contains; a PDF
 * written in the test says exactly what the parser is being asked to find.
 */

/** A one-page PDF with a statblock's worth of text on it, built by hand. */
function statblockPdf(): string {
  const lines = [
    "BT /F1 18 Tf 72 720 Td (ADULT RED DRAGON) Tj ET",
    "BT /F1 9 Tf 72 700 Td (Gargantuan dragon, chaotic evil) Tj ET",
    "BT /F1 9 Tf 72 686 Td (Armor Class 22) Tj ET",
    "BT /F1 9 Tf 72 672 Td (Hit Points 546) Tj ET",
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

/** The same document with its cross-reference table destroyed. */
function brokenPdf(): string {
  // Everything from `xref` onwards replaced with nonsense: the objects are
  // intact and the index is not, which is the state 37% of a real library is
  // in and the reason the crate rebuilds one.
  return `${statblockPdf().split("xref\n")[0]}xref\nGARBAGE\ntrailer\n<< >>\nstartxref\n999999\n%%EOF\n`;
}

test.describe("Reading a PDF in the browser (spec 047/048)", () => {
  test("the same parser the server runs reads a document with no server involved", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto("/");

    const result = await page.evaluate(
      async ([good, broken]) => {
        const reader = (await import(
          /* @vite-ignore */ "/src/services/pdfReader.ts"
        )) as typeof import("../src/services/pdfReader");

        const bytes = (text: string) =>
          Uint8Array.from(text, (character) => character.charCodeAt(0));

        const ok = await reader.readPages(bytes(good), 1, 1);
        const repaired = await reader.readPages(bytes(broken), 1, 1);
        return {
          pages: ok.pages,
          text: ok.lines.map((line) => line.text),
          headings: ok.lines.filter((line) => line.heading).map((l) => l.text),
          wasRepaired: repaired.repaired,
          repairedText: repaired.lines.map((line) => line.text),
        };
      },
      [statblockPdf(), brokenPdf()],
    );

    expect(result.pages).toBe(1);
    expect(result.text).toContain("Armor Class 22");
    expect(result.text).toContain("Hit Points 546");

    // The styling a reader above this depends on: the title is set larger than
    // the body, and the parser says so. Without this it is a wall of text.
    expect(
      result.headings,
      "the dragon's name is read as a heading, not as another line of prose",
    ).toContain("ADULT RED DRAGON");

    // And the repair works here too. It is not a server-side nicety: a Game
    // Master's own copy of a book is as likely to have a broken index as
    // anybody's, and 37% of a real library does.
    expect(result.wasRepaired, "the broken index was rebuilt").toBe(true);
    expect(result.repairedText).toContain("Armor Class 22");
  });

  test("a document that is not a PDF is refused in the same words the server would use", async ({
    page,
  }) => {
    test.setTimeout(120_000);
    await page.goto("/");

    const message = await page.evaluate(async () => {
      const reader = (await import(
        /* @vite-ignore */ "/src/services/pdfReader.ts"
      )) as typeof import("../src/services/pdfReader");
      try {
        await reader.readPages(
          Uint8Array.from("this is not a pdf at all", (c) => c.charCodeAt(0)),
          1,
          1,
        );
        return null;
      } catch (error) {
        return String(error);
      }
    });

    expect(message, "a refusal, not a silent empty document").not.toBeNull();
    expect(message).toContain("could not read the document");
  });
});
