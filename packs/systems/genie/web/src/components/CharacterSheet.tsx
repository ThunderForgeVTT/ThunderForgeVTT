import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import ConditionTrack from './ConditionTrack';

/**
 * Spec 018 (Genie) `ability_data` shape — see `packs/systems/genie/system.json`'s
 * `data_types.ability_data`.
 */
export interface GenieAbilityData {
  might: number;
  cunning: number;
  spirit: number;
}

/**
 * Spec 018 `proficiency_data` shape — a flat list of trained skill keys
 * (data-model.md: "Skill training flags, one boolean per Genie skill",
 * represented here as membership in this array rather than a map of
 * booleans, mirroring the manifest stub's `trained_skills` array).
 */
export interface GenieProficiencyData {
  trained_skills: string[];
}

/** One entry from the manifest's (currently empty, per-world-content) `skills` block. */
export interface GenieSkillDefinition {
  key: string;
  label: string;
  ability: keyof GenieAbilityData;
}

export interface GenieCharacter {
  id: string;
  name: string;
  abilityData: GenieAbilityData;
  proficiencyData: GenieProficiencyData;
  /** `trait_data.active_conditions` (spec 018 User Story 4) — optional
   * since older callers/fixtures may not supply it yet. */
  activeConditions?: string[];
}

interface CharacterSheetProps {
  character?: GenieCharacter;
  /** The world's declared skills (`system.json`'s `skills` block) — a
   * skill's Manifestation-roll rating is its linked ability's score,
   * plus a flat trained bonus when the character knows it (FR-003:
   * "skills linked to abilities"). */
  skills?: GenieSkillDefinition[];
  /** Flat bonus added to a skill's rating when the character is trained
   * in it. Kept as a prop rather than a hardcoded constant since Genie's
   * manifest doesn't yet fix this value. */
  trainedBonus?: number;
  isEditable?: boolean;
  onAbilityChange?: (ability: keyof GenieAbilityData, value: number) => void;
  onSkillTrainedToggle?: (skillKey: string) => void;
}

const ABILITY_LABELS: Record<keyof GenieAbilityData, string> = {
  might: 'Might',
  cunning: 'Cunning',
  spirit: 'Spirit',
};

/**
 * Genie Character Sheet — spec 018 User Story 1 (T017).
 *
 * Displays ability scores and skill ratings from a character's
 * `ability_data` / `proficiency_data`, the two `data_types` blocks
 * declared in `packs/systems/genie/system.json`. Follows the same
 * props-driven, Radix-tabs shape as `packs/systems/dnd5e/web/src/components/CharacterSheet.tsx`,
 * scoped down to what Genie's manifest actually declares today (no
 * class/level/spellbook concepts — Genie is class-less per spec.md's
 * Assumptions).
 */
export const CharacterSheet: React.FC<CharacterSheetProps> = ({
  character,
  skills = [],
  trainedBonus = 2,
  isEditable = false,
  onAbilityChange,
  onSkillTrainedToggle,
}) => {
  const [selectedTab, setSelectedTab] = useState<string>('abilities');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const { abilityData, proficiencyData } = character;
  const trainedSkills = new Set(proficiencyData.trained_skills ?? []);

  const skillRating = (skill: GenieSkillDefinition): number => {
    const base = abilityData[skill.ability] ?? 0;
    return trainedSkills.has(skill.key) ? base + trainedBonus : base;
  };

  const handleAbilityChange = (ability: keyof GenieAbilityData, value: number) => {
    if (!isEditable || !onAbilityChange) return;
    onAbilityChange(ability, value);
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
      </div>

      <Tabs.Root value={selectedTab} onValueChange={setSelectedTab} className="w-full">
        <Tabs.List className="flex gap-2 border-b mb-4">
          <Tabs.Trigger
            value="abilities"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Abilities
          </Tabs.Trigger>
          <Tabs.Trigger
            value="skills"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Skills
          </Tabs.Trigger>
          <Tabs.Trigger
            value="conditions"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Conditions
          </Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="abilities" className="p-4">
          <div className="grid grid-cols-3 gap-4">
            {(Object.keys(ABILITY_LABELS) as (keyof GenieAbilityData)[]).map((ability) => (
              <div key={ability} className="flex flex-col items-center gap-1 border rounded p-3">
                <span className="text-sm font-semibold text-gray-600">{ABILITY_LABELS[ability]}</span>
                {isEditable ? (
                  <input
                    type="number"
                    className="w-16 text-center border rounded"
                    value={abilityData[ability]}
                    onChange={(e) => handleAbilityChange(ability, Number(e.target.value))}
                  />
                ) : (
                  <span className="text-2xl font-bold">{abilityData[ability]}</span>
                )}
              </div>
            ))}
          </div>
        </Tabs.Content>

        <Tabs.Content value="skills" className="p-4">
          {skills.length === 0 ? (
            <p className="text-sm text-gray-500">No skills have been defined for this world yet.</p>
          ) : (
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b">
                  <th className="py-1">Skill</th>
                  <th className="py-1">Ability</th>
                  <th className="py-1">Trained</th>
                  <th className="py-1">Rating (Manifestation pool)</th>
                </tr>
              </thead>
              <tbody>
                {skills.map((skill) => (
                  <tr key={skill.key} className="border-b last:border-0">
                    <td className="py-1">{skill.label}</td>
                    <td className="py-1">{ABILITY_LABELS[skill.ability]}</td>
                    <td className="py-1">
                      <input
                        type="checkbox"
                        checked={trainedSkills.has(skill.key)}
                        disabled={!isEditable || !onSkillTrainedToggle}
                        onChange={() => onSkillTrainedToggle?.(skill.key)}
                      />
                    </td>
                    <td className="py-1 font-semibold">{skillRating(skill)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Tabs.Content>

        <Tabs.Content value="conditions" className="p-4">
          <ConditionTrack activeConditions={character.activeConditions} variant="sheet" />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
