import { postGraphQL } from "@/api/graphqlClient";
import type { ModerationEntityType } from "@/types/moderation";

/**
 * Spec 039 US5: where an account stands, and what it has been told.
 *
 * Everything on `Standing` is derived by the server on every read, so there is
 * no count here that can go stale — reload and it is the truth.
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

export interface Standing {
  strikes: Strike[];
  strikeCount: number;
  warnAt: number;
  suspendPublishingAt: number;
  threshold: number;
  warned: boolean;
  mayPublish: boolean;
  disabled: boolean;
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

/** The caller's own standing and notices, in one request. */
export function readMyStanding(): Promise<{
  standing: Standing;
  notices: AccountNotice[];
}> {
  return postGraphQL<{ myStanding: Standing; myNotices: AccountNotice[] }>(`
    query MyStanding {
      myStanding {
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
