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
      legalTermsChangeNotice
      legalCommunityAddendum
      legalMinimumAgeStatement
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
    legalTermsChangeNotice: string | null;
    legalCommunityAddendum: string | null;
    legalMinimumAgeStatement: string | null;
  } | null;
}

/**
 * The prose an operator wrote for their own terms.
 *
 * Kept apart from `OperatorValues` on purpose. Those are *tokens*: each one
 * has a place in a shipped document and a marker to show while unset. These
 * have neither — they are whole paragraphs, they belong to no `{{token}}`, and
 * an unwritten one renders nothing at all rather than a marker. Folding them
 * into the token set would have given each a marker it must never show.
 */
export interface OperatorProse {
  termsChangeNotice: string;
  communityAddendum: string;
  minimumAgeStatement: string;
}

interface Published {
  values: OperatorValues;
  prose: OperatorProse;
}

const NONE: OperatorValues = {};
const NO_PROSE: OperatorProse = {
  termsChangeNotice: "",
  communityAddendum: "",
  minimumAgeStatement: "",
};
const NOTHING: Published = { values: NONE, prose: NO_PROSE };

let inFlight: Promise<Published> | null = null;

/**
 * Fetch the values, resolving to `{}` for any failure.
 *
 * The cached promise is never cleared: these change when an operator edits
 * instance settings, which is not something a reader of the terms of service
 * does mid-session, and a stale contact for the length of one page load is a
 * far smaller problem than re-querying on every card.
 */
export function fetchPublishedOperatorValues(): Promise<OperatorValues> {
  return fetchPublished().then((published) => published.values);
}

/** The operator's own prose, from the same single request. */
export function fetchPublishedOperatorProse(): Promise<OperatorProse> {
  return fetchPublished().then((published) => published.prose);
}

function fetchPublished(): Promise<Published> {
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
        return NOTHING;
      }
      // An instance that is not open keeps the operator from a caller with no
      // session, so an answer without one is not kept: the same reader asks
      // again once they have signed in, without having to reload the page.
      if (!v.operatorName) {
        inFlight = null;
      }
      const values = {
        "operator.name": v.operatorName,
        "operator.contact_email": v.operatorContactEmail,
        "operator.jurisdiction": v.operatorJurisdiction,
        "notice.contact_name": v.noticeContactName,
        "notice.contact_email": v.noticeContactEmail,
        "notice.contact_postal_address": v.noticeContactPostalAddress,
      } satisfies OperatorValues;
      return {
        values,
        prose: {
          termsChangeNotice: (v.legalTermsChangeNotice ?? "").trim(),
          communityAddendum: (v.legalCommunityAddendum ?? "").trim(),
          minimumAgeStatement: (v.legalMinimumAgeStatement ?? "").trim(),
        },
      };
    })
    .catch(() => NOTHING);
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

/**
 * The operator's own prose, starting empty.
 *
 * Empty is also what a failure and an operator who wrote nothing resolve to,
 * and all three render the same way: nothing.
 */
export function usePublishedOperatorProse(): OperatorProse {
  const [prose, setProse] = useState<OperatorProse>(NO_PROSE);

  useEffect(() => {
    let isActive = true;
    void fetchPublishedOperatorProse().then((next) => {
      if (isActive) {
        setProse(next);
      }
    });
    return () => {
      isActive = false;
    };
  }, []);

  return prose;
}
