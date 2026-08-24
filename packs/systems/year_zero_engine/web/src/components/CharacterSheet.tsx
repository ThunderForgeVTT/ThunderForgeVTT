import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

/**
 * The Year Zero Engine's 4 core attributes (research/system_year_zero_engine.json
 * `core_stats`), standard d6-pool variant range 1-5. Mirrors system.json's `abilities`.
 */
export type YzeAttribute = 'strength' | 'agility' | 'wits' | 'empathy';

/**
 * The 12 core skills, each linked to one attribute (research/system_year_zero_engine.json
 * `skills`). Mirrors system.json's `skills` object 1:1.
 */
export const YZE_SKILLS: Record<string, { ability: YzeAttribute; label: string }> = {
  force: { ability: 'strength', label: 'Force' },
  melee: { ability: 'strength', label: 'Melee' },
  stamina: { ability: 'strength', label: 'Stamina' },
  marksmanship: { ability: 'agility', label: 'Marksmanship' },
  mobility: { ability: 'agility', label: 'Mobility' },
  stealth: { ability: 'agility', label: 'Stealth' },
  crafting: { ability: 'wits', label: 'Crafting' },
  observation: { ability: 'wits', label: 'Observation' },
  survival: { ability: 'wits', label: 'Survival' },
  healing: { ability: 'empathy', label: 'Healing' },
  insight: { ability: 'empathy', label: 'Insight' },
  persuasion: { ability: 'empathy', label: 'Persuasion' },
};

const ATTRIBUTE_LABELS: Record<YzeAttribute, string> = {
  strength: 'Strength',
  agility: 'Agility',
  wits: 'Wits',
  empathy: 'Empathy',
};

export interface YzeCharacter {
  id: string;
  name: string;
  attributes: Record<YzeAttribute, number>;
  /** Trained level (0-5) per skill key from YZE_SKILLS. */
  skills: Record<string, number>;
  resources: {
    health: number;
    resolve: number;
    stress: number;
    experience_points: number;
  };
}

interface CharacterSheetProps {
  character?: YzeCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<YzeCharacter>) => void;
}

/**
 * Year Zero Engine Character Sheet Component
 *
 * Main character display UI with a tabbed interface:
 * - Attributes: the 4 core attributes (Strength/Agility/Wits/Empathy)
 * - Skills: all 12 core skills, grouped by their linked attribute
 * - Resources: Health, Resolve, Stress, and Experience Points
 *
 * Uses RxDB to read/write character data; mirrors
 * packs/systems/dnd5e/web/src/components/CharacterSheet.tsx's conventions
 * (Radix tabs, props-driven, no client-side persistence of its own).
 */
const CharacterSheet: React.FC<CharacterSheetProps> = ({ character, isEditable = true, onUpdate }) => {
  const [selectedTab, setSelectedTab] = useState<string>('attributes');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const handleAttributeChange = (attribute: YzeAttribute, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      attributes: {
        ...character.attributes,
        [attribute]: value,
      },
    });
  };

  const handleSkillChange = (skill: string, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      skills: {
        ...character.skills,
        [skill]: value,
      },
    });
  };

  const handleResourceChange = (resource: keyof YzeCharacter['resources'], value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      resources: {
        ...character.resources,
        [resource]: value,
      },
    });
  };

  const attributeEntries = Object.keys(ATTRIBUTE_LABELS) as YzeAttribute[];
  const skillsByAttribute = attributeEntries.reduce<Record<YzeAttribute, string[]>>(
    (acc, attribute) => {
      acc[attribute] = Object.keys(YZE_SKILLS).filter((key) => YZE_SKILLS[key].ability === attribute);
      return acc;
    },
    { strength: [], agility: [], wits: [], empathy: [] },
  );

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      {/* Header */}
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
        <div className="mt-2 flex gap-4 text-sm">
          <div>
            <span className="font-semibold">Health:</span> {character.resources.health}
          </div>
          <div>
            <span className="font-semibold">Resolve:</span> {character.resources.resolve}
          </div>
          <div>
            <span className="font-semibold">Stress:</span> {character.resources.stress}
          </div>
          <div>
            <span className="font-semibold">XP:</span> {character.resources.experience_points}
          </div>
        </div>
      </div>

      {/* Tabbed Interface */}
      <Tabs.Root value={selectedTab} onValueChange={setSelectedTab} className="w-full">
        <Tabs.List className="flex gap-2 border-b mb-4">
          <Tabs.Trigger
            value="attributes"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Attributes
          </Tabs.Trigger>
          <Tabs.Trigger
            value="skills"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Skills
          </Tabs.Trigger>
          <Tabs.Trigger
            value="resources"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Resources
          </Tabs.Trigger>
        </Tabs.List>

        {/* Attributes Tab */}
        <Tabs.Content value="attributes" className="p-4">
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
            {attributeEntries.map((attribute) => (
              <div key={attribute} className="flex flex-col items-center gap-1 p-3 border rounded">
                <span className="text-sm font-semibold text-gray-600">{ATTRIBUTE_LABELS[attribute]}</span>
                {isEditable ? (
                  <input
                    type="number"
                    min={1}
                    max={5}
                    value={character.attributes[attribute]}
                    onChange={(e) => handleAttributeChange(attribute, Number(e.target.value))}
                    className="w-16 text-center border rounded"
                  />
                ) : (
                  <span className="text-2xl font-bold">{character.attributes[attribute]}</span>
                )}
              </div>
            ))}
          </div>
        </Tabs.Content>

        {/* Skills Tab */}
        <Tabs.Content value="skills" className="p-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-6">
            {attributeEntries.map((attribute) => (
              <div key={attribute}>
                <h3 className="font-semibold text-gray-700 mb-2">{ATTRIBUTE_LABELS[attribute]}</h3>
                <ul className="space-y-1">
                  {skillsByAttribute[attribute].map((skillKey) => (
                    <li key={skillKey} className="flex items-center justify-between gap-2">
                      <span>{YZE_SKILLS[skillKey].label}</span>
                      {isEditable ? (
                        <input
                          type="number"
                          min={0}
                          max={5}
                          value={character.skills[skillKey] ?? 0}
                          onChange={(e) => handleSkillChange(skillKey, Number(e.target.value))}
                          className="w-14 text-center border rounded"
                        />
                      ) : (
                        <span className="font-semibold">{character.skills[skillKey] ?? 0}</span>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </Tabs.Content>

        {/* Resources Tab */}
        <Tabs.Content value="resources" className="p-4">
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
            {(Object.keys(character.resources) as Array<keyof YzeCharacter['resources']>).map((resource) => (
              <div key={resource} className="flex flex-col items-center gap-1 p-3 border rounded">
                <span className="text-sm font-semibold text-gray-600 capitalize">
                  {resource.replace('_', ' ')}
                </span>
                {isEditable ? (
                  <input
                    type="number"
                    min={0}
                    value={character.resources[resource]}
                    onChange={(e) => handleResourceChange(resource, Number(e.target.value))}
                    className="w-16 text-center border rounded"
                  />
                ) : (
                  <span className="text-2xl font-bold">{character.resources[resource]}</span>
                )}
              </div>
            ))}
          </div>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
