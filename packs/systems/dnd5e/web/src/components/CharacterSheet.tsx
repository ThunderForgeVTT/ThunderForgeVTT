import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import AbilityScores from './AbilityScores';
import SkillsList from './SkillsList';
import Spellbook from './Spellbook';

export interface DnD5eCharacter {
  id: string;
  name: string;
  class: string;
  level: number;
  experience: number;
  hitPoints: number;
  maxHitPoints: number;
  armorClass: number;
  abilities: {
    strength: number;
    dexterity: number;
    constitution: number;
    intelligence: number;
    wisdom: number;
    charisma: number;
  };
  proficiencies: {
    skills: Record<string, boolean>;
    savingThrows: Record<string, boolean>;
    weapons: string[];
    armor: string[];
  };
  knownSpells: string[];
}

interface CharacterSheetProps {
  character?: DnD5eCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<DnD5eCharacter>) => void;
}

/**
 * D&D 5e Character Sheet Component
 *
 * Main character display UI with tabbed interface:
 * - Attributes: Ability scores and derived stats
 * - Skills: All 18 skills with proficiency UI
 * - Spellbook: Known spells and spell slots
 * - Combat: HP, AC, initiative, etc.
 *
 * Uses RxDB to read/write character data; all derived stats calculated client-side
 */
const CharacterSheet: React.FC<CharacterSheetProps> = ({
  character,
  isEditable = true,
  onUpdate,
}) => {
  const [selectedTab, setSelectedTab] = useState<string>('attributes');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const handleAbilityChange = (ability: keyof typeof character.abilities, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      abilities: {
        ...character.abilities,
        [ability]: value,
      },
    });
  };

  const handleSkillToggle = (skill: string) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      proficiencies: {
        ...character.proficiencies,
        skills: {
          ...character.proficiencies.skills,
          [skill]: !character.proficiencies.skills[skill],
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
          Level {character.level} {character.class}
        </p>
        <div className="mt-2 flex gap-4 text-sm">
          <div>
            <span className="font-semibold">HP:</span> {character.hitPoints}/{character.maxHitPoints}
          </div>
          <div>
            <span className="font-semibold">AC:</span> {character.armorClass}
          </div>
          <div>
            <span className="font-semibold">XP:</span> {character.experience}
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
            value="spellbook"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Spellbook
          </Tabs.Trigger>
        </Tabs.List>

        {/* Attributes Tab */}
        <Tabs.Content value="attributes" className="p-4">
          <AbilityScores
            abilities={character.abilities}
            profBonus={calculateProficiencyBonus(character.level)}
            savingThrowProficiencies={character.proficiencies.savingThrows}
            isEditable={isEditable}
            onAbilityChange={handleAbilityChange}
          />
        </Tabs.Content>

        {/* Skills Tab */}
        <Tabs.Content value="skills" className="p-4">
          <SkillsList
            abilities={character.abilities}
            skillProficiencies={character.proficiencies.skills}
            profBonus={calculateProficiencyBonus(character.level)}
            isEditable={isEditable}
            onSkillToggle={handleSkillToggle}
          />
        </Tabs.Content>

        {/* Spellbook Tab */}
        <Tabs.Content value="spellbook" className="p-4">
          <Spellbook
            knownSpells={character.knownSpells}
            characterLevel={character.level}
            characterClass={character.class}
            isEditable={isEditable}
          />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

/**
 * Calculate proficiency bonus based on character level
 * D&D 5e formula: +2 (levels 1-4), +3 (5-8), +4 (9-12), +5 (13-16), +6 (17-20)
 */
function calculateProficiencyBonus(level: number): number {
  if (level <= 4) return 2;
  if (level <= 8) return 3;
  if (level <= 12) return 4;
  if (level <= 16) return 5;
  return 6;
}

export default CharacterSheet;
