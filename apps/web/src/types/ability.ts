import type { ActorPermissionLevel, LoreLinkSourceRecord } from "@/types/actor";

export type { ActorPermissionLevel };

/**
 * Spec 025 (T012): the fixed, system-agnostic classification set (FR-009).
 * Shared by every game system so ability data survives a system change
 * intact — systems re-label these for display via their vocabulary
 * (`@/abilities/vocabulary`, spec 033 FR-006), but cannot add to the set.
 *
 * GraphQL enum casing (`SPELL`), not the DB casing (`spell`); use
 * `toAbilityClassificationKey` to cross between them.
 */
/**
 * An ability's type, as a stable identity.
 *
 * A closed union until spec 033: `"SPELL" | "FEAT" | "POWER" | "TALENT"`. The
 * available types are now the union of the four built-ins and whatever the
 * world's system declares (FR-011), so a 5e pack may name an Enchantment and
 * no fixed union could hold it. What a person reads comes from
 * `abilityVocabulary(worldId)`; this is only the identity.
 *
 * Sent uppercase, stored lowercase — the server normalises, so both spellings
 * mean the same type.
 */
export type AbilityClassification = string;

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
   * The value on this type's declared grade, where its type declares one.
   *
   * `null` for an ungraded type — FR-022 says such a type shows no grade
   * anywhere, so this is absence rather than zero.
   */
  grade: number | null;
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
  /** FR-029: every lore entry currently linking to this ability. */
  linkedFromLore: LoreLinkSourceRecord[];
};

export type AbilityPermissionRecord = {
  abilityId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};
