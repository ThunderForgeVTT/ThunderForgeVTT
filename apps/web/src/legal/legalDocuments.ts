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
