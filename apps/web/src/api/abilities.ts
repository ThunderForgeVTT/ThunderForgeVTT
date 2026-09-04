import { postGraphQL } from "@/api/graphqlClient";
import type {
  AbilityClassification,
  AbilityEffectRecord,
  AbilityEffectTrigger,
  AbilityEffectType,
  AbilityPermissionRecord,
  WorldAbilityRecord,
} from "@/types/ability";

/**
 * Spec 025 (T024): the Ability GraphQL client.
 *
 * **Argument shapes are per-operation and must match the resolver exactly**
 * (research.md §5). This codebase has a documented bug class here: spec 005
 * found five separate calls sending flat arguments where the resolver expected
 * a single `input` object, silently breaking the invite panel and join flow.
 * The rule is to write the resolver first, then match the query string to it —
 * so, mirroring `mutations_abilities.rs`:
 *
 *   * `input:` object  → createAbility, updateAbility
 *   * flat scalar args → deleteAbility, setAbilityGmOnly, and every query
 */

const ABILITY_EFFECT_FIELDS = `
  id
  abilityId
  effectType
  formula
  target
  triggerKind
  sortOrder
`;

const WORLD_ABILITY_FIELDS = `
  id
  worldId
  name
  description
  classification
  grade
  gmOnly
  effects {
    ${ABILITY_EFFECT_FIELDS}
  }
  myPermissionLevel
  moderated
  moderationCaseId
  createdAt
  updatedAt
  linkedFromLore {
    id
    title
    slug
  }
`;

/**
 * FR-005: every world member may browse. GM-only abilities are filtered
 * server-side for non-DMs (FR-024b) — there is no client-side visibility
 * filtering to do, and none should be added.
 */
export function getWorldAbilities(
  worldId: string,
  search?: string,
): Promise<WorldAbilityRecord[]> {
  return postGraphQL<{ worldAbilities: WorldAbilityRecord[] }>(
    `
      query WorldAbilities($worldId: UUID!, $search: String) {
        worldAbilities(worldId: $worldId, search: $search) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { worldId, search },
  ).then((data) => data.worldAbilities);
}

/** FR-025: a GM-only ability errors identically to a nonexistent one for a
 * non-DM, so callers must not try to distinguish the two. */
export function getAbility(abilityId: string): Promise<WorldAbilityRecord> {
  return postGraphQL<{ ability: WorldAbilityRecord }>(
    `
      query Ability($abilityId: UUID!) {
        ability(abilityId: $abilityId) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { abilityId },
  ).then((data) => data.ability);
}

/** FR-007: advisory "did you mean?" only — never gates creation. */
export function suggestAbilityName(
  worldId: string,
  name: string,
): Promise<WorldAbilityRecord[]> {
  return postGraphQL<{ suggestAbilityName: WorldAbilityRecord[] }>(
    `
      query SuggestAbilityName($worldId: UUID!, $name: String!) {
        suggestAbilityName(worldId: $worldId, name: $name) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { worldId, name },
  ).then((data) => data.suggestAbilityName);
}

export type CreateAbilityInput = {
  worldId: string;
  name: string;
  description?: string | null;
  classification: AbilityClassification;
  grade?: number | null;
  gmOnly?: boolean;
};

/** FR-002: DM-only, enforced server-side. */
export function createAbility(
  input: CreateAbilityInput,
): Promise<WorldAbilityRecord> {
  return postGraphQL<{ createAbility: WorldAbilityRecord }>(
    `
      mutation CreateAbility($input: CreateAbilityInput!) {
        createAbility(input: $input) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createAbility);
}

export type UpdateAbilityInput = {
  abilityId: string;
  name?: string;
  description?: string | null;
  classification?: AbilityClassification;
  grade?: number | null;
  /**
   * Explicit clear. `description: null` alone cannot mean "clear it", because
   * an omitted field is also null over the wire — which is precisely why
   * `updateItem` (spec 013) can never clear a description once set. Pass
   * `clearDescription: true` to actually blank it.
   */
  clearDescription?: boolean;
};

export function updateAbility(
  input: UpdateAbilityInput,
): Promise<WorldAbilityRecord> {
  return postGraphQL<{ updateAbility: WorldAbilityRecord }>(
    `
      mutation UpdateAbility($input: UpdateAbilityInput!) {
        updateAbility(input: $input) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateAbility);
}

/** Requires Owner on the ability. Never blocked by references — actor
 * known-ability entries and lore links tombstone instead (FR-023, FR-031). */
export function deleteAbility(abilityId: string): Promise<boolean> {
  return postGraphQL<{ deleteAbility: boolean }>(
    `
      mutation DeleteAbility($abilityId: UUID!) {
        deleteAbility(abilityId: $abilityId)
      }
    `,
    { abilityId },
  ).then((data) => data.deleteAbility);
}

/**
 * FR-024c: **DM-only** — Owner-level permission on the ability is not
 * sufficient. Its own mutation rather than a field on `updateAbility`, which
 * needs only Editor and would otherwise let an Editor un-hide a GM's secret.
 */
export function setAbilityGmOnly(
  abilityId: string,
  gmOnly: boolean,
): Promise<WorldAbilityRecord> {
  return postGraphQL<{ setAbilityGmOnly: WorldAbilityRecord }>(
    `
      mutation SetAbilityGmOnly($abilityId: UUID!, $gmOnly: Boolean!) {
        setAbilityGmOnly(abilityId: $abilityId, gmOnly: $gmOnly) {
          ${WORLD_ABILITY_FIELDS}
        }
      }
    `,
    { abilityId, gmOnly },
  ).then((data) => data.setAbilityGmOnly);
}

export type AbilityEffectInput = {
  effectType: AbilityEffectType;
  formula: string;
  target: string;
  triggerKind?: AbilityEffectTrigger | null;
  sortOrder?: number;
};

/**
 * FR-017/FR-018. Flat args, matching `add_ability_effect`'s resolver signature
 * — effect mutations deliberately do NOT take an `input:` object (see the
 * argument-shape note at the top of this file).
 *
 * Permission is checked against the parent ability (Editor), not the effect.
 */
export function addAbilityEffect(
  abilityId: string,
  effect: AbilityEffectInput,
): Promise<AbilityEffectRecord> {
  return postGraphQL<{ addAbilityEffect: AbilityEffectRecord }>(
    `
      mutation AddAbilityEffect($abilityId: UUID!, $effect: AbilityEffectInput!) {
        addAbilityEffect(abilityId: $abilityId, effect: $effect) {
          ${ABILITY_EFFECT_FIELDS}
        }
      }
    `,
    { abilityId, effect },
  ).then((data) => data.addAbilityEffect);
}

export function updateAbilityEffect(
  effectId: string,
  effect: AbilityEffectInput,
): Promise<AbilityEffectRecord> {
  return postGraphQL<{ updateAbilityEffect: AbilityEffectRecord }>(
    `
      mutation UpdateAbilityEffect($effectId: UUID!, $effect: AbilityEffectInput!) {
        updateAbilityEffect(effectId: $effectId, effect: $effect) {
          ${ABILITY_EFFECT_FIELDS}
        }
      }
    `,
    { effectId, effect },
  ).then((data) => data.updateAbilityEffect);
}

export function removeAbilityEffect(effectId: string): Promise<boolean> {
  return postGraphQL<{ removeAbilityEffect: boolean }>(
    `
      mutation RemoveAbilityEffect($effectId: UUID!) {
        removeAbilityEffect(effectId: $effectId)
      }
    `,
    { effectId },
  ).then((data) => data.removeAbilityEffect);
}

/** FR-026: DM-only, enforced server-side. */
export function getAbilityPermissions(
  abilityId: string,
): Promise<AbilityPermissionRecord[]> {
  return postGraphQL<{ abilityPermissions: AbilityPermissionRecord[] }>(
    `
      query AbilityPermissions($abilityId: UUID!) {
        abilityPermissions(abilityId: $abilityId) {
          abilityId
          userId
          level
          updatedAt
        }
      }
    `,
    { abilityId },
  ).then((data) => data.abilityPermissions);
}

export function setAbilityPermission(
  abilityId: string,
  userId: string,
  level: AbilityPermissionRecord["level"],
): Promise<AbilityPermissionRecord> {
  return postGraphQL<{ setAbilityPermission: AbilityPermissionRecord }>(
    `
      mutation SetAbilityPermission($input: SetAbilityPermissionInput!) {
        setAbilityPermission(input: $input) {
          abilityId
          userId
          level
          updatedAt
        }
      }
    `,
    { input: { abilityId, userId, level } },
  ).then((data) => data.setAbilityPermission);
}

/** Idempotent — removing a nonexistent grant resolves false, and removing an
 * existing one reverts that member to the implicit Viewer default (FR-024). */
export function removeAbilityPermission(
  abilityId: string,
  userId: string,
): Promise<boolean> {
  return postGraphQL<{ removeAbilityPermission: boolean }>(
    `
      mutation RemoveAbilityPermission($abilityId: UUID!, $userId: UUID!) {
        removeAbilityPermission(abilityId: $abilityId, userId: $userId)
      }
    `,
    { abilityId, userId },
  ).then((data) => data.removeAbilityPermission);
}

export type { AbilityPermissionRecord };
