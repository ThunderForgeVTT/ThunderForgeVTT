/**
 * Spec 088 (FR-008): why a world link admits no one, as the join page says
 * it. The server's message is the body (contracts/graphql.md); this adds a
 * heading per case. Spec 027 gave every dead link one message; the owner
 * chose to say which instead, and the rate limit on `joinWorld` is what
 * keeps an unknown code from being a useful oracle.
 */
export type JoinRefusalKind =
  | "LINK_REVOKED"
  | "LINK_EXPIRED"
  | "LINK_USED_UP"
  | "LINK_UNKNOWN"
  | "ALREADY_MEMBER";

export interface JoinRefusal {
  kind: JoinRefusalKind;
  heading: string;
  message: string;
}

const HEADINGS: Record<JoinRefusalKind, string> = {
  LINK_REVOKED: "This link was withdrawn",
  LINK_EXPIRED: "This link has expired",
  LINK_USED_UP: "This link has been used",
  LINK_UNKNOWN: "No world behind this link",
  ALREADY_MEMBER: "You're already in this world.",
};

function isKind(code: string): code is JoinRefusalKind {
  return code in HEADINGS;
}

/** The refusal behind a request's error codes, or `null` for anything else. */
export function joinRefusalOf(
  codes: readonly string[],
  message: string,
): JoinRefusal | null {
  const kind = codes.find(isKind);
  return kind ? { kind, heading: HEADINGS[kind], message } : null;
}
