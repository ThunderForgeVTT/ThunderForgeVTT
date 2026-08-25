import { withCsrf } from "@/api/auth";
import type { WorldAbilityRecord } from "@/types/ability";
import type {
  AbilityShareLinkRecord,
  DmWorldSummary,
  SharedAbilityPreview,
} from "@/types/abilityShare";

/**
 * Spec 025 US6: ability share links, governed by ADR-049.
 *
 * ⚠️ There is deliberately **no** "list my shares" or "shares in this world"
 * call here, and none may be added. No-enumeration is one of the six
 * invariants the DMCA determination is conditional on — adding a listing
 * re-opens it.
 */

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: { message?: string }[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const EFFECT_FIELDS = `
  id
  abilityId
  effectType
  formula
  target
  triggerKind
  sortOrder
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({ query, variables }),
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

/** FR-032: Owner-level only, enforced server-side. */
export function createAbilityShareLink(abilityId: string): Promise<AbilityShareLinkRecord> {
  return postGraphQL<{ createAbilityShareLink: AbilityShareLinkRecord }>(
    `
      mutation CreateAbilityShareLink($abilityId: UUID!) {
        createAbilityShareLink(abilityId: $abilityId) {
          id
          abilityId
          shareCode
          revoked
          createdAt
        }
      }
    `,
    { abilityId },
  ).then((data) => data.createAbilityShareLink);
}

/** FR-036: a soft revoke — the link resolves to a distinct unavailable state
 * afterward rather than looking like a code that never existed. */
export function revokeAbilityShareLink(shareId: string): Promise<boolean> {
  return postGraphQL<{ revokeAbilityShareLink: boolean }>(
    `
      mutation RevokeAbilityShareLink($shareId: UUID!) {
        revokeAbilityShareLink(shareId: $shareId)
      }
    `,
    { shareId },
  ).then((data) => data.revokeAbilityShareLink);
}

/** Requires login but NOT world membership — that is the point of a share. */
export function getSharedAbility(shareCode: string): Promise<SharedAbilityPreview> {
  return postGraphQL<{ sharedAbility: SharedAbilityPreview }>(
    `
      query SharedAbility($shareCode: String!) {
        sharedAbility(shareCode: $shareCode) {
          name
          description
          classification
          effects {
            ${EFFECT_FIELDS}
          }
        }
      }
    `,
    { shareCode },
  ).then((data) => data.sharedAbility);
}

/** Reuses the world-agnostic query the actor/item share pages already use. */
export function getMyDmWorlds(): Promise<DmWorldSummary[]> {
  return postGraphQL<{ myDmWorlds: DmWorldSummary[] }>(
    `
      query MyDmWorlds {
        myDmWorlds {
          id
          name
        }
      }
    `,
  ).then((data) => data.myDmWorlds);
}

/** FR-035: a one-time deep copy into a world the viewer DMs. The copy is
 * fully independent — no live link back to the source. */
export function copySharedAbilityToWorld(
  shareCode: string,
  destinationWorldId: string,
): Promise<WorldAbilityRecord> {
  return postGraphQL<{ copySharedAbilityToWorld: WorldAbilityRecord }>(
    `
      mutation CopySharedAbilityToWorld($input: CopySharedAbilityInput!) {
        copySharedAbilityToWorld(input: $input) {
          id
          worldId
          name
          classification
          gmOnly
        }
      }
    `,
    { input: { shareCode, destinationWorldId } },
  ).then((data) => data.copySharedAbilityToWorld);
}
