import { LegalProse } from "@/components/legal/LegalProse";
import { TakedownNoticeForm } from "@/components/legal/TakedownNoticeForm";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import {
  legalSections,
  resolveOperatorValue,
  type OperatorValueKey,
  type OperatorValues,
} from "@/legal/legalDocuments";
import { usePublishedOperatorValues } from "@/legal/publishedOperatorValues";
import type { SeoConfig } from "@/types/seo";

export const dmcaComplianceSeo: SeoConfig = {
  title: "DMCA / Copyright Policy",
  description:
    "ThunderForge's designated DMCA agent and notice-and-takedown process.",
  canonicalPath: "/legal/dmca",
  noindex: false,
};

/**
 * Spec 015 (FR-001, FR-002, FR-014, FR-015..FR-018, SC-004): the platform's
 * public DMCA agent designation, takedown intake channel, and the statement of
 * where the platform's reach ends. Reachable without authentication — this
 * route carries no auth guard by design.
 *
 * # Where the words are
 *
 * `legal/dmca-policy.md`. This component owns the page's *structure* — which
 * sections become cards, where the agent designation and the notice form sit —
 * and none of its prose.
 *
 * That split exists because the two halves have different reviewers. A lawyer
 * reviews the policy and should not have to read JSX to do it; an engineer
 * reviews the page and should not be editing legal text by accident. Before
 * this, both lived here, and "what exactly does our DMCA page say" meant
 * opening a React component and mentally stripping tags.
 *
 * Adding a `##` heading to the markdown adds a card. The surfaces are keyed by
 * heading rather than by position, so reordering the document reorders the page
 * and does not silently repaint one section with another's text.
 */

export default function DmcaCompliancePage() {
  const sections = legalSections("dmca-policy");
  const operatorValues = usePublishedOperatorValues();
  const intro = sections.find((s) => s.heading === null);
  const body = sections.filter((s) => s.heading !== null);

  return (
    <>
      <SEO {...dmcaComplianceSeo} />
      <Container>
        <main className="grid gap-8 py-8" data-testid="dmca-compliance-page">
          <section className="grid gap-3">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Legal
            </p>
            <h1 className="text-3xl font-semibold">DMCA / Copyright Policy</h1>
            <div className="grid max-w-2xl gap-3">
              {intro ? (
                <LegalProse
                  body={intro.body}
                  className="text-muted-foreground"
                />
              ) : null}
            </div>
          </section>

          {/* Structured, not prose: these are instance configuration values
              rendered as a definition list. They stay here rather than in the
              markdown because a reviewer editing policy text should not be
              editing an address field, and because the mailing address is a
              value the operator supplies rather than something anyone writes.

              That comment was written waiting for spec 040, and this is it:
              the three literals that used to sit here — "Copyright Agent,
              ThunderForge", a placeholder address, and an @example.com
              mailbox — now come from the notice-contact settings, read over
              the unauthenticated query because whoever needs to file a notice
              has no account here (spec 039 FR-056).

              A value nobody has set still renders the pre-launch placeholder,
              unchanged and marked so it reads as unconfigured. An instance
              that has not been through setup therefore publishes the page it
              published before — which is the honest one: it also cannot mint
              a share link, because spec 040's publishing gate refuses while
              the notice contact is unset. */}
          <Card
            surface="stone"
            className="grid gap-3 p-6"
            data-testid="dmca-agent-designation"
          >
            <h2 className="text-lg font-semibold">Designated DMCA Agent</h2>
            <dl className="grid gap-1 text-sm">
              <AgentValue
                label="Name/Title"
                settingKey="notice.contact_name"
                values={operatorValues}
              />
              <AgentValue
                label="Mailing Address"
                settingKey="notice.contact_postal_address"
                values={operatorValues}
              />
              <AgentValue
                label="Electronic Contact"
                settingKey="notice.contact_email"
                values={operatorValues}
              />
            </dl>
            <p className="text-xs text-muted-foreground">
              This designation is kept current with the U.S. Copyright
              Office&apos;s Designated Agent Directory (17 U.S.C. § 512(c)(2)).
            </p>
          </Card>

          <Card surface="leather" className="grid gap-3 p-6">
            <h2 className="text-lg font-semibold">Submit a Takedown Notice</h2>
            <p className="text-sm text-muted-foreground">
              If you believe specific user-entered content on ThunderForge
              infringes your copyright, submit a notice below. We will disable
              access to the identified content and notify its owner, who may
              submit a counter-notice if they believe the removal was a mistake.
            </p>
            <TakedownNoticeForm />
          </Card>

          {body.map((section) => (
            <Card
              key={section.heading}
              surface="parchment"
              className="grid gap-3 p-6"
              data-testid={`dmca-section-${slugify(section.heading ?? "")}`}
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

/**
 * One row of the agent designation, resolved or visibly unconfigured.
 *
 * The value is printed as text and never parsed — the same rule
 * `LegalProse.tsx` follows, for the same reason: this is an operator's typed
 * setting rendered on a page that carries a statutory designation.
 *
 * An unset value keeps the "configure before launch" placeholder rather than
 * rendering blank. A designation naming nobody, that reads as complete, is
 * worse than one that says out loud it has not been filled in.
 */
function AgentValue({
  label,
  settingKey,
  values,
}: {
  label: string;
  settingKey: OperatorValueKey;
  values: OperatorValues;
}) {
  const { text, isSet } = resolveOperatorValue(settingKey, values);
  return (
    <div>
      <dt className="inline font-medium">{label}: </dt>
      <dd
        className="inline text-muted-foreground"
        data-testid={`dmca-agent-${settingKey.replace(/[^a-z]+/g, "-")}`}
        data-configured={isSet ? "true" : "false"}
      >
        {text}
      </dd>
    </div>
  );
}

/** A stable test id per section, so an e2e names a heading rather than an index. */
function slugify(heading: string): string {
  return heading
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}
