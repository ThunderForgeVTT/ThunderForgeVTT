/**
 * SkillsList.tsx
 * D&D 5e Character Skills Component
 *
 * Phase 4.8.1: System-Aware React Components (Phase E.1)
 *
 * Displays all 18 D&D 5e skills with proficiency tracking:
 * - Skill Name (Acrobatics, Animal Handling, Arcana, etc.)
 * - Associated Ability (DEX, WIS, INT, etc.)
 * - Proficiency checkbox
 * - Modifier calculation: ability_modifier + (2 or 0) if proficient
 * - Passive check value: 10 + modifier
 *
 * Skills are grouped by ability for better organization.
 */

import type { ReactNode } from "react";
import { Card } from "@/components/ui/card/Card";
import { cn } from "@/utils/cn";
import styles from "./SkillsList.module.scss";

export interface SkillsListProps {
  abilityData?: Record<string, number>;
  proficiencyData?: Record<string, boolean>;
  editable?: boolean;
  onToggleProficiency?: (skillId: string, proficient: boolean) => void;
}

interface Skill {
  id: string;
  name: string;
  ability: "strength" | "dexterity" | "constitution" | "intelligence" | "wisdom" | "charisma";
  abilityShort: string;
}

const SKILLS: Skill[] = [
  // Strength
  { id: "athletics", name: "Athletics", ability: "strength", abilityShort: "STR" },
  // Dexterity
  { id: "acrobatics", name: "Acrobatics", ability: "dexterity", abilityShort: "DEX" },
  { id: "sleight_of_hand", name: "Sleight of Hand", ability: "dexterity", abilityShort: "DEX" },
  { id: "stealth", name: "Stealth", ability: "dexterity", abilityShort: "DEX" },
  // Intelligence
  { id: "arcana", name: "Arcana", ability: "intelligence", abilityShort: "INT" },
  { id: "history", name: "History", ability: "intelligence", abilityShort: "INT" },
  { id: "investigation", name: "Investigation", ability: "intelligence", abilityShort: "INT" },
  { id: "nature", name: "Nature", ability: "intelligence", abilityShort: "INT" },
  { id: "religion", name: "Religion", ability: "intelligence", abilityShort: "INT" },
  // Wisdom
  { id: "animal_handling", name: "Animal Handling", ability: "wisdom", abilityShort: "WIS" },
  { id: "insight", name: "Insight", ability: "wisdom", abilityShort: "WIS" },
  { id: "medicine", name: "Medicine", ability: "wisdom", abilityShort: "WIS" },
  { id: "perception", name: "Perception", ability: "wisdom", abilityShort: "WIS" },
  { id: "survival", name: "Survival", ability: "wisdom", abilityShort: "WIS" },
  // Charisma
  { id: "deception", name: "Deception", ability: "charisma", abilityShort: "CHA" },
  { id: "intimidation", name: "Intimidation", ability: "charisma", abilityShort: "CHA" },
  { id: "performance", name: "Performance", ability: "charisma", abilityShort: "CHA" },
  { id: "persuasion", name: "Persuasion", ability: "charisma", abilityShort: "CHA" },
];

function calculateAbilityModifier(score?: number): number {
  return Math.floor(((score ?? 10) - 10) / 2);
}

function calculateSkillModifier(
  abilityModifier: number,
  isProficient: boolean,
  proficiencyBonus: number = 2
): number {
  return abilityModifier + (isProficient ? proficiencyBonus : 0);
}

function SkillRow({
  skill,
  abilityData = {},
  proficiencyData = {},
  editable = false,
  onToggleProficiency,
}: {
  skill: Skill;
  abilityData?: Record<string, number>;
  proficiencyData?: Record<string, boolean>;
  editable?: boolean;
  onToggleProficiency?: (skillId: string, proficient: boolean) => void;
}): ReactNode {
  const abilityScore = abilityData[skill.ability] ?? 10;
  const abilityModifier = calculateAbilityModifier(abilityScore);
  const isProficient = proficiencyData[skill.id] ?? false;
  const skillModifier = calculateSkillModifier(abilityModifier, isProficient);
  const passiveCheck = 10 + skillModifier;

  const modifierText = skillModifier >= 0 ? `+${skillModifier}` : `${skillModifier}`;
  const proficiencyClass = isProficient ? styles.proficient : "";

  return (
    <div className={cn(styles.skillRow, proficiencyClass)}>
      <div className={styles.skillName}>
        <span className={styles.name}>{skill.name}</span>
        <span className={styles.ability}>{skill.abilityShort}</span>
      </div>

      {editable ? (
        <label className={styles.proficiencyCheckbox}>
          <input
            type="checkbox"
            checked={isProficient}
            onChange={(e) => onToggleProficiency?.(skill.id, e.target.checked)}
            aria-label={`${skill.name} proficiency`}
          />
          <span className={styles.checkmark} />
        </label>
      ) : (
        <div className={styles.proficiencyIndicator}>
          {isProficient && <span className={styles.dot}>●</span>}
        </div>
      )}

      <div className={styles.modifierDisplay}>
        <span className={styles.modifier}>{modifierText}</span>
      </div>

      <div className={styles.passiveDisplay}>
        <span className={styles.passiveLabel}>Passive:</span>
        <span className={styles.passive}>{passiveCheck}</span>
      </div>
    </div>
  );
}

/**
 * Display all 18 D&D 5e skills grouped by ability
 *
 * Usage:
 * ```tsx
 * <SkillsList
 *   abilityData={{ strength: 15, dexterity: 14, ... }}
 *   proficiencyData={{ acrobatics: true, stealth: false, ... }}
 *   editable={true}
 *   onToggleProficiency={(skillId, proficient) => mutateToken(...)}
 * />
 * ```
 */
export function SkillsList({
  abilityData = {},
  proficiencyData = {},
  editable = false,
  onToggleProficiency,
}: SkillsListProps) {
  // Group skills by ability
  const skillsByAbility = SKILLS.reduce(
    (acc, skill) => {
      if (!acc[skill.ability]) {
        acc[skill.ability] = [];
      }
      acc[skill.ability].push(skill);
      return acc;
    },
    {} as Record<string, Skill[]>
  );

  const abilityOrder = [
    "strength",
    "dexterity",
    "constitution",
    "intelligence",
    "wisdom",
    "charisma",
  ];

  return (
    <Card surface="parchment" className={styles.container}>
      <div className={styles.header}>
        <h3>Skills</h3>
        <p className={styles.subtitle}>
          Trained abilities derived from your abilities
        </p>
      </div>

      <div className={styles.skillsContainer}>
        {abilityOrder.map((ability) => {
          const skillsForAbility = skillsByAbility[ability];
          if (!skillsForAbility) return null;

          return (
            <div key={ability} className={styles.abilityGroup}>
              <div className={styles.abilityHeader}>
                <h4>{ability.charAt(0).toUpperCase() + ability.slice(1)}</h4>
              </div>

              <div className={styles.skillsList}>
                {skillsForAbility.map((skill) => (
                  <SkillRow
                    key={skill.id}
                    skill={skill}
                    abilityData={abilityData}
                    proficiencyData={proficiencyData}
                    editable={editable}
                    onToggleProficiency={onToggleProficiency}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {editable && (
        <div className={styles.hint}>
          Click proficiency circle to toggle skill proficiency. Modifiers update automatically.
        </div>
      )}
    </Card>
  );
}
