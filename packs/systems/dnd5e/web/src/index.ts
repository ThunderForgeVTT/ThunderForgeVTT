//! D&D 5e Web Package - System Manifest Export
//!
//! Exports the D&D 5e system manifest for lazy-loading in the core web app.
//! The manifest includes React components, RxDB schema extensions, and derived data calculators.

export interface DnD5eSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
    AbilityScores: React.ComponentType<any>;
    SkillsList: React.ComponentType<any>;
    Spellbook: React.ComponentType<any>;
  };
  rxdbSchema: Record<string, any>;
  derivedDataCalculators: {
    abilityModifier: (score: number) => number;
    skillBonus: (abilityMod: number, isProficient: boolean, profBonus: number) => number;
    proficiencyBonus: (level: number) => number;
    maxSpellSlots: (level: number, spellLevel: number) => number;
  };
}

// Lazy-loaded React components
import CharacterSheet from './components/CharacterSheet';
import AbilityScores from './components/AbilityScores';
import SkillsList from './components/SkillsList';
import Spellbook from './components/Spellbook';

// Derived data calculators
export const DerivedDataCalculators = {
  abilityModifier: (score: number): number => {
    return Math.floor((score - 10) / 2);
  },

  skillBonus: (abilityMod: number, isProficient: boolean, profBonus: number): number => {
    return abilityMod + (isProficient ? profBonus : 0);
  },

  proficiencyBonus: (level: number): number => {
    if (level <= 4) return 2;
    if (level <= 8) return 3;
    if (level <= 12) return 4;
    if (level <= 16) return 5;
    return 6;
  },

  maxSpellSlots: (characterLevel: number, spellLevel: number): number => {
    const spellSlotsByLevel: Record<number, number[]> = {
      1: [2, 0, 0, 0, 0, 0, 0, 0, 0],
      2: [3, 2, 0, 0, 0, 0, 0, 0, 0],
      3: [4, 3, 2, 0, 0, 0, 0, 0, 0],
      4: [4, 3, 3, 2, 0, 0, 0, 0, 0],
      5: [4, 4, 3, 3, 2, 0, 0, 0, 0],
      6: [4, 4, 3, 3, 3, 2, 0, 0, 0],
      7: [4, 4, 4, 3, 3, 3, 2, 0, 0],
      8: [4, 4, 4, 3, 3, 3, 3, 2, 0],
      9: [4, 4, 4, 4, 3, 3, 3, 3, 3],
      10: [5, 4, 4, 4, 3, 3, 3, 3, 3],
      11: [5, 4, 4, 4, 4, 3, 3, 3, 3],
      12: [5, 4, 4, 4, 4, 3, 3, 3, 3],
      13: [5, 4, 4, 4, 4, 4, 3, 3, 3],
      14: [5, 4, 4, 4, 4, 4, 3, 3, 3],
      15: [5, 4, 4, 4, 4, 4, 4, 3, 3],
      16: [5, 4, 4, 4, 4, 4, 4, 3, 3],
      17: [5, 5, 4, 4, 4, 4, 4, 4, 3],
      18: [5, 5, 4, 4, 4, 4, 4, 4, 3],
      19: [5, 5, 4, 4, 4, 4, 4, 4, 4],
      20: [5, 5, 4, 4, 4, 4, 4, 4, 4],
    };

    const slots = spellSlotsByLevel[characterLevel];
    return slots && spellLevel < slots.length ? slots[spellLevel] : 0;
  },
};

// RxDB schema extensions for D&D 5e actors and items
export const RxDBSchema = {
  DnD5eActorData: {
    title: 'D&D 5e Actor',
    version: 0,
    type: 'object',
    properties: {
      class: {
        type: 'string',
        description: 'Character class (e.g., Rogue, Cleric)',
      },
      level: {
        type: 'integer',
        minimum: 1,
        maximum: 20,
        description: 'Character level',
      },
      abilities: {
        type: 'object',
        properties: {
          strength: { type: 'integer' },
          dexterity: { type: 'integer' },
          constitution: { type: 'integer' },
          intelligence: { type: 'integer' },
          wisdom: { type: 'integer' },
          charisma: { type: 'integer' },
        },
      },
      proficiencies: {
        type: 'object',
        properties: {
          skills: { type: 'object' },
          savingThrows: { type: 'object' },
          weapons: { type: 'array', items: { type: 'string' } },
          armor: { type: 'array', items: { type: 'string' } },
          tools: { type: 'array', items: { type: 'string' } },
          languages: { type: 'array', items: { type: 'string' } },
        },
      },
      hitPoints: { type: 'integer' },
      armorClass: { type: 'integer' },
      experience: { type: 'integer' },
      knownSpells: { type: 'array', items: { type: 'string' } },
    },
    required: ['class', 'level', 'abilities'],
  },
};

// Export D&D 5e System Manifest
export const dnd5eSystemManifest: DnD5eSystemManifest = {
  id: 'dnd5e',
  title: 'Dungeons & Dragons 5th Edition',
  version: '0.1.0',
  components: {
    CharacterSheet,
    AbilityScores,
    SkillsList,
    Spellbook,
  },
  rxdbSchema: RxDBSchema,
  derivedDataCalculators: DerivedDataCalculators,
};

export default dnd5eSystemManifest;
