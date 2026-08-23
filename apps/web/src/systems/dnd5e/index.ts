/**
 * apps/web/src/systems/dnd5e/index.ts
 * 5E System Core Web Module Entry Point
 *
 * Phase 4.8.1: System-Aware React Components (Phase E.1)
 *
 * Exports all 5E System Core game system web assets:
 * - React components for character sheet
 * - RxDB schema extensions
 * - Local derived data calculators
 * - System manifest configuration
 *
 * This module is lazy-loaded by the core web app (@thunderforge/web)
 * when a world specifies gameSystemId: "dnd5e".
 */

import { CharacterSheet } from "@/components/game-systems/dnd5e/CharacterSheet";
import { AbilityScores } from "@/components/game-systems/dnd5e/AbilityScores";
import { SkillsList } from "@/components/game-systems/dnd5e/SkillsList";

/**
 * 5E System Core Manifest
 * Defines metadata, capabilities, and schema for the system
 */
export const DnD5eSystemManifest = {
  id: "dnd5e",
  title: "5E System Core",
  version: "0.1.0",
  author: "ThunderForge Contributors",
  compatibility: {
    minEngineVersion: "0.4.0",
  },

  /**
   * Data type schemas matching the system.json manifest
   * Validate actor system data structure server-side
   */
  data_types: {
    ability_data: {
      description: "D&D 5e Ability Scores (3-20)",
      properties: {
        strength: { type: "integer", min: 3, max: 20 },
        dexterity: { type: "integer", min: 3, max: 20 },
        constitution: { type: "integer", min: 3, max: 20 },
        intelligence: { type: "integer", min: 3, max: 20 },
        wisdom: { type: "integer", min: 3, max: 20 },
        charisma: { type: "integer", min: 3, max: 20 },
      },
      required: ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"],
    },

    resource_data: {
      description: "D&D 5e Resources (HP, Spell Slots, Inspiration)",
      properties: {
        hit_points: {
          type: "object",
          properties: {
            current: { type: "integer", min: 0 },
            max: { type: "integer", min: 1 },
            temporary: { type: "integer", min: 0 },
          },
        },
        spell_slots: { type: "object" },
        inspiration: { type: "integer", min: 0, max: 1 },
        hp: { type: "integer", min: 0 },
        ac: { type: "integer", min: 0 },
        speed: { type: "integer", min: 0 },
      },
    },

    proficiency_data: {
      description: "D&D 5e Proficiencies (Skills, Saving Throws, Tools)",
      properties: {
        skills: { type: "object" },
        saving_throws: { type: "object" },
        armor: { type: "array" },
        weapons: { type: "array" },
        languages: { type: "array" },
        tools: { type: "object" },
      },
    },

    trait_data: {
      description: "D&D 5e Character Traits (Class, Subclass, Feats, Personality)",
      properties: {
        class: { type: "string" },
        level: { type: "integer", min: 1, max: 20 },
        subclass: { type: "string" },
        race: { type: "string" },
        background: { type: "string" },
        alignment: { type: "string" },
        personality_traits: { type: "string" },
        ideals: { type: "string" },
        bonds: { type: "string" },
        flaws: { type: "string" },
        feats: { type: "array" },
        experience: { type: "integer", min: 0 },
      },
    },

    spell_data: {
      description: "D&D 5e Spellcasting (Known Spells, Prepared Spells, Slots)",
      properties: {
        known_spells: { type: "array" },
        prepared_spells: { type: "array" },
        cantrips: { type: "array" },
        ritual_spells: { type: "array" },
        spellcasting_ability: { type: "string" },
        spell_save_dc: { type: "integer" },
        spell_attack_bonus: { type: "integer" },
        spell_slots: { type: "object" },
      },
    },
  },

  /**
   * React Components
   * Lazy-loaded by the main app when displaying D&D 5e characters
   */
  components: {
    CharacterSheet,
    AbilityScores,
    SkillsList,
  },

  /**
   * Local Derived Data Calculators
   * These run in React to avoid redundant network calls
   */
  calculators: {
    /**
     * Calculate ability modifier from ability score
     * D&D 5e formula: (score - 10) / 2, rounded down
     */
    calculateAbilityModifier(score: number): number {
      return Math.floor((score - 10) / 2);
    },

    /**
     * Calculate passive skill check
     * D&D 5e formula: 10 + ability modifier + (proficiency bonus if proficient)
     */
    calculatePassiveSkill(
      abilityModifier: number,
      isProficient: boolean,
      proficiencyBonus: number = 2,
    ): number {
      return 10 + abilityModifier + (isProficient ? proficiencyBonus : 0);
    },

    /**
     * Calculate proficiency bonus from character level
     * D&D 5e formula: floor((level - 1) / 4) + 2
     */
    calculateProficiencyBonus(level: number): number {
      return Math.floor((level - 1) / 4) + 2;
    },

    /**
     * Get ability score color quality indicator
     * Used for visual design feedback in UI
     */
    getScoreQuality(score: number): "critical" | "poor" | "average" | "good" | "excellent" | "legendary" {
      if (score <= 3) return "critical";
      if (score <= 6) return "poor";
      if (score <= 10) return "average";
      if (score <= 14) return "good";
      if (score <= 17) return "excellent";
      return "legendary";
    },

    /**
     * Get color for score quality
     */
    getQualityColor(quality: string): string {
      const colors: Record<string, string> = {
        critical: "#d32f2f",
        poor: "#f57c00",
        average: "#fbc02d",
        good: "#558b2f",
        excellent: "#1565c0",
        legendary: "#7b1fa2",
      };
      return colors[quality] ?? "#666";
    },

    /**
     * Calculate ability score modifier bonuses
     * Returns full modifier data for all 6 abilities
     */
    calculateAllModifiers(abilityData: Record<string, number>): Record<string, number> {
      return {
        strength_mod: this.calculateAbilityModifier(abilityData.strength ?? 10),
        dexterity_mod: this.calculateAbilityModifier(abilityData.dexterity ?? 10),
        constitution_mod: this.calculateAbilityModifier(abilityData.constitution ?? 10),
        intelligence_mod: this.calculateAbilityModifier(abilityData.intelligence ?? 10),
        wisdom_mod: this.calculateAbilityModifier(abilityData.wisdom ?? 10),
        charisma_mod: this.calculateAbilityModifier(abilityData.charisma ?? 10),
      };
    },
  },

  /**
   * Skills Database
   * Defines all 18 skills with their associated abilities
   */
  skills: [
    { id: "acrobatics", name: "Acrobatics", ability: "dexterity" },
    { id: "animal_handling", name: "Animal Handling", ability: "wisdom" },
    { id: "arcana", name: "Arcana", ability: "intelligence" },
    { id: "athletics", name: "Athletics", ability: "strength" },
    { id: "deception", name: "Deception", ability: "charisma" },
    { id: "history", name: "History", ability: "intelligence" },
    { id: "insight", name: "Insight", ability: "wisdom" },
    { id: "intimidation", name: "Intimidation", ability: "charisma" },
    { id: "investigation", name: "Investigation", ability: "intelligence" },
    { id: "medicine", name: "Medicine", ability: "wisdom" },
    { id: "nature", name: "Nature", ability: "intelligence" },
    { id: "perception", name: "Perception", ability: "wisdom" },
    { id: "performance", name: "Performance", ability: "charisma" },
    { id: "persuasion", name: "Persuasion", ability: "charisma" },
    { id: "religion", name: "Religion", ability: "intelligence" },
    { id: "sleight_of_hand", name: "Sleight of Hand", ability: "dexterity" },
    { id: "stealth", name: "Stealth", ability: "dexterity" },
    { id: "survival", name: "Survival", ability: "wisdom" },
  ],
};

/**
 * Export individual components for direct import
 */
export { CharacterSheet } from "@/components/game-systems/dnd5e/CharacterSheet";
export { AbilityScores } from "@/components/game-systems/dnd5e/AbilityScores";
export { SkillsList } from "@/components/game-systems/dnd5e/SkillsList";

/**
 * Export type definitions for consumers
 */
export type DnD5eSystemManifestType = typeof DnD5eSystemManifest;
