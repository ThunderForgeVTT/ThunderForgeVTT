/**
 * Spec 025 (T098): display labels for effect types.
 *
 * Item effects (spec 013) and ability effects (spec 025) deliberately share
 * one type set — `heal | damage | modifier | attack_roll` — so a future
 * resolution engine can consume both through one code path. They should
 * therefore share one label map too; before this it was copied verbatim into
 * four components, and a fifth effect type would have had to be added in four
 * places to avoid a silent fallback to the raw enum name.
 */

export const EFFECT_TYPE_LABELS: Record<string, string> = {
  HEAL: "Heal",
  DAMAGE: "Damage",
  MODIFIER: "Modifier",
  ATTACK_ROLL: "Attack Roll",
};

/** Falls back to the raw value for an unrecognized type rather than rendering
 * blank — an older client reading a newer effect type shows something. */
export function effectTypeLabel(effectType: string): string {
  return EFFECT_TYPE_LABELS[effectType] ?? effectType;
}
