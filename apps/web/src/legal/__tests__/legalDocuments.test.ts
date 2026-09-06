import { describe, expect, it } from "vitest";
import {
  LEGAL_DOCUMENTS,
  legalSections,
  legalStatement,
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
   * Every document something renders must be discoverable, or that surface
   * shows a title over nothing. Named explicitly rather than counted, because
   * "there are four documents" passes while the wrong four are present.
   *
   * `collection-sharing-terms` has no route of its own — spec 026's FR-026
   * renders it inline at the share step, which is the only place it is read.
   * It belongs here for exactly the reason the others do: if the glob stops
   * matching, the share step renders an empty box above the button and a Game
   * Master shares without being told what sharing does.
   */
  it.each([
    "dmca-policy",
    "terms-of-service",
    "privacy-policy",
    "collection-sharing-terms",
  ])("publishes %s, which a rendered surface depends on", (slug) => {
    expect(legalSections(slug).length).toBeGreaterThan(0);
  });

  /**
   * The base drafts carry `[OPERATOR — ...]` markers where a self-hosting
   * operator must supply their own name, contact and jurisdiction. They are
   * meant to be visible: a published policy showing an unfilled marker is
   * embarrassing, and a *silently dropped* one is worse, because the page then
   * reads as complete while saying nothing about who holds your data.
   *
   * This asserts they survive rendering rather than that they are absent. The
   * day they should disappear is the day an operator fills them in, and that is
   * an edit to the markdown, not to this test.
   */
  it("keeps operator placeholders visible rather than swallowing them", () => {
    const privacy = legalSections("privacy-policy")
      .map((s) => s.body)
      .join("\n");
    expect(privacy).toContain("[OPERATOR");
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

/**
 * The statements a submitter affirms under penalty of perjury.
 *
 * These are looked up by heading rather than read in order, which introduces a
 * failure the prose documents do not have: rename a heading in
 * `notice-attestations.md` and the form asking for that affirmation loses its
 * label. A live checkbox beside blank text is a submitter agreeing to nothing,
 * on the one form where the words are the entire point.
 *
 * So every identifier the forms use is asserted here. `legalStatement` throws
 * rather than returning empty, and this is what keeps that throw from being
 * discovered in a browser.
 */
describe("notice attestations", () => {
  const REFERENCED_BY_FORMS = [
    "takedown-good-faith",
    "takedown-accuracy",
    "counter-notice-good-faith",
    "counter-notice-jurisdiction",
  ] as const;

  it.each(REFERENCED_BY_FORMS)("resolves %s to real text", (id) => {
    const text = legalStatement("notice-attestations", id);
    expect(text.length).toBeGreaterThan(20);
    expect(text).not.toContain("\n");
  });

  /**
   * Statutory language, so the substance is asserted rather than only the
   * presence of a string. Someone softening "under penalty of perjury" out of
   * either affirmation has changed what the form legally is.
   */
  it("keeps the perjury affirmation in both notices", () => {
    expect(legalStatement("notice-attestations", "takedown-accuracy")).toMatch(
      /under penalty of perjury/i,
    );
    expect(
      legalStatement("notice-attestations", "counter-notice-good-faith"),
    ).toMatch(/under penalty of perjury/i);
  });

  it("refuses a statement it cannot find, rather than rendering nothing", () => {
    expect(() =>
      legalStatement("notice-attestations", "no-such-statement"),
    ).toThrow(/missing from legal/);
  });
});
