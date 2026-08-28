import { TakedownNoticeForm } from "@/components/legal/TakedownNoticeForm";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import type { SeoConfig } from "@/types/seo";

export const dmcaComplianceSeo: SeoConfig = {
  title: "DMCA / Copyright Policy",
  description:
    "ThunderForge's designated DMCA agent and notice-and-takedown process.",
  canonicalPath: "/legal/dmca",
  noindex: false,
};

/**
 * Spec 015 (FR-001, FR-002, FR-014, SC-004): the platform's public DMCA
 * agent designation and takedown intake channel. Reachable without
 * authentication — this route carries no auth guard by design.
 */
export default function DmcaCompliancePage() {
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
            <p className="max-w-2xl text-muted-foreground">
              ThunderForge ships official game-system packs (5E System Core,
              Pathfinder 2e, and others) distributed under their respective open
              licenses. Anything a GM or player enters into their own
              world&apos;s compendium — custom NPCs, items, or lore — is that
              user&apos;s sole responsibility and is subject to the
              notice-and-takedown process below.
            </p>
          </section>

          <Card
            surface="stone"
            className="grid gap-3 p-6"
            data-testid="dmca-agent-designation"
          >
            <h2 className="text-lg font-semibold">Designated DMCA Agent</h2>
            <dl className="grid gap-1 text-sm">
              <div>
                <dt className="inline font-medium">Name/Title: </dt>
                <dd className="inline text-muted-foreground">
                  Copyright Agent, ThunderForge
                </dd>
              </div>
              <div>
                <dt className="inline font-medium">Mailing Address: </dt>
                <dd className="inline text-muted-foreground">
                  [Configure via instance legal/compliance settings before
                  launch]
                </dd>
              </div>
              <div>
                <dt className="inline font-medium">Electronic Contact: </dt>
                <dd className="inline text-muted-foreground">
                  dmca@thunderforge.example
                </dd>
              </div>
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

          <Card surface="parchment" className="grid gap-2 p-6">
            <h2 className="text-lg font-semibold">
              Counter-Notices &amp; Repeat Infringers
            </h2>
            <p className="text-sm text-muted-foreground">
              A GM whose content was disabled may submit a counter-notice from
              that content&apos;s detail page. Absent further action from the
              original claimant, disabled content is restored after the
              statutory waiting period. Accounts with a pattern of repeated,
              valid infringement notices are subject to termination under our
              repeat-infringer policy.
            </p>
          </Card>
        </main>
      </Container>
    </>
  );
}
