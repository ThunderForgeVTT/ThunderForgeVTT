import { useEffect, useState } from "react";
import { postGraphQL } from "@/api/graphqlClient";

/** The anonymous transport, as `api/collections.ts` and four others name it. */
const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";
import type { OperatorValues } from "@/legal/operatorTokens";

/**
 * The six operator values the published legal pages render.
 *
 * # Why this query is unauthenticated
 *
 * Spec 039's FR-056: somebody who needs to file a copyright notice against
 * this instance has no account on it, and requiring one to find out who to
 * write to would make the designation undiscoverable by exactly the person it
 * exists for. The query exposes these six values and nothing else about how
 * the instance is configured (spec 040 `contracts/legal-rendering.md` rule 4).
 *
 * # Why a failure is not an error state
 *
 * A page with no values renders its `[OPERATOR — ...]` markers, which is the
 * honest rendering for an instance that has not been configured — and is
 * exactly what these pages showed before this feature. A network failure and
 * an unconfigured instance therefore produce the same page, deliberately: the
 * alternative is a legal page replaced by an error box, which serves nobody
 * who came to it to find a contact address.
 *
 * # Why the request is shared rather than per-component
 *
 * `LegalProse` resolves these itself so that every surface rendering legal
 * text names the operator without each page having to remember to pass them —
 * the terms, the privacy policy, the collection sharing terms at the share
 * step. That is several mounts of the same component on one page, so the
 * in-flight promise is cached at module scope and the query runs once.
 */

const QUERY = `
  query PublishedOperatorValues {
    publishedOperatorValues {
      operatorName
      operatorContactEmail
      operatorJurisdiction
      noticeContactName
      noticeContactEmail
      noticeContactPostalAddress
    }
  }
`;

interface PublishedOperatorValuesData {
  publishedOperatorValues: {
    operatorName: string | null;
    operatorContactEmail: string | null;
    operatorJurisdiction: string | null;
    noticeContactName: string | null;
    noticeContactEmail: string | null;
    noticeContactPostalAddress: string | null;
  } | null;
}

const NONE: OperatorValues = {};

let inFlight: Promise<OperatorValues> | null = null;

/**
 * Fetch the values, resolving to `{}` for any failure.
 *
 * The cached promise is never cleared: these change when an operator edits
 * instance settings, which is not something a reader of the terms of service
 * does mid-session, and a stale contact for the length of one page load is a
 * far smaller problem than re-querying on every card.
 */
export function fetchPublishedOperatorValues(): Promise<OperatorValues> {
  // `/api/graphql/public`, not `/api/graphql`. The default endpoint is wrapped
  // in `require_authenticated_user`, so this query — whose entire reason for
  // existing is that the reader has no account — came back 401 and the legal
  // pages silently fell back to their unset markers. The failure was invisible
  // because the fallback is *correct behaviour* for an unconfigured instance:
  // a configured one looked identical to one nobody had set up.
  //
  // Found by the first-run e2e; the five other anonymous readers
  // (`collections`, `abilityShares`, `itemShares`, `actorShares`,
  // `moderation`) already name this endpoint, and this is now the sixth.
  inFlight ??= postGraphQL<PublishedOperatorValuesData>(QUERY, undefined, {
    endpoint: GRAPHQL_PUBLIC_ENDPOINT,
  })
    .then((data) => {
      const v = data.publishedOperatorValues;
      if (!v) {
        return NONE;
      }
      return {
        "operator.name": v.operatorName,
        "operator.contact_email": v.operatorContactEmail,
        "operator.jurisdiction": v.operatorJurisdiction,
        "notice.contact_name": v.noticeContactName,
        "notice.contact_email": v.noticeContactEmail,
        "notice.contact_postal_address": v.noticeContactPostalAddress,
      } satisfies OperatorValues;
    })
    .catch(() => NONE);
  return inFlight;
}

/** Test seam: forget the cached request. */
export function resetPublishedOperatorValues(): void {
  inFlight = null;
}

/**
 * The resolved values, starting empty.
 *
 * Starting empty means the first paint shows the markers and a later one shows
 * the operator. That flicker is accepted rather than blocking the page on a
 * network call: a legal page must render its text even when the query never
 * answers, which is the same reason the failure path resolves to `{}`.
 */
export function usePublishedOperatorValues(): OperatorValues {
  const [values, setValues] = useState<OperatorValues>(NONE);

  useEffect(() => {
    let isActive = true;
    void fetchPublishedOperatorValues().then((next) => {
      if (isActive) {
        setValues(next);
      }
    });
    return () => {
      isActive = false;
    };
  }, []);

  return values;
}
