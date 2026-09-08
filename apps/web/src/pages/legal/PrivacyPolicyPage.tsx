import { LegalEnquiryForm } from "@/components/legal/LegalEnquiryForm";
import { LegalDocumentPage } from "@/pages/legal/LegalDocumentPage";
import type { SeoConfig } from "@/types/seo";

export const privacyPolicySeo: SeoConfig = {
  title: "Privacy Policy",
  description:
    "What this ThunderForge instance stores, what it does not, and how to get or delete your data.",
  canonicalPath: "/legal/privacy",
  noindex: false,
};

/** Prose in `legal/privacy-policy.md`. */
export default function PrivacyPolicyPage() {
  return (
    <LegalDocumentPage
      slug="privacy-policy"
      title="Privacy Policy"
      seo={privacyPolicySeo}
      afterProse={
        <LegalEnquiryForm
          kind="PRIVACY"
          heading="Ask about your data"
          description="Ask what this instance holds about you, ask for a copy, or ask for it to be deleted. This reaches the people who run it directly."
        />
      }
    />
  );
}
