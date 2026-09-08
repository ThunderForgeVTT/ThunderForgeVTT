import { LegalEnquiryForm } from "@/components/legal/LegalEnquiryForm";
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
      afterProse={
        <LegalEnquiryForm
          kind="TERMS"
          heading="Dispute or question these terms"
          description="This reaches the people who run this instance. There is no public address to write to — an address on a page is scraped within days, and this gets to the same person without that."
        />
      }
    />
  );
}
