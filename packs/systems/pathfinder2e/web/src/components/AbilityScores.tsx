import React from 'react';

export interface Pathfinder2eAbilities {
  strength: number;
  dexterity: number;
  constitution: number;
  intelligence: number;
  wisdom: number;
  charisma: number;
}

interface AbilityScoresProps {
  abilities: Pathfinder2eAbilities;
  isEditable?: boolean;
  onAbilityChange?: (ability: keyof Pathfinder2eAbilities, value: number) => void;
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
 * Ability Scores Display Component (Pathfinder 2e / Remaster)
 *
 * Unlike dnd5e, PF2e's ability values are already modifiers rather than a
 * raw 1-20 score (research/system_pathfinder2e.json `core_stats[].scale`:
 * "modifier-based, typically -5 to +10ish") — there is no score-to-modifier
 * conversion step here, the stored value *is* the modifier.
 */
const AbilityScores: React.FC<AbilityScoresProps> = ({ abilities, isEditable = true, onAbilityChange }) => {
  return (
    <div className="w-full">
      <h2 className="text-2xl font-bold mb-6">Ability Modifiers</h2>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {ABILITIES.map(({ key, label, shorthand }) => {
          const modifier = abilities[key];

          return (
            <div key={key} className="p-4 border rounded-lg bg-gray-50 hover:bg-gray-100 transition">
              <div className="flex justify-between items-center mb-2">
                <h3 className="font-bold text-lg">{shorthand}</h3>
                {isEditable && (
                  <input
                    type="number"
                    min={-5}
                    max={10}
                    value={modifier}
                    onChange={(e) => {
                      const newValue = parseInt(e.target.value, 10);
                      if (!isNaN(newValue) && onAbilityChange) {
                        onAbilityChange(key, newValue);
                      }
                    }}
                    className="w-14 px-2 py-1 border rounded text-center font-semibold"
                  />
                )}
                {!isEditable && (
                  <span className="text-lg font-semibold">
                    {modifier >= 0 ? '+' : ''}
                    {modifier}
                  </span>
                )}
              </div>

              <p className="text-sm text-gray-600 mb-3">{label}</p>

              <div className="bg-white p-2 rounded border-2 border-blue-300 text-center">
                <div className="text-2xl font-bold text-blue-600">
                  {modifier >= 0 ? '+' : ''}
                  {modifier}
                </div>
                <div className="text-xs text-gray-600">Modifier</div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default AbilityScores;
