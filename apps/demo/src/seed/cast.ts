/**
 * Who is in the demo world.
 *
 * Two heroes, written by hand in the shape the 5e sheet reads (the pack's
 * `validators.rs` says what each slot may hold), and the ambushers, taken from
 * the pack's SRD 5.2.1 stat blocks through the same mapper the real app uses
 * when a Game Master applies one to an NPC. The heroes belong to the seeded
 * player, so "view as player" has something to own; the ambushers are hidden
 * from players until the Game Master reveals them, as a real ambush would be.
 */
import {
  STAT_BLOCKS,
  slotsFor,
} from "../../../../packs/systems/dnd5e/web/src/StatBlocks";
import type { Row } from "../backend/state";

export type SystemSlots = Record<
  | "ability_data"
  | "resource_data"
  | "proficiency_data"
  | "trait_data"
  | "spell_data",
  Record<string, unknown> | null
>;

export interface CastMember {
  /** Stable key the encounter refers to. */
  key: string;
  label: string;
  description: string;
  isNpc: boolean;
  /** Spec 046: a named individual's tokens are linked; a kind's are copies. */
  isUnique: boolean;
  /** Owner decision 2026-09-15: NPCs start hidden from players. */
  visibleToPlayers: boolean;
  slots: SystemSlots;
}

function fromStatBlock(
  key: string,
  blockId: string,
  description: string,
): CastMember {
  const block = STAT_BLOCKS.find((candidate) => candidate.id === blockId);
  if (!block) {
    throw new Error(
      `the demo cast names a stat block the pack lacks: ${blockId}`,
    );
  }
  return {
    key,
    label: block.name,
    description,
    isNpc: true,
    isUnique: false,
    visibleToPlayers: false,
    slots: { ...slotsFor(block, null), spell_data: null } as SystemSlots,
  };
}

export const CAST: CastMember[] = [
  {
    key: "fighter",
    label: "Brannoc Stoneward",
    description:
      "Human fighter, third level. A soldier who took the road south when the war ended and has not stopped walking.",
    isNpc: false,
    isUnique: true,
    visibleToPlayers: true,
    slots: {
      ability_data: {
        strength: 16,
        dexterity: 13,
        constitution: 14,
        intelligence: 10,
        wisdom: 12,
        charisma: 8,
        armor_class: 18,
      },
      resource_data: {
        max_hp: 28,
        current_hp: 28,
        temporary_hp: 0,
        hit_dice: "3d10",
        hit_dice_used: 0,
        death_save_successes: 0,
        death_save_failures: 0,
      },
      proficiency_data: {
        skill_proficiencies: ["athletics", "intimidation", "perception"],
        skill_expertise: [],
        saving_throw_proficiencies: ["strength", "constitution"],
        proficiency_bonus: 2,
        armor: ["Light", "Medium", "Heavy", "Shields"],
        weapons: ["Simple", "Martial"],
        tools: ["Dice set"],
        languages: ["Common", "Dwarvish"],
      },
      trait_data: {
        class: "Fighter",
        subclass: "Champion",
        level: 3,
        race: "Human",
        background: "Soldier",
        alignment: "Lawful Good",
        size: "medium",
        speed_walk: 30,
        darkvision: 0,
        experience: 900,
        inspiration: false,
        feats: [],
        traits: [
          "Second Wind. Bonus Action: regain 1d10 + 3 hit points, once per rest.",
          "Action Surge. Take one additional action, once per rest.",
          "Improved Critical. Weapon attacks score a critical hit on a 19 or 20.",
        ],
        notes: "",
      },
      spell_data: null,
    },
  },
  {
    key: "wizard",
    label: "Elowen Vire",
    description:
      "Elf wizard, third level. Left the academy with a satchel of notes and a conviction that the field teaches faster.",
    isNpc: false,
    isUnique: true,
    visibleToPlayers: true,
    slots: {
      ability_data: {
        strength: 8,
        dexterity: 14,
        constitution: 13,
        intelligence: 16,
        wisdom: 12,
        charisma: 10,
        armor_class: 12,
      },
      resource_data: {
        max_hp: 17,
        current_hp: 17,
        temporary_hp: 0,
        hit_dice: "3d6",
        hit_dice_used: 0,
        death_save_successes: 0,
        death_save_failures: 0,
      },
      proficiency_data: {
        skill_proficiencies: [
          "arcana",
          "history",
          "investigation",
          "perception",
        ],
        skill_expertise: [],
        saving_throw_proficiencies: ["intelligence", "wisdom"],
        proficiency_bonus: 2,
        armor: [],
        weapons: ["Simple"],
        tools: [],
        languages: ["Common", "Elvish", "Draconic"],
      },
      trait_data: {
        class: "Wizard",
        subclass: "Evoker",
        level: 3,
        race: "Elf",
        background: "Sage",
        alignment: "Neutral Good",
        size: "medium",
        speed_walk: 30,
        darkvision: 60,
        experience: 900,
        inspiration: false,
        feats: [],
        traits: [
          "Arcane Recovery. Once a day after a short rest, recover spell slots totalling one level.",
          "Fey Ancestry. Advantage on saves against being charmed; magic cannot put her to sleep.",
        ],
        notes: "",
      },
      spell_data: {
        spellcasting_ability: "intelligence",
        spell_save_dc: 13,
        spell_attack_bonus: 5,
        cantrips_known: ["Fire Bolt", "Mage Hand", "Light", "Prestidigitation"],
        spells_known: [
          "Magic Missile",
          "Shield",
          "Sleep",
          "Mage Armor",
          "Misty Step",
          "Scorching Ray",
        ],
        // Stored, not derived: the sheet's slot table is off by one at the
        // first level and would show a third-level wizard no slots at all.
        spell_slots: { level_1: 4, level_2: 2 },
        spell_slots_used: { level_1: 0, level_2: 0 },
      },
    },
  },
  fromStatBlock(
    "goblin",
    "goblin-warrior",
    "Three of them, in the long grass either side of the road. They are after the packs, not a fight to the death.",
  ),
  fromStatBlock(
    "hobgoblin",
    "hobgoblin-warrior",
    "The one who planned this. Holds the far side of the road and will not be the first to run.",
  ),
  fromStatBlock(
    "wolf",
    "dire-wolf",
    "The hobgoblin's. Waits behind the rocks for the first hero to break from the group.",
  ),
];

/** `cast[key]` as the pack's data type name → that slot's JSON. */
export function slotRows(member: CastMember): Array<[string, Row | null]> {
  return Object.entries(member.slots) as Array<[string, Row | null]>;
}
