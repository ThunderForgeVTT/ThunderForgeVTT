import { postGraphQL } from "@/api/graphqlClient";
import type { WorldAbilityRecord } from "@/types/ability";
import type {
  AbilityShareLinkRecord,
  DmWorldSummary,
  SharedAbilityPreview,
} from "@/types/abilityShare";

// ADR-071: the share preview is readable without an account, so it goes to
// the unauthenticated route spec 015 built for the takedown notice — the
// same one `sharedCollection` uses. Everything else in this file keeps the
// authenticated endpoint.
const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";

/**
 * Spec 025 US6: ability share links, governed by ADR-049.
 *
 * ⚠️ There is deliberately **no** "list my shares" or "shares in this world"
 * call here, and none may be added. No-enumeration is one of the six
 * invariants the DMCA determination is conditional on — adding a listing
 * re-opens it.
 */

const EFFECT_FIELDS = `
  id
  abilityId
  effectType
  formula
  target
  triggerKind
  sortOrder
`;

/** FR-032: Owner-level only, enforced server-side. */
export function createAbilityShareLink(
  abilityId: string,
): Promise<AbilityShareLinkRecord> {
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

/**
 * ADR-071: readable **with no account at all**. This one call goes to the
 * public endpoint — `/api/graphql` sits behind a router-level auth gate, so
 * pointing it there would make the page require a login and quietly undo the
 * decision.
 *
 * Copying does not go here. Viewing and copying diverge at exactly that call.
 */
export function getSharedAbility(
  shareCode: string,
): Promise<SharedAbilityPreview> {
  return postGraphQL<{ sharedAbility: SharedAbilityPreview }>(
    `
      query SharedAbility($shareCode: String!) {
        sharedAbility(shareCode: $shareCode) {
          name
          description
          classification
          classificationLabel
          effects {
            ${EFFECT_FIELDS}
          }
        }
      }
    `,
    { shareCode },
    { endpoint: GRAPHQL_PUBLIC_ENDPOINT },
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
          classificationLabel
          gmOnly
        }
      }
    `,
    { input: { shareCode, destinationWorldId } },
  ).then((data) => data.copySharedAbilityToWorld);
}
