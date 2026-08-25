import type { ActorPermissionLevel, LoreLinkSourceRecord } from "@/types/actor";

export type { ActorPermissionLevel };

/**
 * Spec 025 (T012): the fixed, system-agnostic classification set (FR-009).
 * Shared by every game system so ability data survives a system change
 * intact — systems re-label these for display via `abilityFacets`
 * (`@/utils/abilityFacets`, FR-010), but cannot add to the set.
 *
 * GraphQL enum casing (`SPELL`), not the DB casing (`spell`); use
 * `toAbilityClassificationKey` to cross between them.
 */
export type AbilityClassification = "SPELL" | "FEAT" | "POWER" | "TALENT";

export type AbilityEffectType = "HEAL" | "DAMAGE" | "MODIFIER" | "ATTACK_ROLL";
export type AbilityEffectTrigger = "ON_USE" | "PASSIVE";

export type AbilityEffectRecord = {
  id: string;
  abilityId: string;
  effectType: AbilityEffectType;
  formula: string;
  target: string;
  triggerKind: AbilityEffectTrigger | null;
  sortOrder: number;
};

export type WorldAbilityRecord = {
  id: string;
  worldId: string;
  name: string;
  description: string | null;
  classification: AbilityClassification;
  /**
   * FR-024a: visibility, deliberately independent of `myPermissionLevel`.
   *
   * Only ever `true` in a response to a DM — every non-DM read path filters
   * GM-only abilities out server-side (FR-024b), so a player's client never
   * receives a row with this set. Do not treat it as a client-side gate; it is
   * for showing the DM that an ability *is* hidden (FR-024d).
   */
  gmOnly: boolean;
  effects: AbilityEffectRecord[];
  /** Edit rights only — NOT visibility. See `gmOnly`. */
  myPermissionLevel: ActorPermissionLevel;
  moderated: boolean;
  moderationCaseId: string | null;
  createdAt: string;
  updatedAt: string;
  /**
   * Optional until US4 (T063) adds `linkedFromLore` to `GraphQLAbility`.
   * The field does not exist in the schema yet, so querying it would be a
   * hard GraphQL error — the API layer deliberately omits it from its
   * selection set for now rather than stubbing an empty array server-side.
   */
  linkedFromLore?: LoreLinkSourceRecord[];
};

export type AbilityPermissionRecord = {
  abilityId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};
