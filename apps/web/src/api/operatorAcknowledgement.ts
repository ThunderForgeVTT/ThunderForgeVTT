import type { Attestation } from "@/api/attestations";
import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 039 T085 (FR-044): where this instance's operator acknowledgement
 * stands against the words it ships. Admin only.
 */
export interface OperatorAcknowledgementState {
  /** The most recent acknowledgement, of any version, in the words it was made to. */
  acknowledgement: Attestation | null;
  /** The operator statement this build ships. */
  currentVersionId: string;
  /** False after an upgrade that changed the words, until acknowledged. */
  isCurrent: boolean;
}

const STATE_FIELDS = `
  acknowledgement {
    id
    subjectUsername
    attestedAt
    termsVersionId
  }
  currentVersionId
  isCurrent
`;

export function readOperatorAcknowledgement(): Promise<OperatorAcknowledgementState> {
  return postGraphQL<{
    instanceOperatorAcknowledgement: OperatorAcknowledgementState;
  }>(`
    query OperatorAcknowledgement {
      instanceOperatorAcknowledgement {
        ${STATE_FIELDS}
      }
    }
  `).then((data) => data.instanceOperatorAcknowledgement);
}

/** Acknowledge the statement this build ships. */
export function acknowledgeOperatorStatement(
  termsVersionId: string,
): Promise<OperatorAcknowledgementState> {
  return postGraphQL<{
    acknowledgeOperatorStatement: OperatorAcknowledgementState;
  }>(
    `
      mutation AcknowledgeOperatorStatement($attestation: AttestationInput!) {
        acknowledgeOperatorStatement(attestation: $attestation) {
          ${STATE_FIELDS}
        }
      }
    `,
    { attestation: { termsVersionId } },
  ).then((data) => data.acknowledgeOperatorStatement);
}
