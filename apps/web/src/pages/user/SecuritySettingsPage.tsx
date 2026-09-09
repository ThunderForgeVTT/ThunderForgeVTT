import { SEO } from "@/components/seo/SEO";
import { Container } from "@/components/ui/container/Container";
import { TwoFactorEnrolmentPanel } from "@/pages/user/components/TwoFactorEnrolmentPanel";
import { TwoFactorRemovalPanel } from "@/pages/user/components/TwoFactorRemovalPanel";
import type { SeoConfig } from "@/types/seo";

export const securitySettingsPageSeo: SeoConfig = {
  title: "Account security",
  description: "Add a second factor to your ThunderForge account.",
  canonicalPath: "/settings/security",
  noindex: true,
};

/**
 * Spec 041 US1 (FR-001): the account-settings entrance to enrolment.
 *
 * Deliberately not `/admin/security`, which is the *instance* policy switch —
 * whether everyone must enrol — and is an admin's screen. This is the account
 * holder's own, and every signed-in person has one, which is the point: FR-001
 * asks for enrolment "from their own account settings", and until now the only
 * way in was to post JSON at the API by hand.
 *
 * FR-001a says this flow is one flow, reachable from here, from first-run
 * setup and from a sign-in that requires enrolment, identical in its steps and
 * wording — which is why the whole of it lives in `TwoFactorEnrolmentPanel`
 * and this file is a page around it. The other two entrances mount the same
 * component and differ only in where they send the person afterwards.
 */
export function SecuritySettingsPage() {
  return (
    <>
      <SEO {...securitySettingsPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Account security</h1>
            <p className="text-sm text-muted-foreground">
              How this account proves it is yours.
            </p>
          </header>
          <TwoFactorEnrolmentPanel />
          {/* Spec 041 US4. Below enrolment, because the page's subject is
              having a second factor; removing one is the other end of the
              same decision and belongs on the same screen, not hidden
              somewhere a person would have to hunt for it. */}
          <TwoFactorRemovalPanel />
        </div>
      </Container>
    </>
  );
}

export default SecuritySettingsPage;
