//! RxDB Schema Extensions for D&D 5e
//!
//! Defines RxDB collection schema additions for D&D 5e actors and items.
//! These are merged with the base world_tokens schema to extend functionality.

export const dnd5eActorSchema = {
  title: 'D&D 5e Actor Data',
  version: 0,
  type: 'object',
  properties: {
    // Base actor type
    actor_type: {
      type: 'string',
      enum: ['player_character', 'npc', 'monster'],
      description: 'Type of actor',
    },

    // Class and Level
    class_name: {
      type: 'string',
      description: 'Character class (Barbarian, Bard, Cleric, etc.)',
    },
    level: {
      type: 'integer',
      minimum: 1,
      maximum: 20,
      description: 'Character level',
    },

    // Ability Scores (base data)
    ability_scores: {
      type: 'object',
      properties: {
        strength: { type: 'integer', minimum: 1, maximum: 20 },
        dexterity: { type: 'integer', minimum: 1, maximum: 20 },
        constitution: { type: 'integer', minimum: 1, maximum: 20 },
        intelligence: { type: 'integer', minimum: 1, maximum: 20 },
        wisdom: { type: 'integer', minimum: 1, maximum: 20 },
        charisma: { type: 'integer', minimum: 1, maximum: 20 },
      },
      required: ['strength', 'dexterity', 'constitution', 'intelligence', 'wisdom', 'charisma'],
    },

    // Proficiencies (base data)
    proficiencies: {
      type: 'object',
      properties: {
        skill_proficiencies: {
          type: 'object',
          additionalProperties: { type: 'boolean' },
          description: 'Map of skill name to proficiency boolean',
        },
        saving_throw_proficiencies: {
          type: 'object',
          additionalProperties: { type: 'boolean' },
          description: 'Map of ability name to proficiency boolean',
        },
        weapon_proficiencies: {
          type: 'array',
          items: { type: 'string' },
          description: 'List of weapon type proficiencies',
        },
        armor_proficiencies: {
          type: 'array',
          items: { type: 'string' },
          description: 'List of armor type proficiencies',
        },
        tool_proficiencies: {
          type: 'array',
          items: { type: 'string' },
          description: 'List of tool proficiencies',
        },
        languages: {
          type: 'array',
          items: { type: 'string' },
          description: 'Known languages',
        },
      },
    },

    // Hit Points (base data)
    max_hit_points: {
      type: 'integer',
      minimum: 1,
      description: 'Maximum hit points',
    },
    current_hit_points: {
      type: 'integer',
      minimum: 0,
      description: 'Current hit points',
    },

    // Armor Class (base data)
    armor_class: {
      type: 'integer',
      minimum: 1,
      description: 'Armor class',
    },

    // Experience and Advancement
    experience_points: {
      type: 'integer',
      minimum: 0,
      description: 'Total experience points',
    },

    // Spellcasting (base data for full casters)
    known_spells: {
      type: 'array',
      items: { type: 'string' },
      description: 'List of known spell IDs',
    },
    prepared_spells: {
      type: 'array',
      items: { type: 'string' },
      description: 'List of prepared spell IDs (for preparing casters)',
    },

    // Currency
    currency: {
      type: 'object',
      properties: {
        platinum: { type: 'integer', minimum: 0 },
        gold: { type: 'integer', minimum: 0 },
        electrum: { type: 'integer', minimum: 0 },
        silver: { type: 'integer', minimum: 0 },
        copper: { type: 'integer', minimum: 0 },
      },
    },

    // Metadata
    inspiration_points: { type: 'integer', minimum: 0, maximum: 1 },
    personality_traits: { type: 'string' },
    ideals: { type: 'string' },
    bonds: { type: 'string' },
    flaws: { type: 'string' },
  },
  required: [
    'actor_type',
    'class_name',
    'level',
    'ability_scores',
    'proficiencies',
    'max_hit_points',
    'current_hit_points',
    'armor_class',
  ],
};

export const dnd5eItemSchema = {
  title: 'D&D 5e Item',
  version: 0,
  type: 'object',
  properties: {
    item_type: {
      type: 'string',
      enum: ['weapon', 'armor', 'spell', 'equipment', 'consumable', 'treasure'],
    },
    name: { type: 'string' },
    description: { type: 'string' },
    rarity: {
      type: 'string',
      enum: ['common', 'uncommon', 'rare', 'very_rare', 'legendary', 'artifact'],
    },
    weight: { type: 'number', minimum: 0 },
    value_gp: { type: 'number', minimum: 0 },
  },
  required: ['item_type', 'name'],
};
