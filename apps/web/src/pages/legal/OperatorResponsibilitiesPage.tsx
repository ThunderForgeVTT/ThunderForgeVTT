import { LegalDocumentPage } from "@/pages/legal/LegalDocumentPage";
import type { SeoConfig } from "@/types/seo";

export const operatorResponsibilitiesSeo: SeoConfig = {
  title: "Operator responsibilities",
  description:
    "What running this ThunderForge instance makes its operator responsible for.",
  canonicalPath: "/legal/operator",
  noindex: false,
};

/**
 * Spec 039 FR-045: the operator statement, readable inside the running
 * instance. An operator should never need the repository to re-read what they
 * took on at setup — and a person using the instance can read what its
 * operator was told. The same words setup showed; the acknowledgement records
 * which version.
 */
export default function OperatorResponsibilitiesPage() {
  return (
    <LegalDocumentPage
      slug="operator-responsibilities"
      title="Operator responsibilities"
      seo={operatorResponsibilitiesSeo}
    />
  );
}
