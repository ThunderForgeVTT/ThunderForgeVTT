import { describe, expect, it } from "vitest";
import {
  LEGAL_DOCUMENTS,
  legalSections,
  sectionsOf,
} from "@/legal/legalDocuments";

/**
 * The legal prose lives in `legal/*.md` so a lawyer can review it without
 * reading TypeScript. That only holds while the application actually reads
 * those files — a glob whose pattern stops matching yields an empty registry
 * and a policy page with headings and no policy, silently.
 */
describe("legalDocuments", () => {
  it("discovers the documents in legal/, without a hand-written list", () => {
    expect(
      Object.keys(LEGAL_DOCUMENTS).length,
      "the glob found no legal documents at all — check its pattern",
    ).toBeGreaterThan(0);
    expect(LEGAL_DOCUMENTS["dmca-policy"]).toContain("notice-and-takedown");
  });

  /**
   * `README.md` explains the directory to a human reviewer. Publishing it
   * would put "Needs legal review before launch" on the public policy page.
   */
  it("does not publish the directory's own README", () => {
    expect(LEGAL_DOCUMENTS.README).toBeUndefined();
  });

  it("splits a document into one section per heading", () => {
    const sections = sectionsOf(
      "Opening words.\n\n## First\n\nOne.\n\n## Second\n\nTwo.\n",
    );
    expect(sections.map((s) => s.heading)).toEqual([null, "First", "Second"]);
    expect(sections[0].body).toBe("Opening words.");
    expect(sections[2].body).toBe("Two.");
  });

  /**
   * Every `legal/*.md` opens with a comment naming what it is and who has
   * reviewed it. That is for a reader of the file and must not reach the page.
   */
  it("strips HTML comments rather than publishing them", () => {
    const sections = sectionsOf("<!-- not reviewed -->\n\nVisible text.");
    expect(sections).toHaveLength(1);
    expect(sections[0].body).toBe("Visible text.");
    expect(sections[0].body).not.toContain("not reviewed");
  });

  it("carries the reach statement the DMCA policy is required to make", () => {
    const headings = legalSections("dmca-policy").map((s) => s.heading);
    expect(headings).toContain("What We Can Reach, and What We Cannot");
  });

  /**
   * A missing document is a build-time mistake. It shows as a page with no
   * prose, not as an exception thrown during render — the same call
   * `resolveActorSheet` makes about a system with no sheet.
   */
  it("answers with no sections for a document that does not exist", () => {
    expect(legalSections("no-such-document")).toEqual([]);
  });
});
