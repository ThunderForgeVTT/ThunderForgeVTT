//! D&D 5e Derived Data Calculators
//!
//! Client-side calculation functions that derive stats from base data.
//! These are identical implementations to the Rust engine, ensuring consistency.

/**
 * Calculate ability modifier from ability score
 *
 * Formula: (score - 10) / 2, floored
 * Examples: 10 → 0, 11 → 0, 12 → 1, 8 → -1, 20 → 5
 */
export function calculateAbilityModifier(score: number): number {
  return Math.floor((score - 10) / 2);
}

/**
 * Calculate proficiency bonus based on character level
 *
 * Formula (D&D 5e):
 * - Levels 1-4: +2
 * - Levels 5-8: +3
 * - Levels 9-12: +4
 * - Levels 13-16: +5
 * - Levels 17-20: +6
 */
export function calculateProficiencyBonus(level: number): number {
  if (level <= 4) return 2;
  if (level <= 8) return 3;
  if (level <= 12) return 4;
  if (level <= 16) return 5;
  return 6;
}

/**
 * Calculate skill bonus
 *
 * Formula: abilityModifier + (isProficient ? proficiencyBonus : 0)
 */
export function calculateSkillBonus(
  abilityModifier: number,
  isProficient: boolean,
  proficiencyBonus: number
): number {
  return abilityModifier + (isProficient ? proficiencyBonus : 0);
}

/**
 * Calculate saving throw modifier
 *
 * Formula: abilityModifier + (hasProficiency ? proficiencyBonus : 0)
 */
export function calculateSavingThrow(
  abilityModifier: number,
  hasProficiency: boolean,
  proficiencyBonus: number
): number {
  return abilityModifier + (hasProficiency ? proficiencyBonus : 0);
}

/**
 * Maximum spell slots by character level and spell level
 *
 * Returns the maximum number of spell slots for a given spell level and character level.
 * Spell level 0 represents cantrips (always unlimited: return -1).
 * Spell levels 1-8 follow the standard full caster progression.
 *
 * @param characterLevel - Character level (1-20)
 * @param spellLevel - Spell level (0 = cantrips, 1-8 = spell levels)
 * @returns Maximum spell slots available
 */
export function calculateMaxSpellSlots(characterLevel: number, spellLevel: number): number {
  // Cantrips are unlimited
  if (spellLevel === 0) {
    return -1; // Sentinel value for "unlimited"
  }

  // Table of spell slots by character level
  // Index = level - 1, value array = [slots for spell level 0-8] where 0 is cantrips
  const spellSlotsByLevel: number[][] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0], // Level 1
    [3, 2, 0, 0, 0, 0, 0, 0, 0], // Level 2
    [4, 3, 2, 0, 0, 0, 0, 0, 0], // Level 3
    [4, 3, 3, 2, 0, 0, 0, 0, 0], // Level 4
    [4, 4, 3, 3, 2, 0, 0, 0, 0], // Level 5
    [4, 4, 3, 3, 3, 2, 0, 0, 0], // Level 6
    [4, 4, 4, 3, 3, 3, 2, 0, 0], // Level 7
    [4, 4, 4, 3, 3, 3, 3, 2, 0], // Level 8
    [4, 4, 4, 4, 3, 3, 3, 3, 3], // Level 9
    [5, 4, 4, 4, 3, 3, 3, 3, 3], // Level 10
    [5, 4, 4, 4, 4, 3, 3, 3, 3], // Level 11
    [5, 4, 4, 4, 4, 3, 3, 3, 3], // Level 12
    [5, 4, 4, 4, 4, 4, 3, 3, 3], // Level 13
    [5, 4, 4, 4, 4, 4, 3, 3, 3], // Level 14
    [5, 4, 4, 4, 4, 4, 4, 3, 3], // Level 15
    [5, 4, 4, 4, 4, 4, 4, 3, 3], // Level 16
    [5, 5, 4, 4, 4, 4, 4, 4, 3], // Level 17
    [5, 5, 4, 4, 4, 4, 4, 4, 3], // Level 18
    [5, 5, 4, 4, 4, 4, 4, 4, 4], // Level 19
    [5, 5, 4, 4, 4, 4, 4, 4, 4], // Level 20
  ];

  if (characterLevel < 1 || characterLevel > 20) {
    return 0;
  }

  const levelSlots = spellSlotsByLevel[characterLevel - 1];
  if (spellLevel < 1 || spellLevel > 8) {
    return 0;
  }

  return levelSlots[spellLevel] || 0;
}

/**
 * Calculate attack roll bonus
 *
 * Formula: abilityModifier + proficiencyBonus (if proficient with weapon)
 */
export function calculateAttackBonus(
  abilityModifier: number,
  isProficient: boolean,
  proficiencyBonus: number
): number {
  return abilityModifier + (isProficient ? proficiencyBonus : 0);
}

/**
 * Get ability modifier for a specific ability score
 *
 * @param abilities - Object with ability scores
 * @param abilityName - Name of ability (strength, dexterity, etc.)
 * @returns Ability modifier
 */
export function getAbilityModifier(
  abilities: Record<string, number>,
  abilityName: string
): number {
  const score = abilities[abilityName.toLowerCase()];
  if (!score) return 0;
  return calculateAbilityModifier(score);
}

/**
 * Get all ability modifiers
 *
 * @param abilities - Object with ability scores
 * @returns Object with ability modifiers
 */
export function getAllAbilityModifiers(
  abilities: Record<string, number>
): Record<string, number> {
  return {
    strength: calculateAbilityModifier(abilities.strength || 10),
    dexterity: calculateAbilityModifier(abilities.dexterity || 10),
    constitution: calculateAbilityModifier(abilities.constitution || 10),
    intelligence: calculateAbilityModifier(abilities.intelligence || 10),
    wisdom: calculateAbilityModifier(abilities.wisdom || 10),
    charisma: calculateAbilityModifier(abilities.charisma || 10),
  };
}

/**
 * Calculate hit point increase per level
 *
 * Formula: (class_hit_die / 2 + 1) + constitution_modifier
 * Minimum increase is 1 HP per level
 */
export function calculateHitPointIncrease(classDiceSize: number, conModifier: number): number {
  const baseIncrease = Math.floor(classDiceSize / 2) + 1;
  return Math.max(1, baseIncrease + conModifier);
}

/**
 * Calculate max hit points for character
 *
 * Formula:
 * - Level 1: classDiceSize + conModifier (minimum 1)
 * - Level 2+: sum of (hitPointIncreasePerLevel) for each level
 */
export function calculateMaxHitPoints(
  level: number,
  classDiceSize: number,
  conModifier: number
): number {
  // Level 1 HP: full hit die + CON modifier
  let hp = classDiceSize + conModifier;
  hp = Math.max(1, hp); // Minimum 1 HP at level 1

  // Additional levels
  const hpPerLevel = calculateHitPointIncrease(classDiceSize, conModifier);
  for (let i = 2; i <= level; i++) {
    hp += hpPerLevel;
  }

  return hp;
}

/**
 * Calculate armor class from base AC and dexterity modifier
 *
 * Formula: baseAC + dexterityModifier
 * (Assumes light or no armor which adds DEX to AC)
 */
export function calculateArmorClass(baseAC: number, dexterityModifier: number): number {
  return baseAC + dexterityModifier;
}

/**
 * Calculate initiative bonus
 *
 * Formula: dexterityModifier
 */
export function calculateInitiative(dexterityModifier: number): number {
  return dexterityModifier;
}
