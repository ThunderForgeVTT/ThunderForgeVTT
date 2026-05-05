import React, { useState } from 'react';

interface SkillsListProps {
  abilities: {
    strength: number;
    dexterity: number;
    constitution: number;
    intelligence: number;
    wisdom: number;
    charisma: number;
  };
  skillProficiencies: Record<string, boolean>;
  profBonus: number;
  isEditable?: boolean;
  onSkillToggle?: (skill: string) => void;
}

const SKILLS = [
  { name: 'Acrobatics', ability: 'dexterity' },
  { name: 'Animal Handling', ability: 'wisdom' },
  { name: 'Arcana', ability: 'intelligence' },
  { name: 'Athletics', ability: 'strength' },
  { name: 'Deception', ability: 'charisma' },
  { name: 'History', ability: 'intelligence' },
  { name: 'Insight', ability: 'wisdom' },
  { name: 'Intimidation', ability: 'charisma' },
  { name: 'Investigation', ability: 'intelligence' },
  { name: 'Medicine', ability: 'wisdom' },
  { name: 'Nature', ability: 'intelligence' },
  { name: 'Perception', ability: 'wisdom' },
  { name: 'Performance', ability: 'charisma' },
  { name: 'Persuasion', ability: 'charisma' },
  { name: 'Religion', ability: 'intelligence' },
  { name: 'Sleight of Hand', ability: 'dexterity' },
  { name: 'Stealth', ability: 'dexterity' },
  { name: 'Survival', ability: 'wisdom' },
];

/**
 * Skills List Component
 *
 * Displays all 18 D&D 5e skills, each with:
 * - Associated ability score
 * - Calculated modifier (ability mod + proficiency bonus if proficient)
 * - Proficiency checkbox
 * - Total skill bonus
 */
const SkillsList: React.FC<SkillsListProps> = ({
  abilities,
  skillProficiencies,
  profBonus,
  isEditable = true,
  onSkillToggle,
}) => {
  const [sortBy, setSortBy] = useState<'name' | 'bonus'>('name');

  const calculateAbilityModifier = (ability: string): number => {
    const score = abilities[ability as keyof typeof abilities];
    return Math.floor((score - 10) / 2);
  };

  const calculateSkillBonus = (skillName: string): number => {
    const skill = SKILLS.find((s) => s.name === skillName);
    if (!skill) return 0;

    const abilityMod = calculateAbilityModifier(skill.ability);
    const isProficient = skillProficiencies[skillName] || false;
    return abilityMod + (isProficient ? profBonus : 0);
  };

  const sortedSkills = [...SKILLS].sort((a, b) => {
    if (sortBy === 'bonus') {
      return calculateSkillBonus(b.name) - calculateSkillBonus(a.name);
    }
    return a.name.localeCompare(b.name);
  });

  return (
    <div className="w-full">
      <div className="flex justify-between items-center mb-6">
        <h2 className="text-2xl font-bold">Skills</h2>
        <div className="flex gap-2">
          <button
            onClick={() => setSortBy('name')}
            className={`px-3 py-1 rounded ${
              sortBy === 'name'
                ? 'bg-blue-500 text-white'
                : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
            }`}
          >
            Sort by Name
          </button>
          <button
            onClick={() => setSortBy('bonus')}
            className={`px-3 py-1 rounded ${
              sortBy === 'bonus'
                ? 'bg-blue-500 text-white'
                : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
            }`}
          >
            Sort by Bonus
          </button>
        </div>
      </div>

      <div className="space-y-2">
        {sortedSkills.map((skill) => {
          const isProficient = skillProficiencies[skill.name] || false;
          const abilityMod = calculateAbilityModifier(skill.ability);
          const skillBonus = calculateSkillBonus(skill.name);

          return (
            <div
              key={skill.name}
              className={`flex items-center gap-4 p-3 rounded border transition ${
                isProficient ? 'bg-green-50 border-green-300' : 'bg-white border-gray-200'
              } hover:border-gray-300`}
            >
              {/* Proficiency Checkbox */}
              <input
                type="checkbox"
                checked={isProficient}
                onChange={() => {
                  if (isEditable && onSkillToggle) {
                    onSkillToggle(skill.name);
                  }
                }}
                disabled={!isEditable}
                className="w-5 h-5 cursor-pointer"
              />

              {/* Skill Name */}
              <div className="flex-1">
                <div className="font-semibold text-gray-900">{skill.name}</div>
                <div className="text-xs text-gray-500">
                  {skill.ability.toUpperCase()} ({abilityMod >= 0 ? '+' : ''}{abilityMod})
                </div>
              </div>

              {/* Skill Bonus */}
              <div className="text-right">
                <div className={`text-lg font-bold ${skillBonus >= 0 ? 'text-green-600' : 'text-red-600'}`}>
                  {skillBonus >= 0 ? '+' : ''}{skillBonus}
                </div>
                {isProficient && <div className="text-xs font-semibold text-green-600">Proficient</div>}
              </div>
            </div>
          );
        })}
      </div>

      {/* Summary Stats */}
      <div className="mt-8 p-4 bg-gray-50 rounded-lg border border-gray-200 grid grid-cols-3 gap-4 text-center">
        <div>
          <p className="text-sm text-gray-600">Proficient Skills</p>
          <p className="text-2xl font-bold">
            {Object.values(skillProficiencies).filter(Boolean).length}
          </p>
        </div>
        <div>
          <p className="text-sm text-gray-600">Total Skills</p>
          <p className="text-2xl font-bold">{SKILLS.length}</p>
        </div>
        <div>
          <p className="text-sm text-gray-600">Highest Bonus</p>
          <p className="text-2xl font-bold">
            +{Math.max(...sortedSkills.map((s) => calculateSkillBonus(s.name)))}
          </p>
        </div>
      </div>
    </div>
  );
};

export default SkillsList;
