/**
 * The one place this app talks to the feedback service (spec 037,
 * `contracts/feedback.md`).
 *
 * # There is no server surface yet
 *
 * `submitFeedback`, `feedbackDestinationNotice` and their types are specified
 * in `specs/037-in-app-feedback/contracts/feedback.md` and **are not
 * implemented in `src/server` at the time of writing**. Everything below posts
 * exactly what that contract declares, so the day the resolvers land nothing
 * in this app changes — no component knows the shape of the request, and no
 * component will need editing when the field names become real.
 *
 * Until then {@link submitFeedback} rejects with a `GraphQLRequestError`
 * naming an unknown field, and the dialog treats it as it treats any refusal:
 * the draft survives and the person is told plainly. That is a deliberately
 * visible failure rather than a stub that resolves and loses what somebody
 * wrote.
 *
 * # Refusals are read from codes, not from messages
 *
 * `FEEDBACK_RATE_LIMITED` and `FEEDBACK_CONTAINS_SECRET` are the two the
 * client must tell apart from a genuine error, and both arrive as
 * `extensions.code`. Matching on the human-readable message would break the
 * first time somebody reworded it.
 */

import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
import type { SubmitFeedbackInput } from "@/services/feedbackPayload";

/** The submission was refused because the person is going too fast (FR-006). */
export const FEEDBACK_RATE_LIMITED = "FEEDBACK_RATE_LIMITED";

/**
 * The server's redaction validator matched an approved attachment.
 *
 * It refuses rather than rewrites, on purpose: an edit after approval would
 * mean the person saw something other than what was sent, which is the
 * promise FR-012 exists to keep (`contracts/attachments.md` § 2).
 */
export const FEEDBACK_CONTAINS_SECRET = "FEEDBACK_CONTAINS_SECRET";

export type FeedbackDeliveryState = "PENDING" | "DELIVERED" | "ABANDONED";

/** What the mutation returns — recorded, not yet delivered (FR-018). */
export interface FeedbackSubmissionRecord {
  id: string;
  kind: string;
  createdAt: string;
  deliveryState: FeedbackDeliveryState;
  attachmentsExpireAt: string;
}

/**
 * What the person must be told **before** they submit (FR-014), read at
 * form-open time so the notice is on screen while they decide.
 */
export interface FeedbackDestinationNotice {
  /** False when nothing is configured. Submission still works (FR-030). */
  configured: boolean;
  repository: string | null;
  /** Determined from the host, never assumed. Null means never checked. */
  isPublic: boolean | null;
  visibilityCheckedAt: string | null;
}

/**
 * The notice shown when the instance could not be asked.
 *
 * "Unknown" rather than "private": FR-014 requires visibility to be
 * *determined*, and the safe rendering of an unknown destination is to warn,
 * not to reassure.
 */
export const UNKNOWN_DESTINATION: FeedbackDestinationNotice = {
  configured: false,
  repository: null,
  isPublic: null,
  visibilityCheckedAt: null,
};

const SUBMIT_FEEDBACK = `
  mutation SubmitFeedback($input: SubmitFeedbackInput!) {
    submitFeedback(input: $input) {
      id
      kind
      createdAt
      deliveryState
      attachmentsExpireAt
    }
  }
`;

const DESTINATION_NOTICE = `
  query FeedbackDestinationNotice {
    feedbackDestinationNotice {
      configured
      repository
      isPublic
      visibilityCheckedAt
    }
  }
`;

/**
 * Record a submission.
 *
 * One function, taking the payload the review approved, unaltered. Nothing
 * between the review and the wire touches it — that is not a stylistic
 * preference, it is `contracts/attachments.md`'s whole claim, and a transform
 * added here would be invisible to every test that only reads the review.
 */
export function submitFeedback(
  input: SubmitFeedbackInput,
): Promise<FeedbackSubmissionRecord> {
  return postGraphQL<{ submitFeedback: FeedbackSubmissionRecord }>(
    SUBMIT_FEEDBACK,
    { input },
  ).then((data) => data.submitFeedback);
}

/**
 * The pre-submission notice.
 *
 * Resolves to {@link UNKNOWN_DESTINATION} rather than rejecting when the
 * query is unavailable — which today it always is. A form that refused to
 * open because it could not describe the destination would collect nothing,
 * and FR-030 says an unconfigured instance still collects feedback.
 */
export async function getFeedbackDestinationNotice(): Promise<FeedbackDestinationNotice> {
  try {
    const data = await postGraphQL<{
      feedbackDestinationNotice: FeedbackDestinationNotice;
    }>(DESTINATION_NOTICE);
    return data.feedbackDestinationNotice;
  } catch {
    return UNKNOWN_DESTINATION;
  }
}

/** Whether a rejection is the rate limiter (FR-006) rather than a failure. */
export function isRateLimited(error: unknown): boolean {
  return (
    error instanceof GraphQLRequestError && error.hasCode(FEEDBACK_RATE_LIMITED)
  );
}

/** Whether the server's validator refused an attachment (FR-012). */
export function containsSecret(error: unknown): boolean {
  return (
    error instanceof GraphQLRequestError &&
    error.hasCode(FEEDBACK_CONTAINS_SECRET)
  );
}
