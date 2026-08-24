import React from 'react';
import type { Pathfinder2eAbilities } from './AbilityScores';

export type ProficiencyRank = 'untrained' | 'trained' | 'expert' | 'master' | 'legendary';

/**
 * research/system_pathfinder2e.json `resolution.modifiers_description`
 * (Player Core, "Checks," pp.400-401): proficiency bonus = 0 for
 * Untrained, or `level + N` for Trained (+2) / Expert (+4) / Master (+6)
 * / Legendary (+8).
 */
const PROFICIENCY_LEVEL_BONUS: Record<ProficiencyRank, number | null> = {
  untrained: null,
  trained: 2,
  expert: 4,
  master: 6,
  legendary: 8,
};

export function proficiencyBonus(rank: ProficiencyRank, characterLevel: number): number {
  const levelAddend = PROFICIENCY_LEVEL_BONUS[rank];
  if (levelAddend === null) return 0;
  return characterLevel + levelAddend;
}

interface SkillsListProps {
  abilities: Pathfinder2eAbilities;
  characterLevel: number;
  skillProficiencies: Record<string, ProficiencyRank>;
  isEditable?: boolean;
  onSkillRankChange?: (skill: string, rank: ProficiencyRank) => void;
}

/**
 * The 18 PF2e (Remaster) skills, per research/system_pathfinder2e.json
 * `skills[]` (acrobatics through thievery, plus perception).
 */
export const SKILLS: Array<{ key: string; label: string; ability: keyof Pathfinder2eAbilities }> = [
  { key: 'acrobatics', label: 'Acrobatics', ability: 'dexterity' },
  { key: 'arcana', label: 'Arcana', ability: 'intelligence' },
  { key: 'athletics', label: 'Athletics', ability: 'strength' },
  { key: 'crafting', label: 'Crafting', ability: 'intelligence' },
  { key: 'deception', label: 'Deception', ability: 'charisma' },
  { key: 'diplomacy', label: 'Diplomacy', ability: 'charisma' },
  { key: 'intimidation', label: 'Intimidation', ability: 'charisma' },
  { key: 'lore', label: 'Lore', ability: 'intelligence' },
  { key: 'medicine', label: 'Medicine', ability: 'wisdom' },
  { key: 'nature', label: 'Nature', ability: 'wisdom' },
  { key: 'occultism', label: 'Occultism', ability: 'intelligence' },
  { key: 'performance', label: 'Performance', ability: 'charisma' },
  { key: 'religion', label: 'Religion', ability: 'wisdom' },
  { key: 'society', label: 'Society', ability: 'intelligence' },
  { key: 'stealth', label: 'Stealth', ability: 'dexterity' },
  { key: 'survival', label: 'Survival', ability: 'wisdom' },
  { key: 'thievery', label: 'Thievery', ability: 'dexterity' },
  { key: 'perception', label: 'Perception', ability: 'wisdom' },
];

const RANKS: ProficiencyRank[] = ['untrained', 'trained', 'expert', 'master', 'legendary'];

/**
 * Skills List Component (Pathfinder 2e / Remaster)
 *
 * Displays all 18 skills with their linked ability modifier, a
 * proficiency-rank selector (Untrained/Trained/Expert/Master/Legendary,
 * per Player Core's proficiency system rather than dnd5e's simple
 * boolean proficiency toggle), and the resulting total skill modifier
 * (ability modifier + proficiency bonus).
 */
const SkillsList: React.FC<SkillsListProps> = ({
  abilities,
  characterLevel,
  skillProficiencies,
  isEditable = true,
  onSkillRankChange,
}) => {
  const rankOf = (skillKey: string): ProficiencyRank => skillProficiencies[skillKey] ?? 'untrained';

  const totalModifier = (skillKey: string, ability: keyof Pathfinder2eAbilities): number => {
    const rank = rankOf(skillKey);
    return abilities[ability] + proficiencyBonus(rank, characterLevel);
  };

  return (
    <div className="w-full">
      <h2 className="text-2xl font-bold mb-6">Skills</h2>

      <div className="space-y-2">
        {SKILLS.map((skill) => {
          const rank = rankOf(skill.key);
          const abilityMod = abilities[skill.ability];
          const modifier = totalModifier(skill.key, skill.ability);

          return (
            <div
              key={skill.key}
              className={`flex items-center gap-4 p-3 rounded border transition ${
                rank !== 'untrained' ? 'bg-green-50 border-green-300' : 'bg-white border-gray-200'
              } hover:border-gray-300`}
            >
              <div className="flex-1">
                <div className="font-semibold text-gray-900">{skill.label}</div>
                <div className="text-xs text-gray-500">
                  {skill.ability.slice(0, 3).toUpperCase()} ({abilityMod >= 0 ? '+' : ''}
                  {abilityMod})
                </div>
              </div>

              <select
                value={rank}
                disabled={!isEditable}
                onChange={(e) => onSkillRankChange?.(skill.key, e.target.value as ProficiencyRank)}
                className="px-2 py-1 border rounded text-sm capitalize"
              >
                {RANKS.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>

              <div className="text-right w-12">
                <div className={`text-lg font-bold ${modifier >= 0 ? 'text-green-600' : 'text-red-600'}`}>
                  {modifier >= 0 ? '+' : ''}
                  {modifier}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default SkillsList;
