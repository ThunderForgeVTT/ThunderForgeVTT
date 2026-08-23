import { withCsrf } from "@/api/auth";
import type {
  ModerationActionType,
  ModerationCaseRecord,
  SubmitCounterNoticeInput,
  SubmitTakedownNoticeInput,
} from "@/types/moderation";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";
// `/api/graphql` sits behind a router-level auth gate, so the one mutation
// that must be reachable by an anonymous rights holder (FR-002) is served
// from a separate, unauthenticated route. See `graphql_public_handler` in
// `src/server/src/main.rs`.
const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";

const MODERATION_ACTION_FIELDS = `
  id
  caseId
  actionType
  entityType
  entityId
  worldId
  validityResult
  missingElements
  restorationDueAt
  createdAt
`;

const MODERATION_CASE_FIELDS = `
  caseId
  entityType
  entityId
  worldId
  currentStatus
  events {
    ${MODERATION_ACTION_FIELDS}
  }
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
  endpoint: string = GRAPHQL_ENDPOINT,
): Promise<TData> {
  const response = await fetch(endpoint, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

type SubmitTakedownNoticeMutation = {
  submitTakedownNotice: ModerationCaseRecord;
};

/** Public — no login required (FR-002). */
export function submitTakedownNotice(
  input: SubmitTakedownNoticeInput,
): Promise<ModerationCaseRecord> {
  return postGraphQL<SubmitTakedownNoticeMutation>(
    `
      mutation SubmitTakedownNotice($input: SubmitTakedownNoticeInput!) {
        submitTakedownNotice(input: $input) {
          ${MODERATION_CASE_FIELDS}
        }
      }
    `,
    { input },
    GRAPHQL_PUBLIC_ENDPOINT,
  ).then((data) => data.submitTakedownNotice);
}

type SubmitCounterNoticeMutation = {
  submitCounterNotice: ModerationCaseRecord;
};

export function submitCounterNotice(
  input: SubmitCounterNoticeInput,
): Promise<ModerationCaseRecord> {
  return postGraphQL<SubmitCounterNoticeMutation>(
    `
      mutation SubmitCounterNotice($input: SubmitCounterNoticeInput!) {
        submitCounterNotice(input: $input) {
          ${MODERATION_CASE_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.submitCounterNotice);
}

type ResolveModerationCaseMutation = {
  resolveModerationCase: ModerationCaseRecord;
};

/** Compliance-staff-only. */
export function resolveModerationCase(
  caseId: string,
  resolution: ModerationActionType,
): Promise<ModerationCaseRecord> {
  return postGraphQL<ResolveModerationCaseMutation>(
    `
      mutation ResolveModerationCase($caseId: UUID!, $resolution: ModerationActionType!) {
        resolveModerationCase(caseId: $caseId, resolution: $resolution) {
          ${MODERATION_CASE_FIELDS}
        }
      }
    `,
    { caseId, resolution },
  ).then((data) => data.resolveModerationCase);
}

type ModerationHistoryQuery = {
  moderationHistoryForAccount: ModerationCaseRecord[];
};

/** Compliance-staff-only. */
export function getModerationHistoryForAccount(accountId: string): Promise<ModerationCaseRecord[]> {
  return postGraphQL<ModerationHistoryQuery>(
    `
      query ModerationHistoryForAccount($accountId: UUID!) {
        moderationHistoryForAccount(accountId: $accountId) {
          ${MODERATION_CASE_FIELDS}
        }
      }
    `,
    { accountId },
  ).then((data) => data.moderationHistoryForAccount);
}

type RepeatInfringerFlagsQuery = {
  repeatInfringerFlags: string[];
};

/** Compliance-staff-only. */
export function getRepeatInfringerFlags(): Promise<string[]> {
  return postGraphQL<RepeatInfringerFlagsQuery>(
    `
      query RepeatInfringerFlags {
        repeatInfringerFlags
      }
    `,
  ).then((data) => data.repeatInfringerFlags);
}
