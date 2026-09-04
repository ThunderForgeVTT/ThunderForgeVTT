import { LegalProse } from "@/components/legal/LegalProse";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { legalSections } from "@/legal/legalDocuments";
import type { SeoConfig } from "@/types/seo";

/**
 * Any legal document that is only prose — currently the terms of service and
 * the privacy policy.
 *
 * The DMCA page has its own component because it is not only prose: it carries
 * the designated-agent definition list and the takedown form, both of which are
 * structure this cannot express. Everything else in `legal/` is a title and a
 * run of sections, and a second bespoke page per document would be three copies
 * of the same twenty lines.
 *
 * Sections are keyed by heading rather than index for the same reason the DMCA
 * page keys them that way: reordering the markdown should reorder the page, not
 * repaint one card with another's text.
 */
export interface LegalDocumentPageProps {
  /** The `legal/<slug>.md` to render. */
  slug: string;
  title: string;
  seo: SeoConfig;
}

export function LegalDocumentPage({
  slug,
  title,
  seo,
}: LegalDocumentPageProps) {
  const sections = legalSections(slug);
  const intro = sections.find((s) => s.heading === null);
  const body = sections.filter((s) => s.heading !== null);

  return (
    <>
      <SEO {...seo} />
      <Container>
        <main className="grid gap-8 py-8" data-testid={`legal-page-${slug}`}>
          <section className="grid gap-3">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Legal
            </p>
            <h1 className="text-3xl font-semibold">{title}</h1>
            <div className="grid max-w-2xl gap-3">
              {intro ? (
                <LegalProse
                  body={intro.body}
                  className="text-muted-foreground"
                />
              ) : null}
            </div>
          </section>

          {body.map((section) => (
            <Card
              key={section.heading}
              surface="parchment"
              className="grid gap-3 p-6"
              data-testid={`legal-section-${slugify(section.heading ?? "")}`}
            >
              <h2 className="text-lg font-semibold">{section.heading}</h2>
              <LegalProse body={section.body} />
            </Card>
          ))}
        </main>
      </Container>
    </>
  );
}

function slugify(heading: string): string {
  return heading
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}
