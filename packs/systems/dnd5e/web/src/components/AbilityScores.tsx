import React from 'react';

interface AbilityScoresProps {
  abilities: {
    strength: number;
    dexterity: number;
    constitution: number;
    intelligence: number;
    wisdom: number;
    charisma: number;
  };
  profBonus: number;
  savingThrowProficiencies: Record<string, boolean>;
  isEditable?: boolean;
  onAbilityChange?: (ability: keyof AbilityScoresProps['abilities'], value: number) => void;
}

const ABILITIES = [
  { key: 'strength' as const, label: 'Strength', shorthand: 'STR' },
  { key: 'dexterity' as const, label: 'Dexterity', shorthand: 'DEX' },
  { key: 'constitution' as const, label: 'Constitution', shorthand: 'CON' },
  { key: 'intelligence' as const, label: 'Intelligence', shorthand: 'INT' },
  { key: 'wisdom' as const, label: 'Wisdom', shorthand: 'WIS' },
  { key: 'charisma' as const, label: 'Charisma', shorthand: 'CHA' },
];

/**
 * Ability Scores Display Component
 *
 * Shows the 6 core D&D 5e ability scores (STR, DEX, CON, INT, WIS, CHA) with:
 * - Raw ability score (1-20)
 * - Calculated modifier (score - 10) / 2
 * - Saving throw proficiency toggle (if proficient: +profBonus to modifier)
 */
const AbilityScores: React.FC<AbilityScoresProps> = ({
  abilities,
  profBonus,
  savingThrowProficiencies,
  isEditable = true,
  onAbilityChange,
}) => {
  const calculateModifier = (score: number): number => {
    return Math.floor((score - 10) / 2);
  };

  const calculateSavingThrow = (abilityKey: string): number => {
    const score = abilities[abilityKey as keyof typeof abilities];
    const mod = calculateModifier(score);
    const isProficient = savingThrowProficiencies[abilityKey] || false;
    return mod + (isProficient ? profBonus : 0);
  };

  return (
    <div className="w-full">
      <h2 className="text-2xl font-bold mb-6">Ability Scores</h2>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {ABILITIES.map(({ key, label, shorthand }) => {
          const score = abilities[key];
          const modifier = calculateModifier(score);
          const savingThrow = calculateSavingThrow(key);
          const isProficient = savingThrowProficiencies[key] || false;

          return (
            <div
              key={key}
              className="p-4 border rounded-lg bg-gray-50 hover:bg-gray-100 transition"
            >
              {/* Header */}
              <div className="flex justify-between items-center mb-2">
                <h3 className="font-bold text-lg">{shorthand}</h3>
                {isEditable && (
                  <input
                    type="number"
                    min="1"
                    max="20"
                    value={score}
                    onChange={(e) => {
                      const newValue = parseInt(e.target.value, 10);
                      if (!isNaN(newValue) && onAbilityChange) {
                        onAbilityChange(key, newValue);
                      }
                    }}
                    className="w-12 px-2 py-1 border rounded text-center font-semibold"
                  />
                )}
                {!isEditable && <span className="text-lg font-semibold">{score}</span>}
              </div>

              {/* Score Description */}
              <p className="text-sm text-gray-600 mb-3">{label}</p>

              {/* Modifier */}
              <div className="bg-white p-2 rounded border-2 border-blue-300 text-center mb-2">
                <div className="text-2xl font-bold text-blue-600">
                  {modifier >= 0 ? '+' : ''}{modifier}
                </div>
                <div className="text-xs text-gray-600">Modifier</div>
              </div>

              {/* Saving Throw */}
              <div className="bg-white p-2 rounded border border-gray-300 flex items-center justify-between">
                <label className="flex items-center gap-2 cursor-pointer flex-1">
                  <input
                    type="checkbox"
                    checked={isProficient}
                    onChange={(e) => {
                      // Placeholder for save proficiency toggle
                      // In real implementation, would call an onSaveToggle callback
                    }}
                    disabled={!isEditable}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-semibold">Save</span>
                </label>
                <span className={`text-sm font-bold ${isProficient ? 'text-green-600' : 'text-gray-600'}`}>
                  {savingThrow >= 0 ? '+' : ''}{savingThrow}
                </span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Proficiency Bonus Display */}
      <div className="mt-8 p-4 bg-blue-50 rounded-lg border border-blue-200">
        <div className="text-center">
          <p className="text-sm text-gray-600 mb-1">Proficiency Bonus</p>
          <p className="text-3xl font-bold text-blue-600">+{profBonus}</p>
        </div>
      </div>
    </div>
  );
};

export default AbilityScores;
