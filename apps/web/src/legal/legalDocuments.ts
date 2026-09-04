/**
 * The product's legal prose, read from `legal/*.md` at build time.
 *
 * # Why the text is not in the component
 *
 * It was, and that made it unreviewable. A DMCA policy is a document a lawyer
 * signs off, and it lived as JSX across two directories — so "what exactly does
 * our policy say" meant opening a React component and mentally stripping tags,
 * and a revision to a sentence arrived as a diff full of markup. `legal/` is
 * now the source of truth, with its own README explaining what a reviewer is
 * looking at; this module is how the application reads it.
 *
 * The consequence that matters: **editing the policy is editing a markdown
 * file**, with no code change. That is what makes a legal review actionable
 * rather than advisory.
 *
 * # Why a glob, and why that is safe
 *
 * The same mechanism `systemActorSheets.ts` and `systemPanels.ts` use, for the
 * same reason: `import.meta.glob` is resolved by Vite **at build time**, so the
 * bundle contains exactly the documents that existed when the product was
 * compiled. There is no fetch and nothing is read at runtime.
 *
 * It also means this text is **ours** — in this repository, reviewed here,
 * compiled in — not user input. That is why it can be rendered without the
 * server's sanitizing markdown pipeline, which exists for lore a player typed.
 * Nothing here should ever render a document that did not come from this glob.
 *
 * # Adding a document requires restarting the dev server
 *
 * `legal/` is at the repository root, outside `apps/web`, so it is outside the
 * directory Vite watches. A *change* to an existing document hot-reloads; a
 * *new file* is not picked up until the dev server restarts, and until then the
 * page renders its title and no prose.
 *
 * Written down because the symptom looks exactly like a broken glob and is not
 * — it cost a confused round of debugging on the day these were added. If a
 * document is missing, check a fresh process (`pnpm --filter @thunderforge/web
 * test`) before touching the pattern.
 */

const DISCOVERED = import.meta.glob<string>("../../../../legal/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
});

function slugFromPath(modulePath: string): string | null {
  const match = /\/legal\/([^/]+)\.md$/.exec(modulePath);
  return match ? match[1] : null;
}

/** Every legal document, keyed by filename without extension. */
export const LEGAL_DOCUMENTS: Record<string, string> = Object.fromEntries(
  Object.entries(DISCOVERED).flatMap(([modulePath, source]) => {
    const slug = slugFromPath(modulePath);
    // `README.md` documents the directory for a human; it is not published.
    return slug && slug !== "README" ? [[slug, source]] : [];
  }),
);

export interface LegalSection {
  /** `null` for the text before the first heading. */
  heading: string | null;
  body: string;
}

/**
 * Split a document into its sections, one per `##` heading.
 *
 * The page renders each as its own card, so the split has to happen somewhere;
 * doing it here keeps the component from parsing markdown at all.
 *
 * HTML comments are stripped first. `legal/*.md` files open with a comment
 * explaining what they are and who has reviewed them — useful to a reader of
 * the file, and not something to publish.
 */
export function sectionsOf(source: string): LegalSection[] {
  const withoutComments = source.replace(/<!--[\s\S]*?-->/g, "").trim();
  const sections: LegalSection[] = [];
  let heading: string | null = null;
  let body: string[] = [];

  const flush = () => {
    const text = body.join("\n").trim();
    if (text.length > 0 || heading !== null) {
      sections.push({ heading, body: text });
    }
    body = [];
  };

  for (const line of withoutComments.split("\n")) {
    const match = /^##\s+(.*)$/.exec(line);
    if (match) {
      flush();
      heading = match[1].trim();
    } else {
      body.push(line);
    }
  }
  flush();

  return sections.filter((s) => s.heading !== null || s.body.length > 0);
}

/**
 * A document's sections, or an empty list where no such document exists.
 *
 * The absence is an answer rather than a throw, for the same reason
 * `resolveActorSheet` returns null: a missing document is a build-time mistake
 * that should show as a page with a heading and no prose, not as a blank screen
 * from an exception thrown during render.
 */
export function legalSections(slug: string): LegalSection[] {
  const source = LEGAL_DOCUMENTS[slug];
  return source ? sectionsOf(source) : [];
}

/**
 * One keyed statement from a document, by its heading.
 *
 * `notice-attestations.md` is a document of *lookups* rather than flowing
 * prose: each heading is a stable identifier a form references, and the text
 * below it is what a submitter affirms. Prose sections are read in order;
 * these are read by name.
 *
 * # Why this throws
 *
 * Unlike `legalSections`, a missing statement is not an answer. A checkbox
 * whose label resolved to nothing would render as an empty affirmation next to
 * a live checkbox — a submitter agreeing to blank text, on a form whose whole
 * purpose is a statement made under penalty of perjury. Failing loudly at the
 * first render is the only safe behaviour, and `legalDocuments.test.ts` asserts
 * every identifier the forms use resolves so it never reaches a browser.
 */
export function legalStatement(slug: string, id: string): string {
  const section = legalSections(slug).find((s) => s.heading === id);
  if (!section || section.body.trim().length === 0) {
    throw new Error(
      `Legal statement "${id}" is missing from legal/${slug}.md. ` +
        `A form references it; add the heading back or update the reference.`,
    );
  }
  // Authored wrapped at 80 columns for review; rejoined for display.
  return section.body.replace(/\s*\n\s*/g, " ").trim();
}
