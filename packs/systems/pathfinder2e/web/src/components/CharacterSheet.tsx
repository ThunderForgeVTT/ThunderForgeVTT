import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import AbilityScores, { type Pathfinder2eAbilities } from './AbilityScores';
import SkillsList, { type ProficiencyRank } from './SkillsList';

export interface Pathfinder2eCharacter {
  id: string;
  name: string;
  ancestry: string;
  class: string;
  level: number;
  currentHp: number;
  maxHp: number;
  focusPoints: number;
  heroPoints: number;
  abilities: Pathfinder2eAbilities;
  skillProficiencies: Record<string, ProficiencyRank>;
}

interface CharacterSheetProps {
  character?: Pathfinder2eCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<Pathfinder2eCharacter>) => void;
}

/**
 * Pathfinder Second Edition (Remaster) Character Sheet Component
 *
 * Mirrors packs/systems/dnd5e/web/src/components/CharacterSheet.tsx's
 * conventions (Radix tabs, props-driven, no internal data fetching) at
 * the same depth: a tabbed Attributes/Skills view over the ability
 * modifiers and skill list from research/system_pathfinder2e.json.
 *
 * Uses RxDB to read/write character data; all derived stats calculated
 * client-side.
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

  const handleAbilityChange = (ability: keyof Pathfinder2eAbilities, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      abilities: {
        ...character.abilities,
        [ability]: value,
      },
    });
  };

  const handleSkillRankChange = (skill: string, rank: ProficiencyRank) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      skillProficiencies: {
        ...character.skillProficiencies,
        [skill]: rank,
      },
    });
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      {/* Header */}
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
        <p className="text-lg text-gray-600">
          Level {character.level} {character.ancestry} {character.class}
        </p>
        <div className="mt-2 flex gap-4 text-sm">
          <div>
            <span className="font-semibold">HP:</span> {character.currentHp}/{character.maxHp}
          </div>
          <div>
            <span className="font-semibold">Focus Points:</span> {character.focusPoints}
          </div>
          <div>
            <span className="font-semibold">Hero Points:</span> {character.heroPoints}
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
        </Tabs.List>

        {/* Attributes Tab */}
        <Tabs.Content value="attributes" className="p-4">
          <AbilityScores
            abilities={character.abilities}
            isEditable={isEditable}
            onAbilityChange={handleAbilityChange}
          />
        </Tabs.Content>

        {/* Skills Tab */}
        <Tabs.Content value="skills" className="p-4">
          <SkillsList
            abilities={character.abilities}
            characterLevel={character.level}
            skillProficiencies={character.skillProficiencies}
            isEditable={isEditable}
            onSkillRankChange={handleSkillRankChange}
          />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
