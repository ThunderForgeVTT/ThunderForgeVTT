import React, { useState } from 'react';

interface SpellbookProps {
  knownSpells: string[];
  characterLevel: number;
  characterClass: string;
  isEditable?: boolean;
}

const SPELL_SLOTS_BY_LEVEL = [
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

/**
 * Spellbook Component
 *
 * Displays character's known spells and spell slots by level:
 * - Cantrips (spell level 0)
 * - Spell slots for levels 1-8 (based on character level)
 * - Known spells list
 *
 * Phase 4.8 MVP: Displays spell information scaffold
 * Phase 4.8.2+: Will implement spell preparation, casting mechanics
 */
const Spellbook: React.FC<SpellbookProps> = ({
  knownSpells,
  characterLevel,
  characterClass,
  isEditable = true,
}) => {
  const [selectedSpellLevel, setSelectedSpellLevel] = useState<number>(0);

  // Get spell slots for this character level
  const spellSlots = characterLevel <= 20 ? SPELL_SLOTS_BY_LEVEL[characterLevel - 1] : [0];

  // Cantrips (spell level 0)
  const cantrips = knownSpells.filter((spell) => spell.startsWith('[0]'));

  return (
    <div className="w-full">
      <h2 className="text-2xl font-bold mb-6">Spellbook</h2>

      {/* Character Class Spellcasting Info */}
      <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg mb-6">
        <div className="grid grid-cols-2 gap-4">
          <div>
            <p className="text-sm text-gray-600">Class</p>
            <p className="text-lg font-semibold">{characterClass}</p>
          </div>
          <div>
            <p className="text-sm text-gray-600">Character Level</p>
            <p className="text-lg font-semibold">{characterLevel}</p>
          </div>
        </div>
      </div>

      {/* Spell Slots Overview */}
      <div className="mb-8">
        <h3 className="text-lg font-bold mb-4">Spell Slots</h3>
        <div className="grid grid-cols-1 md:grid-cols-9 gap-2">
          {/* Cantrips (unlimited) */}
          <div className="p-3 rounded-lg bg-purple-100 border border-purple-300 text-center">
            <div className="text-2xl font-bold text-purple-700">∞</div>
            <div className="text-xs text-purple-600 font-semibold">Cantrips</div>
          </div>

          {/* Spell slots 1-8 */}
          {spellSlots.slice(1).map((slots, index) => {
            const spellLevel = index + 1;
            const available = slots > 0;

            return (
              <button
                key={spellLevel}
                onClick={() => setSelectedSpellLevel(spellLevel)}
                className={`p-3 rounded-lg border-2 text-center transition cursor-pointer ${
                  selectedSpellLevel === spellLevel
                    ? 'bg-blue-500 border-blue-600 text-white'
                    : available
                      ? 'bg-white border-gray-300 hover:border-blue-400'
                      : 'bg-gray-100 border-gray-200 text-gray-400 cursor-not-allowed'
                }`}
                disabled={!available}
              >
                <div className="text-2xl font-bold">{slots}</div>
                <div className="text-xs font-semibold">Level {spellLevel}</div>
              </button>
            );
          })}
        </div>
      </div>

      {/* Known Spells */}
      <div className="mb-8">
        <h3 className="text-lg font-bold mb-4">Known Spells</h3>

        {/* Cantrips Section */}
        <div className="mb-6">
          <h4 className="font-semibold text-purple-700 mb-3">
            Cantrips ({cantrips.length})
          </h4>
          {cantrips.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
              {cantrips.map((spell, idx) => (
                <div key={idx} className="p-2 bg-purple-50 border border-purple-200 rounded">
                  <p className="font-semibold text-sm">{spell.replace('[0] ', '')}</p>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-gray-500 italic">No cantrips known</p>
          )}
        </div>

        {/* Spells by Level */}
        {[1, 2, 3, 4, 5, 6, 7, 8].map((spellLevel) => {
          const spellsAtLevel = knownSpells.filter(
            (spell) => spell.startsWith(`[${spellLevel}]`)
          );

          if (spellsAtLevel.length === 0) return null;

          return (
            <div key={spellLevel} className="mb-4">
              <h4 className="font-semibold text-blue-700 mb-2">
                Level {spellLevel} Spells ({spellsAtLevel.length})
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
                {spellsAtLevel.map((spell, idx) => (
                  <div key={idx} className="p-2 bg-blue-50 border border-blue-200 rounded">
                    <p className="font-semibold text-sm">{spell.replace(`[${spellLevel}] `, '')}</p>
                  </div>
                ))}
              </div>
            </div>
          );
        })}

        {knownSpells.length === 0 && (
          <p className="text-gray-500 italic">No spells known. Add spells to expand spellbook.</p>
        )}
      </div>

      {/* Empty State */}
      {knownSpells.length === 0 && characterClass !== 'Wizard' && (
        <div className="p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
          <p className="text-sm text-yellow-800">
            <strong>Note:</strong> Not all classes are full spellcasters. Spellbook availability
            depends on class features and level.
          </p>
        </div>
      )}
    </div>
  );
};

export default Spellbook;
