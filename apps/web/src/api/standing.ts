import { postGraphQL } from "@/api/graphqlClient";
import type { ModerationEntityType } from "@/types/moderation";

/**
 * Spec 039 US5 and US7: where an account stands, what it has been told, and —
 * at the last rung — the window and its two remedies.
 *
 * Everything on `Standing` except `termination` is derived by the server on
 * every read, so there is no count here that can go stale.
 */
export interface Strike {
  caseId: string;
  /** A string as well as the union: the server knows `WORLD_ABILITY` too. */
  entityType: ModerationEntityType | string;
  entityId: string;
  worldId: string;
  recordedAt: string;
  /** When this strike stops counting. */
  agesOutAt: string;
}

/** A termination window (US7). */
export interface Termination {
  openedAt: string;
  deletionDueAt: string;
  strikeCountAtOpen: number;
  appealState: "none" | "open" | "upheld" | "rejected" | string;
  /** A person decides the end, not a timer. */
  requiresHuman: boolean;
  /** False only for the instance's last administrator. */
  disablesAccount: boolean;
  appealStatement: string | null;
  appealFiledAt: string | null;
  appealResolvedAt: string | null;
  appealNote: string | null;
  /** Past its date with no appeal open. */
  due: boolean;
}

export interface Standing {
  strikes: Strike[];
  strikeCount: number;
  warnAt: number;
  suspendPublishingAt: number;
  threshold: number;
  warned: boolean;
  mayPublish: boolean;
  disabled: boolean;
  termination: Termination | null;
}

/**
 * A notice's words are not stored — `kind` and `payload` are rendered here,
 * so a wording fix reaches every notice ever written.
 */
export interface AccountNotice {
  id: string;
  kind: string;
  subjectRef: Record<string, unknown> | null;
  payload: Record<string, unknown> | null;
  createdAt: string;
  readAt: string | null;
}

const TERMINATION_FIELDS = `
  openedAt
  deletionDueAt
  strikeCountAtOpen
  appealState
  requiresHuman
  disablesAccount
  appealStatement
  appealFiledAt
  appealResolvedAt
  appealNote
  due
`;

const STANDING_FIELDS = `
  strikes {
    caseId
    entityType
    entityId
    worldId
    recordedAt
    agesOutAt
  }
  strikeCount
  warnAt
  suspendPublishingAt
  threshold
  warned
  mayPublish
  disabled
  termination {
    ${TERMINATION_FIELDS}
  }
`;

/**
 * The caller's own standing and notices, in one request. Reachable by a
 * disabled account — it is the page its remedies are on.
 */
export function readMyStanding(): Promise<{
  standing: Standing;
  notices: AccountNotice[];
}> {
  return postGraphQL<{ myStanding: Standing; myNotices: AccountNotice[] }>(`
    query MyStanding {
      myStanding {
        ${STANDING_FIELDS}
      }
      myNotices {
        id
        kind
        subjectRef
        payload
        createdAt
        readAt
      }
    }
  `).then((data) => ({ standing: data.myStanding, notices: data.myNotices }));
}

/** The appeal (FR-031). One per window, in the person's own words. */
export function fileAppeal(statement: string): Promise<Termination> {
  return postGraphQL<{ fileAppeal: Termination }>(
    `
      mutation FileAppeal($statement: String!) {
        fileAppeal(statement: $statement) {
          ${TERMINATION_FIELDS}
        }
      }
    `,
    { statement },
  ).then((data) => data.fileAppeal);
}

/** Anybody's standing. Admin only. */
export function getAccountStanding(accountId: string): Promise<Standing> {
  return postGraphQL<{ accountStanding: Standing }>(
    `
      query AccountStanding($accountId: UUID!) {
        accountStanding(accountId: $accountId) {
          ${STANDING_FIELDS}
        }
      }
    `,
    { accountId },
  ).then((data) => data.accountStanding);
}

/** Decide an appeal. Admin only. */
export function resolveAppeal(
  accountId: string,
  upheld: boolean,
  note: string | null,
): Promise<Termination> {
  return postGraphQL<{ resolveAppeal: Termination }>(
    `
      mutation ResolveAppeal($accountId: UUID!, $upheld: Boolean!, $note: String) {
        resolveAppeal(accountId: $accountId, upheld: $upheld, note: $note) {
          ${TERMINATION_FIELDS}
        }
      }
    `,
    { accountId, upheld, note },
  ).then((data) => data.resolveAppeal);
}

/** Carry out a window that waits for a person. Admin only. */
export function executeTermination(accountId: string): Promise<boolean> {
  return postGraphQL<{ executeTermination: boolean }>(
    `
      mutation ExecuteTermination($accountId: UUID!) {
        executeTermination(accountId: $accountId)
      }
    `,
    { accountId },
  ).then((data) => data.executeTermination);
}
