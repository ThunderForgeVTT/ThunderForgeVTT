import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 036 US4: the sessions this account holds, now that it may hold several.
 *
 * # Why there is a list at all
 *
 * Signing in used to end every other session. It no longer does — that was the
 * defect spec 036 exists to fix, because one person with a sheet on a second
 * screen is the ordinary case, not an attack. But an account that can hold ten
 * live sessions and cannot see them has traded one problem for a worse one:
 * "am I signed in somewhere I should not be" became unanswerable.
 *
 * # What a session says about itself
 *
 * A coarse client description and two timestamps. **Never an address** —
 * spec 035's rule, recorded here because a session list is the most tempting
 * place in the product to put one: "Chrome on Linux · 2 hours ago" is useful,
 * and "Chrome on Linux · 2 hours ago · 81.2.44.19" is a location history the
 * instance did not need to keep.
 */

export interface UserSession {
  id: string;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string;
  /** Coarse origin — a browser and a platform, never an address. */
  clientDescription: string | null;
  /** True for the session making this request. */
  isCurrent: boolean;
}

const FIELDS = `
  id
  createdAt
  lastSeenAt
  expiresAt
  clientDescription
  isCurrent
`;

export async function getMySessions(): Promise<UserSession[]> {
  const data = await postGraphQL<{ mySessions: UserSession[] }>(
    `query MySessions { mySessions { ${FIELDS} } }`,
  );
  return data.mySessions;
}

/** End one session. Ending the current one signs this browser out. */
export async function endSession(sessionId: string): Promise<boolean> {
  const data = await postGraphQL<{ endSession: boolean }>(
    `mutation EndSession($sessionId: UUID!) { endSession(sessionId: $sessionId) }`,
    { sessionId },
  );
  return data.endSession;
}

/** End every session except this one. Returns how many were ended. */
export async function endAllSessions(): Promise<number> {
  const data = await postGraphQL<{ endAllSessions: number }>(
    `mutation EndAllSessions { endAllSessions }`,
  );
  return data.endAllSessions;
}
