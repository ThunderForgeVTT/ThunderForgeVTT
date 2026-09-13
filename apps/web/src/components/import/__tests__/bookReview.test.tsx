import { describe, expect, it } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { BookReview } from "../BookReview";
import {
  approved,
  correct,
  includeEntry,
  includeKind,
  nothingExcluded,
  tally,
} from "../selection";
import type { ContentEntry } from "@/engine/sdk/ContentEntry";
import type { ReadBook } from "@/services/bookImport";

/**
 * Spec 049 T034 — what the review shows, and what it hands back.
 *
 * Two halves, proved in two places on purpose. *What a Game Master can do in
 * the window* is a claim about a real browser driving a real read, and lives in
 * `e2e/book-import-review.spec.ts`; a simulated version of it would be worth
 * very little. *What appears, and what one decision turns into* is a claim
 * about this component and these functions, and is cheapest and sharpest here.
 *
 * Rendered to markup rather than into a DOM because `apps/web` has neither
 * jsdom nor testing-library — see `sheet-layout/__tests__/sheetLayout.test.tsx`
 * for why that is not being changed for this spec's sake. The first render is
 * exactly what a Game Master is shown when the read finishes, which is the
 * moment every rule below is about.
 */

function entry(over: Partial<ContentEntry> = {}): ContentEntry {
  return {
    kind: "creature",
    name: "GOBLIN",
    nameState: "clear",
    page: 7,
    values: {
      armorClass: { state: "clear", value: "15" },
      hitPoints: { state: "uncertain", value: "7" },
      challenge: { state: "unread" },
    },
    text: null,
    suspect: false,
    ...over,
  };
}

function book(over: Partial<ReadBook> = {}): ReadBook {
  return {
    entries: [entry()],
    pages: 10,
    silentPages: 0,
    repaired: false,
    ...over,
  };
}

const render = (read: ReadBook) =>
  renderToStaticMarkup(
    <BookReview
      title="Dungeon Master's Guide.pdf"
      book={read}
      onSubmit={() => {
        throw new Error("submit is the Game Master's, not the renderer's");
      }}
      onClose={() => {}}
    />,
  );

describe("what the review shows (FR-021 to FR-023)", () => {
  it("counts every kind it found, and the whole", () => {
    const html = render(
      book({
        entries: [
          entry(),
          entry({ name: "ADULT RED DRAGON" }),
          entry({ kind: "spell", name: "Fireball" }),
        ],
      }),
    );

    expect(html).toContain("creature: 2 of 2");
    expect(html).toContain("spell: 1 of 1");
    expect(html).toContain("<strong>3</strong> found");
  });

  it("names the page an entry came from, so it can be checked", () => {
    expect(render(book({ entries: [entry({ page: 253 })] }))).toContain(
      "page 253",
    );
  });

  it("invents no value for a field that was not found", () => {
    const html = render(book());

    // The field is present and marked, and carries no reading of any sort.
    // `ReadValue` gives `unread` no value at all, so there is nothing here
    // that a careless edit could promote into a number.
    expect(html).toContain('data-field="challenge" data-read-state="unread"');
    expect(html).toContain("not found");
  });

  it("marks read, unsure and unread differently from one another", () => {
    const html = render(book());

    expect(html).toContain('data-field="armorClass" data-read-state="clear"');
    expect(html).toContain(
      'data-field="hitPoints" data-read-state="uncertain"',
    );
    expect(html).toContain('data-field="challenge" data-read-state="unread"');
    // The unsure one is the only one a Game Master can type over.
    expect(html).toContain('value="7"');
    expect(html).toContain("unsure");
  });

  it("says how many pages were silent rather than only looking thin", () => {
    const html = render(book({ pages: 300, silentPages: 97 }));
    expect(html).toContain("97 of 300 pages had no text");
  });

  it("refuses a book that is images from cover to cover, and offers no submit", () => {
    // FR-005: not an import that found nothing — a book that was never read.
    const html = render(book({ entries: [], pages: 12, silentPages: 12 }));

    expect(html).toContain("We could not read this book");
    expect(html).not.toContain('data-testid="submit-import"');
  });
});

describe("what submit hands back (FR-026, FR-027, spec 048 FR-022)", () => {
  const entries = [
    entry({ name: "GOBLIN" }),
    entry({ name: "ADULT RED DRAGON" }),
    entry({ kind: "spell", name: "Fireball" }),
  ];

  it("leaves out an excluded entry, and nothing else", () => {
    const chosen = approved(entries, includeEntry(nothingExcluded(), 1, false));
    expect(chosen.map((found) => found.name)).toEqual(["GOBLIN", "Fireball"]);
  });

  it("leaves out a whole excluded kind", () => {
    const chosen = approved(
      entries,
      includeKind(nothingExcluded(), "creature", false),
    );
    expect(chosen.map((found) => found.name)).toEqual(["Fireball"]);
  });

  it("counts what survives per kind, beside what was found", () => {
    expect(tally(entries, includeEntry(nothingExcluded(), 0, false))).toEqual([
      { kind: "creature", found: 2, included: 1 },
      { kind: "spell", found: 1, included: 1 },
    ]);
  });

  it("carries a correction into what is submitted, as a clear reading", () => {
    const chosen = approved(
      entries,
      correct(nothingExcluded(), 0, "hitPoints", "77"),
    );
    expect(chosen[0]?.values.hitPoints).toEqual({
      state: "clear",
      value: "77",
    });
    // And only that field of only that entry.
    expect(chosen[1]?.values.hitPoints).toEqual({
      state: "uncertain",
      value: "7",
    });
  });

  it("will not let a correction put a value on an unread field", () => {
    // FR-002 again, from the other direction: a field the reader never found
    // has no reading to revise, and the review is not a place to invent one.
    const chosen = approved(
      entries,
      correct(nothingExcluded(), 0, "challenge", "5"),
    );
    expect(chosen[0]?.values.challenge).toEqual({ state: "unread" });
  });

  it("gives the reader's own reading back when a correction is blanked", () => {
    const withdrawn = correct(
      correct(nothingExcluded(), 0, "hitPoints", "77"),
      0,
      "hitPoints",
      "  ",
    );
    expect(approved(entries, withdrawn)[0]?.values.hitPoints).toEqual({
      state: "uncertain",
      value: "7",
    });
  });

  it("does not touch the read itself", () => {
    // FR-027 is a claim about one direction only. Corrections live beside the
    // entries, so the entry a second reviewer sees is the one the book gave.
    approved(entries, correct(nothingExcluded(), 0, "hitPoints", "77"));
    expect(entries[0]?.values.hitPoints).toEqual({
      state: "uncertain",
      value: "7",
    });
  });
});
