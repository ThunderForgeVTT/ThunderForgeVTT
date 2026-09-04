import { LegalDocumentPage } from "@/pages/legal/LegalDocumentPage";
import type { SeoConfig } from "@/types/seo";

export const termsOfServiceSeo: SeoConfig = {
  title: "Terms of Service",
  description: "The terms governing use of this ThunderForge instance.",
  canonicalPath: "/legal/terms",
  noindex: false,
};

/** Prose in `legal/terms-of-service.md`. */
export default function TermsOfServicePage() {
  return (
    <LegalDocumentPage
      slug="terms-of-service"
      title="Terms of Service"
      seo={termsOfServiceSeo}
    />
  );
}
