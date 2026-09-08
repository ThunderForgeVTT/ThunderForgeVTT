import { describe, expect, it } from "vitest";
import {
  LEGAL_DOCUMENTS,
  legalSections,
  legalStatement,
  OPERATOR_TOKENS,
  OPERATOR_UNSET_MARKERS,
  resolveOperatorValue,
  sectionsOf,
  substituteOperatorValues,
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

/**
 * Spec 040 T055/T064/T065: the four *value* markers now resolve from settings.
 *
 * The invariant these guard is the one the placeholder test above states, moved
 * one layer down. Before this feature an unfilled operator name was a literal
 * `[OPERATOR — ...]` in the markdown and could only ever render. Now it is a
 * token, and a token has a way to render as nothing — which would produce the
 * page that reads as complete while naming nobody, on the two documents where
 * that matters most. So the unset rendering is asserted directly.
 */
describe("operator value tokens", () => {
  const NAME_MARKER = OPERATOR_UNSET_MARKERS["operator.name"];

  /** The rendered text of a substitution, prose and values alike. */
  const rendered = (text: string, values = {}) =>
    substituteOperatorValues(text, values)
      .segments.map((segment) => segment.text)
      .join("");

  it("renders the visible marker for a token nobody has set", () => {
    const out = substituteOperatorValues("Operator: {{operator.name}}");
    expect(out.segments.map((s) => s.kind)).toEqual(["prose"]);
    expect(rendered("Operator: {{operator.name}}")).toBe(
      `Operator: ${NAME_MARKER}`,
    );
    expect(NAME_MARKER).toContain("[OPERATOR");
  });

  /**
   * Null and blank are the shapes an unset setting actually arrives in — the
   * GraphQL field is nullable, and a text input that was focused and left
   * empty submits `""`. Neither may be published as the operator's name.
   */
  it.each([
    ["absent", undefined],
    ["null", null],
    ["empty", ""],
    ["whitespace", "   "],
  ])("treats a %s value as unset rather than as a name", (_label, value) => {
    expect(rendered("{{operator.name}}", { "operator.name": value })).toBe(
      NAME_MARKER,
    );
  });

  it("substitutes a value that is set", () => {
    expect(
      rendered("Operator: {{operator.name}}", {
        "operator.name": "Riverside Table",
      }),
    ).toBe("Operator: Riverside Table");
  });

  /**
   * The trust boundary, asserted as data rather than as rendering: a value is
   * its own segment, so `LegalProse` can print it without passing it through
   * the `**bold**` / `[text](url)` matcher. An operator name that looks like a
   * link must reach the page as text — publishing an operator-chosen link
   * inside the terms of service is the failure this whole split exists to
   * prevent.
   */
  it("keeps a substituted value in its own segment, apart from the prose", () => {
    const out = substituteOperatorValues(
      "Operator: {{operator.name}}, contact **{{operator.contact_email}}**.",
      {
        "operator.name": "[click here](https://elsewhere.example)",
        "operator.contact_email": "gm@riverside.example",
      },
    );
    expect(out.segments).toEqual([
      { kind: "prose", text: "Operator: " },
      { kind: "value", text: "[click here](https://elsewhere.example)" },
      { kind: "prose", text: ", contact **" },
      { kind: "value", text: "gm@riverside.example" },
      { kind: "prose", text: "**." },
    ]);
  });

  /**
   * Prose either side of an unset token is merged back into one segment, so a
   * marker sitting inside a `**bold**` run does not split the emphasis in half
   * and leave the asterisks on the page.
   */
  it("merges prose around an unset token into one segment", () => {
    const out = substituteOperatorValues("**Operator: {{operator.name}}**");
    expect(out.segments).toHaveLength(1);
    expect(out.segments[0].kind).toBe("prose");
  });

  it("leaves a token outside the closed set exactly as written", () => {
    expect(rendered("Say {{operator.favourite_colour}} here")).toBe(
      "Say {{operator.favourite_colour}} here",
    );
  });

  /**
   * A `{{...}}` nobody implemented renders verbatim rather than throwing, which
   * is the right behaviour for a page and the wrong one for a review: it would
   * ship as visible braces in a published policy. So the documents are checked
   * against the closed set here, where it fails in CI instead.
   */
  it("uses no token in legal/*.md that the closed set does not declare", () => {
    const declared = new Set(Object.keys(OPERATOR_TOKENS));
    for (const slug of Object.keys(LEGAL_DOCUMENTS)) {
      // The published sections, not the raw file: each document opens with an
      // HTML comment explaining the token mechanism to a reviewer, and that
      // comment names token shapes it is describing rather than using.
      const published = legalSections(slug)
        .map((section) => section.body)
        .join("\n");
      for (const token of published.match(/\{\{[^}]*\}\}/g) ?? []) {
        expect(declared.has(token), `legal/${slug}.md uses ${token}`).toBe(
          true,
        );
      }
    }
  });

  /**
   * The four value markers, named per document. Counted would pass while the
   * wrong ones were present, and these two documents are the pages FR-005 is
   * about: an instance that finished setup must publish an operator and a
   * contact on both, with no file edited.
   */
  it.each(["terms-of-service", "privacy-policy"])(
    "resolves the operator and the contact in %s",
    (slug) => {
      const body = legalSections(slug)
        .map((s) => s.body)
        .join("\n");
      expect(body).toContain("{{operator.name}}");
      expect(body).toContain("{{operator.contact_email}}");

      const filled = rendered(body, {
        "operator.name": "Riverside Table",
        "operator.contact_email": "gm@riverside.example",
      });
      expect(filled).toContain("Riverside Table");
      expect(filled).toContain("gm@riverside.example");
      // The ten prose markers are not values and are not filled by anything.
      // They stay visible until a human writes them (research.md § D2).
      expect(rendered(body)).toContain("[OPERATOR");
    },
  );

  /**
   * The DMCA agent designation reads the same settings as a definition list
   * rather than as prose. Unset, it keeps the "configure before launch" text
   * the page has always shown — a designation that names nobody must say so.
   */
  it("keeps the pre-launch placeholder for an unset notice contact", () => {
    for (const key of [
      "notice.contact_name",
      "notice.contact_email",
      "notice.contact_postal_address",
    ] as const) {
      const unset = resolveOperatorValue(key);
      expect(unset.isSet).toBe(false);
      expect(unset.text).toContain("before launch");

      const set = resolveOperatorValue(key, { [key]: "Copyright Agent" });
      expect(set).toEqual({ text: "Copyright Agent", isSet: true });
    }
  });
});
