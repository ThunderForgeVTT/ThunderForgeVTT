import { postGraphQL } from "@/api/graphqlClient";

/**
 * Terms disputes and privacy requests.
 *
 * # Submitting goes to the public endpoint, on purpose
 *
 * `/api/graphql` is wrapped in `require_authenticated_user`. A person
 * disputing the terms of service is often disputing the terms they were asked
 * to accept, so a complaints channel that requires an account first is not
 * one — the same reasoning `publishedOperatorValues` records, and the same
 * defect: sending this to the default endpoint would answer 401 and the form
 * would fail for exactly the people it exists for.
 *
 * # Reading is a different endpoint and a different audience
 *
 * The queue carries names, addresses and whatever somebody chose to write
 * about themselves. Those calls are administrators-only and go through the
 * authenticated transport like any other admin read.
 */

const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";

export type LegalEnquiryKind = "TERMS" | "PRIVACY";
export type LegalEnquiryStatus = "OPEN" | "ACKNOWLEDGED" | "CLOSED";

export interface LegalEnquiry {
  id: string;
  kind: string;
  submitterName: string;
  submitterContact: string;
  subject: string;
  body: string;
  status: string;
  submittedBy: string | null;
  createdAt: string;
  handledBy: string | null;
  handledAt: string | null;
  resolutionNote: string | null;
}

export interface LegalEnquiryReceipt {
  accepted: boolean;
  message: string;
}

export interface LegalEnquiryCount {
  kind: string;
  open: number;
}

const ENQUIRY_FIELDS = `
  id
  kind
  submitterName
  submitterContact
  subject
  body
  status
  submittedBy
  createdAt
  handledBy
  handledAt
  resolutionNote
`;

export async function submitLegalEnquiry(input: {
  kind: LegalEnquiryKind;
  submitterName: string;
  submitterContact: string;
  subject: string;
  body: string;
}): Promise<LegalEnquiryReceipt> {
  const data = await postGraphQL<{ submitLegalEnquiry: LegalEnquiryReceipt }>(
    `mutation SubmitLegalEnquiry(
      $kind: LegalEnquiryKind!
      $submitterName: String!
      $submitterContact: String!
      $subject: String!
      $body: String!
    ) {
      submitLegalEnquiry(
        kind: $kind
        submitterName: $submitterName
        submitterContact: $submitterContact
        subject: $subject
        body: $body
      ) { accepted message }
    }`,
    input,
    { endpoint: GRAPHQL_PUBLIC_ENDPOINT },
  );
  return data.submitLegalEnquiry;
}

export async function fetchLegalEnquiries(
  kind?: LegalEnquiryKind,
  status?: LegalEnquiryStatus,
): Promise<LegalEnquiry[]> {
  const data = await postGraphQL<{ legalEnquiries: LegalEnquiry[] }>(
    `query LegalEnquiries($kind: LegalEnquiryKind, $status: LegalEnquiryStatus) {
      legalEnquiries(kind: $kind, status: $status) { ${ENQUIRY_FIELDS} }
    }`,
    { kind, status },
  );
  return data.legalEnquiries;
}

export async function fetchOpenEnquiryCounts(): Promise<LegalEnquiryCount[]> {
  const data = await postGraphQL<{
    openLegalEnquiryCounts: LegalEnquiryCount[];
  }>(`query OpenLegalEnquiryCounts { openLegalEnquiryCounts { kind open } }`);
  return data.openLegalEnquiryCounts;
}

export async function setLegalEnquiryStatus(
  id: string,
  status: LegalEnquiryStatus,
  resolutionNote?: string,
): Promise<LegalEnquiry> {
  const data = await postGraphQL<{ setLegalEnquiryStatus: LegalEnquiry }>(
    `mutation SetLegalEnquiryStatus(
      $id: UUID!
      $status: LegalEnquiryStatus!
      $resolutionNote: String
    ) {
      setLegalEnquiryStatus(id: $id, status: $status, resolutionNote: $resolutionNote) {
        ${ENQUIRY_FIELDS}
      }
    }`,
    { id, status, resolutionNote },
  );
  return data.setLegalEnquiryStatus;
}
