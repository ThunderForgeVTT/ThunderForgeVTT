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
    />
  );
}
