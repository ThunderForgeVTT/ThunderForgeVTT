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
import { cn } from "@/lib/utils";

export interface SkillsListProps {
  abilityData?: Record<string, number>;
  proficiencyData?: Record<string, boolean>;
  editable?: boolean;
  onToggleProficiency?: (skillId: string, proficient: boolean) => void;
}

interface Skill {
  id: string;
  name: string;
  ability:
    | "strength"
    | "dexterity"
    | "constitution"
    | "intelligence"
    | "wisdom"
    | "charisma";
  abilityShort: string;
}

const SKILLS: Skill[] = [
  // Strength
  {
    id: "athletics",
    name: "Athletics",
    ability: "strength",
    abilityShort: "STR",
  },
  // Dexterity
  {
    id: "acrobatics",
    name: "Acrobatics",
    ability: "dexterity",
    abilityShort: "DEX",
  },
  {
    id: "sleight_of_hand",
    name: "Sleight of Hand",
    ability: "dexterity",
    abilityShort: "DEX",
  },
  { id: "stealth", name: "Stealth", ability: "dexterity", abilityShort: "DEX" },
  // Intelligence
  {
    id: "arcana",
    name: "Arcana",
    ability: "intelligence",
    abilityShort: "INT",
  },
  {
    id: "history",
    name: "History",
    ability: "intelligence",
    abilityShort: "INT",
  },
  {
    id: "investigation",
    name: "Investigation",
    ability: "intelligence",
    abilityShort: "INT",
  },
  {
    id: "nature",
    name: "Nature",
    ability: "intelligence",
    abilityShort: "INT",
  },
  {
    id: "religion",
    name: "Religion",
    ability: "intelligence",
    abilityShort: "INT",
  },
  // Wisdom
  {
    id: "animal_handling",
    name: "Animal Handling",
    ability: "wisdom",
    abilityShort: "WIS",
  },
  { id: "insight", name: "Insight", ability: "wisdom", abilityShort: "WIS" },
  { id: "medicine", name: "Medicine", ability: "wisdom", abilityShort: "WIS" },
  {
    id: "perception",
    name: "Perception",
    ability: "wisdom",
    abilityShort: "WIS",
  },
  { id: "survival", name: "Survival", ability: "wisdom", abilityShort: "WIS" },
  // Charisma
  {
    id: "deception",
    name: "Deception",
    ability: "charisma",
    abilityShort: "CHA",
  },
  {
    id: "intimidation",
    name: "Intimidation",
    ability: "charisma",
    abilityShort: "CHA",
  },
  {
    id: "performance",
    name: "Performance",
    ability: "charisma",
    abilityShort: "CHA",
  },
  {
    id: "persuasion",
    name: "Persuasion",
    ability: "charisma",
    abilityShort: "CHA",
  },
];

function calculateAbilityModifier(score?: number): number {
  return Math.floor(((score ?? 10) - 10) / 2);
}

function calculateSkillModifier(
  abilityModifier: number,
  isProficient: boolean,
  proficiencyBonus: number = 2,
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

  const modifierText =
    skillModifier >= 0 ? `+${skillModifier}` : `${skillModifier}`;

  return (
    <div
      className={cn(
        "grid grid-cols-[1fr_auto_auto_auto] items-center gap-3 rounded-md px-3 py-2 text-sm",
        isProficient && "bg-primary/5",
      )}
    >
      <div className="flex items-center gap-2">
        <span>{skill.name}</span>
        <span className="text-xs text-muted-foreground">
          {skill.abilityShort}
        </span>
      </div>

      {editable ? (
        <label className="inline-flex items-center">
          <input
            type="checkbox"
            checked={isProficient}
            onChange={(e) => onToggleProficiency?.(skill.id, e.target.checked)}
            aria-label={`${skill.name} proficiency`}
            className="size-4 rounded border-input accent-primary"
          />
        </label>
      ) : (
        <div className="w-4 text-center">
          {isProficient && <span className="text-primary">●</span>}
        </div>
      )}

      <div>
        <span className="font-medium">{modifierText}</span>
      </div>

      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        <span>Passive:</span>
        <span className="font-medium text-foreground">{passiveCheck}</span>
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
    {} as Record<string, Skill[]>,
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
    <Card surface="parchment" className="grid gap-4 p-6">
      <div>
        <h3 className="text-lg font-semibold">Skills</h3>
        <p className="text-sm text-muted-foreground">
          Trained abilities derived from your abilities
        </p>
      </div>

      <div className="grid gap-4">
        {abilityOrder.map((ability) => {
          const skillsForAbility = skillsByAbility[ability];
          if (!skillsForAbility) return null;

          return (
            <div key={ability} className="grid gap-1">
              <h4 className="px-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                {ability.charAt(0).toUpperCase() + ability.slice(1)}
              </h4>

              <div className="grid">
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
        <div className="text-xs text-muted-foreground">
          Click proficiency circle to toggle skill proficiency. Modifiers update
          automatically.
        </div>
      )}
    </Card>
  );
}
