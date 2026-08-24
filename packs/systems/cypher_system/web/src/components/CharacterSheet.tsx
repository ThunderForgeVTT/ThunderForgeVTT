import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

/**
 * A single Cypher System stat Pool (Might/Speed/Intellect), per
 * research/system_cypher_system.json's `core_stats`/`resources` (a Pool
 * is the raw point total; Edge reduces the Pool-point cost of spending
 * from that stat, including Effort — see `glossary`).
 */
export interface CypherPool {
  current: number;
  max: number;
  edge: number;
}

export interface CypherCharacter {
  id: string;
  name: string;
  descriptor: string;
  type: string;
  focus: string;
  tier: number;
  effort: number;
  xp: number;
  pools: {
    might: CypherPool;
    speed: CypherPool;
    intellect: CypherPool;
  };
  trainedSkills: string[];
}

interface CharacterSheetProps {
  character?: CypherCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<CypherCharacter>) => void;
}

const POOLS = [
  { key: 'might' as const, label: 'Might', abbreviation: 'Mgt' },
  { key: 'speed' as const, label: 'Speed', abbreviation: 'Spd' },
  { key: 'intellect' as const, label: 'Intellect', abbreviation: 'Int' },
];

/**
 * Cypher System Character Sheet Component
 *
 * Mirrors packs/systems/dnd5e/web/src/components/CharacterSheet.tsx's
 * conventions (Radix tabs, props-driven, no data fetching of its own).
 * Shows the three stat Pools (Might/Speed/Intellect) with current/max
 * values and per-stat Edge (research/system_cypher_system.json's
 * `resources.edge`: reduces the Pool-point cost of spending from that
 * stat, including Effort).
 */
const CharacterSheet: React.FC<CharacterSheetProps> = ({
  character,
  isEditable = true,
  onUpdate,
}) => {
  const [selectedTab, setSelectedTab] = useState<string>('pools');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const handlePoolChange = (
    poolKey: keyof CypherCharacter['pools'],
    field: keyof CypherPool,
    value: number,
  ) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      pools: {
        ...character.pools,
        [poolKey]: {
          ...character.pools[poolKey],
          [field]: value,
        },
      },
    });
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      {/* Header */}
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
        <p className="text-lg text-gray-600">
          Tier {character.tier} {character.descriptor} {character.type} who {character.focus}
        </p>
        <div className="mt-2 flex gap-4 text-sm">
          <div>
            <span className="font-semibold">Effort:</span> {character.effort}
          </div>
          <div>
            <span className="font-semibold">XP:</span> {character.xp}
          </div>
        </div>
      </div>

      {/* Tabbed Interface */}
      <Tabs.Root value={selectedTab} onValueChange={setSelectedTab} className="w-full">
        <Tabs.List className="flex gap-2 border-b mb-4">
          <Tabs.Trigger
            value="pools"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Pools
          </Tabs.Trigger>
          <Tabs.Trigger
            value="skills"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Skills
          </Tabs.Trigger>
        </Tabs.List>

        {/* Pools Tab */}
        <Tabs.Content value="pools" className="p-4">
          <h2 className="text-2xl font-bold mb-6">Stat Pools</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {POOLS.map(({ key, label, abbreviation }) => {
              const pool = character.pools[key];
              return (
                <div key={key} className="p-4 border rounded-lg bg-gray-50 hover:bg-gray-100 transition">
                  <div className="flex justify-between items-center mb-2">
                    <h3 className="font-bold text-lg">{abbreviation}</h3>
                    <span className="text-sm text-gray-600">{label}</span>
                  </div>

                  {/* Current / Max Pool */}
                  <div className="bg-white p-2 rounded border-2 border-blue-300 text-center mb-2 flex items-center justify-center gap-2">
                    {isEditable ? (
                      <input
                        type="number"
                        min={0}
                        value={pool.current}
                        onChange={(e) => {
                          const newValue = parseInt(e.target.value, 10);
                          if (!isNaN(newValue)) handlePoolChange(key, 'current', newValue);
                        }}
                        className="w-14 px-1 py-1 border rounded text-center font-bold text-xl text-blue-600"
                      />
                    ) : (
                      <span className="text-2xl font-bold text-blue-600">{pool.current}</span>
                    )}
                    <span className="text-xl text-gray-400">/</span>
                    {isEditable ? (
                      <input
                        type="number"
                        min={0}
                        value={pool.max}
                        onChange={(e) => {
                          const newValue = parseInt(e.target.value, 10);
                          if (!isNaN(newValue)) handlePoolChange(key, 'max', newValue);
                        }}
                        className="w-14 px-1 py-1 border rounded text-center font-bold text-xl"
                      />
                    ) : (
                      <span className="text-2xl font-bold">{pool.max}</span>
                    )}
                  </div>
                  <div className="text-xs text-gray-600 text-center mb-2">Pool (current / max)</div>

                  {/* Edge */}
                  <div className="bg-white p-2 rounded border border-gray-300 flex items-center justify-between">
                    <span className="text-sm font-semibold">Edge</span>
                    {isEditable ? (
                      <input
                        type="number"
                        min={0}
                        value={pool.edge}
                        onChange={(e) => {
                          const newValue = parseInt(e.target.value, 10);
                          if (!isNaN(newValue)) handlePoolChange(key, 'edge', newValue);
                        }}
                        className="w-12 px-1 py-1 border rounded text-center font-semibold"
                      />
                    ) : (
                      <span className="text-sm font-bold text-green-600">{pool.edge}</span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </Tabs.Content>

        {/* Skills Tab */}
        <Tabs.Content value="skills" className="p-4">
          <h2 className="text-2xl font-bold mb-4">Trained Skills</h2>
          <p className="text-sm text-gray-600 mb-4">
            The Cypher System has no fixed skill list — skills are freeform, described only by a
            training tier (trained/specialized/practiced/inability) relative to a task.
          </p>
          {character.trainedSkills.length === 0 ? (
            <p className="text-gray-500">No trained skills recorded.</p>
          ) : (
            <ul className="list-disc list-inside space-y-1">
              {character.trainedSkills.map((skill) => (
                <li key={skill} className="text-sm">
                  {skill}
                </li>
              ))}
            </ul>
          )}
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
