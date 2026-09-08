import { Link } from "react-router-dom";
import { Container } from "@/components/ui/container/Container";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import {
  SITE_LINK_GROUPS,
  type SiteLinkGroup,
} from "@/components/navigation/siteLinks";

/**
 * The site footer.
 *
 * # Why this is not decoration
 *
 * Spec 039 FR-056 requires the contact for copyright notices to be
 * **discoverable by anyone who needs to file one, without an account**. The
 * pages that carry it — `/legal/dmca` and its two siblings — have existed and
 * rendered the operator's details correctly for some time, and *nothing in the
 * product linked to them*. They were reachable only by typing the URL, which
 * is the same defect spec 031 found in the admin area and fixed with a nav.
 *
 * A person with a copyright complaint is the least likely visitor to guess a
 * URL, and the most consequential one to lose. So the legal column is the
 * reason this component exists; the rest is the footer it needed to live in.
 *
 * # Why it does not name the operator here
 *
 * It could: `publishedOperatorValues` is anonymous and cheap. But it would be
 * a fetch on every page in the product to render a line most visitors never
 * read, and the page it links to already renders those values as its subject
 * rather than its footnote. A link that works is worth more than a name that
 * costs.
 *
 * # Why the links are grouped and labelled rather than a row of text
 *
 * The previous footer was one marketing sentence and a single "System status"
 * link. Adding three legal links to that row would have produced five
 * undifferentiated words, where "Privacy" and "Status" read as equals. They
 * are not: one is a legal document and the other is a diagnostic.
 */

function LinkColumn({ heading, links, testId }: SiteLinkGroup) {
  return (
    <nav className="grid gap-2" aria-label={heading} data-testid={testId}>
      <h2 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        {heading}
      </h2>
      <ul className="grid gap-1.5">
        {links.map((link) => (
          <li key={link.to}>
            <Link
              to={link.to}
              className="text-sm text-muted-foreground transition-colors hover:text-foreground hover:underline"
            >
              {link.label}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}

export function AppFooter() {
  return (
    <footer className="pb-8" data-testid="app-footer">
      <Container>
        <div className="grid gap-8 border-t border-border pt-8 sm:grid-cols-2 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)_minmax(0,1fr)]">
          <div className="grid gap-2 self-start">
            <span className="inline-flex items-center gap-2">
              <FantasyIcon name="crown" size={18} />
              <strong className="text-sm tracking-wide">ThunderForge</strong>
            </span>
            {/* Deliberately free of the product's own nouns. The first
                version read "your worlds, your players, your server" and broke
                `session-notes.spec.ts`, which asserts `getByText("Players")`
                resolves to one element — Playwright matches a bare string as a
                case-insensitive substring, so a footer on every page had
                quietly become a second match for a word the UI uses as a
                heading. Blurb copy is not worth making an assertion brittle
                for. */}
            <p className="max-w-prose text-sm text-muted-foreground">
              Self-hosted, and yours. The campaign lives on a machine you
              control.
            </p>
          </div>

          {SITE_LINK_GROUPS.map((group) => (
            <LinkColumn key={group.heading} {...group} />
          ))}
        </div>
      </Container>
    </footer>
  );
}
