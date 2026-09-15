// Spec 046: attacks, offers and auto-apply
// (src/server/src/graphql/mutations_attacks.rs, queries/attacks.rs).
//
// The server rolls, judges and redacts. Nothing here computes a result, a hit
// or a name: every answer is the server's, built for the person asking.

import { postGraphQL } from "@/api/graphqlClient";
import type { CombatRecord } from "@/types/combat";
import type {
  AttackFields,
  AttackInput,
  AttackPreviewRecord,
  AttackRecord,
  OfferRecord,
} from "@/types/attack";

const ROLL_FIELDS = `
  formula
  dice { sidesKind numericSides rolls kept finalValue }
  resultKind
  resultValue
`;

const OFFER_FIELDS = `
  id
  sceneId
  attackId
  kind
  amount
  target { tokenId label }
  status
  resolvedBy
  resolvedOnBehalf
  mayResolve
  createdAt
`;

const ATTACK_FIELDS = `
  id
  sceneId
  attacker { tokenId label }
  target { tokenId label }
  abilityName
  toHit { ${ROLL_FIELDS} }
  damage { ${ROLL_FIELDS} }
  defence
  outcome
  distance
  flags
  actionCost
  offer { ${OFFER_FIELDS} }
  multiattackOf
  createdAt
`;

/** Make an attack. Several for a multiattack. */
export function makeAttack(input: AttackInput): Promise<AttackRecord[]> {
  return postGraphQL<{ makeAttack: AttackRecord[] }>(
    `
      mutation MakeAttack($input: AttackInput!) {
        makeAttack(input: $input) {
          ${ATTACK_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.makeAttack);
}

/** The warning before rolling (FR-033). Refuses nothing, writes nothing. */
export function previewAttack(
  input: AttackInput,
): Promise<AttackPreviewRecord> {
  return postGraphQL<{ previewAttack: AttackPreviewRecord }>(
    `
      query PreviewAttack($input: AttackInput!) {
        previewAttack(input: $input) {
          distance
          flags
          turn { allowed activeLabel }
        }
      }
    `,
    { input },
  ).then((data) => data.previewAttack);
}

/** One attack, as this viewer may see it. */
export function getAttack(id: string): Promise<AttackRecord | null> {
  return postGraphQL<{ attack: AttackRecord | null }>(
    `
      query Attack($id: UUID!) {
        attack(id: $id) {
          ${ATTACK_FIELDS}
        }
      }
    `,
    { id },
  ).then((data) => data.attack);
}

/** A scene's attacks, newest first, fifty a page. */
export function getSceneAttacks(
  sceneId: string,
  before?: string | null,
): Promise<AttackRecord[]> {
  return postGraphQL<{ sceneAttacks: AttackRecord[] }>(
    `
      query SceneAttacks($sceneId: UUID!, $before: UUID) {
        sceneAttacks(sceneId: $sceneId, before: $before) {
          ${ATTACK_FIELDS}
        }
      }
    `,
    { sceneId, before: before ?? null },
  ).then((data) => data.sceneAttacks);
}

/** The caller's pending offers; every one in the world for a Game Master. */
export function getPendingOffers(worldId: string): Promise<OfferRecord[]> {
  return postGraphQL<{ pendingOffers: OfferRecord[] }>(
    `
      query PendingOffers($worldId: UUID!) {
        pendingOffers(worldId: $worldId) {
          ${OFFER_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.pendingOffers);
}

/** Take or decline an offer, once. */
export function resolveOffer(
  offerId: string,
  take: boolean,
): Promise<OfferRecord> {
  return postGraphQL<{ resolveOffer: OfferRecord }>(
    `
      mutation ResolveOffer($offerId: UUID!, $take: Boolean!) {
        resolveOffer(offerId: $offerId, take: $take) {
          ${OFFER_FIELDS}
        }
      }
    `,
    { offerId, take },
  ).then((data) => data.resolveOffer);
}

/** Game Master only: the world's auto-apply default for their NPCs. */
export function updateWorldAutoApplyNpcDamage(
  worldId: string,
  enabled: boolean,
): Promise<boolean> {
  return postGraphQL<{
    updateWorldAutoApplyNpcDamage: { autoApplyNpcDamage: boolean };
  }>(
    `
      mutation UpdateWorldAutoApplyNpcDamage(
        $input: UpdateWorldAutoApplyNpcDamageInput!
      ) {
        updateWorldAutoApplyNpcDamage(input: $input) {
          autoApplyNpcDamage
        }
      }
    `,
    { input: { worldId, enabled } },
  ).then((data) => data.updateWorldAutoApplyNpcDamage.autoApplyNpcDamage);
}

/** Game Master only: whether a world applies hits on its NPCs by default. */
export function getWorldAutoApplyNpcDamage(worldId: string): Promise<boolean> {
  return postGraphQL<{ world: { autoApplyNpcDamage: boolean } | null }>(
    `
      query WorldAutoApply($id: UUID!) {
        world(id: $id) {
          autoApplyNpcDamage
        }
      }
    `,
    { id: worldId },
  ).then((data) => data.world?.autoApplyNpcDamage ?? false);
}

/**
 * Game Master only: this encounter's override. `null` returns it to the
 * world's setting.
 */
export function setCombatAutoApply(
  combatId: string,
  enabled: boolean | null,
): Promise<Pick<CombatRecord, "id"> & AutoApplyState> {
  return postGraphQL<{
    setCombatAutoApply: Pick<CombatRecord, "id"> & AutoApplyState;
  }>(
    `
      mutation SetCombatAutoApply($combatId: UUID!, $enabled: Boolean) {
        setCombatAutoApply(combatId: $combatId, enabled: $enabled) {
          id
          autoApply
          effectiveAutoApply
        }
      }
    `,
    { combatId, enabled },
  ).then((data) => data.setCombatAutoApply);
}

export interface AutoApplyState {
  autoApply: boolean | null;
  effectiveAutoApply: boolean;
}

/** What an ability is as an attack. Editor on the ability. */
export function setAbilityAttack(
  abilityId: string,
  attack: AttackFields,
): Promise<boolean> {
  return postGraphQL<{ setAbilityAttack: boolean }>(
    `
      mutation SetAbilityAttack($abilityId: UUID!, $attack: AttackFieldsInput!) {
        setAbilityAttack(abilityId: $abilityId, attack: $attack)
      }
    `,
    { abilityId, attack },
  ).then((data) => data.setAbilityAttack);
}

/** What an item is as an attack. Editor on the item. */
export function setItemAttack(
  itemId: string,
  attack: AttackFields,
): Promise<boolean> {
  return postGraphQL<{ setItemAttack: boolean }>(
    `
      mutation SetItemAttack($itemId: UUID!, $attack: AttackFieldsInput!) {
        setItemAttack(itemId: $itemId, attack: $attack)
      }
    `,
    { itemId, attack },
  ).then((data) => data.setItemAttack);
}
