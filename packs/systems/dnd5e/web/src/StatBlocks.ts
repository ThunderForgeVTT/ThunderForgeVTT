/**
 * The creatures this pack can put on an actor, and where each line of a
 * stat block goes.
 *
 * The host finds this file by its path (`systemStatBlocks.ts` in the
 * application explains the seam) and asks it two things: which blocks there
 * are, and what applying one would write. It answers with data only. The host
 * does the writing, with its own permissions and its own API, so nothing here
 * talks to a server.
 *
 * # How a monster has to be written down to fight
 *
 * The combat flow has no notion of a monster. It reads what the manifest
 * points it at, the same for a goblin as for a paladin:
 *
 * | Printed                  | Stored                                             |
 * |--------------------------|----------------------------------------------------|
 * | Hit Points 10 (3d6)      | `resource_data.max_hp`, `current_hp`; `hit_dice`   |
 * | AC 15                    | `ability_data.armor_class`                         |
 * | STR … CHA                | `ability_data.strength` … `charisma`               |
 * | CR 1/4                   | `trait_data.challenge` (gives the proficiency bonus)|
 * | Small                    | `trait_data.size`                                  |
 * | Speed 30 ft., Fly 40 ft. | `trait_data.speed_walk`, `speed_fly`, … in feet    |
 * | Darkvision 60 ft.        | `trait_data.darkvision` in feet                    |
 * | other senses             | `trait_data.blindsight`, `tremorsense`, `truesight`|
 * | Stealth +6               | `proficiency_data.skill_proficiencies`, and        |
 * |                          | `skill_expertise` when the bonus is doubled        |
 * | saving throws            | `proficiency_data.saving_throw_proficiencies`      |
 * | traits                   | `trait_data.traits`, one line each                 |
 *
 * A skill or save bonus is never stored: the server derives it from the score
 * and the challenge rating, which is why the sheet shows +6 for a goblin's
 * Stealth without anyone typing six.
 *
 * **An attack is an ability, not a field.** `1d20+4` as its attack roll,
 * `1d6+2` as its damage (a second kind of damage is further terms on the same
 * formula, because one damage roll is what the flow makes), and a reach or a
 * range in feet. Multiattack is one more ability that names its parts.
 * Anything a block says that is not arithmetic stays prose, for the Game
 * Master to rule on.
 *
 * Hit points are the average the book prints. The dice are kept in
 * `hit_dice` for a Game Master who would rather roll.
 *
 * The same table, with what reads each field, is in
 * `server/src/stat_blocks.rs`, whose tests hold `stat-blocks.json` to the
 * book's arithmetic.
 */

import type {
  StatBlockAttackPlan,
  StatBlockPlan,
  StatBlockSlots,
  StatBlockSource,
} from "@thunderforge/host";
import file from "../../stat-blocks.json";

type Grade = "proficient" | "expertise";

/** One creature, as `stat-blocks.json` has it. */
export interface StatBlock {
  id: string;
  name: string;
  /** The bestiary creature this block answers to, by the bestiary's slug. */
  bestiary: string | null;
  size: string;
  creatureType: string;
  alignment: string;
  armorClass: number;
  /** The average the book prints beside the dice. */
  hitPoints: number;
  hitDice: string;
  /** Feet, by mode: walk, fly, swim, climb, burrow. */
  speed: Record<string, number>;
  abilities: Record<string, number>;
  savingThrows: string[];
  skills: Record<string, Grade>;
  /** Feet, by sense: darkvision, blindsight, tremorsense, truesight. */
  senses: Record<string, number>;
  languages: string[];
  challenge: string;
  traits: { name: string; text: string }[];
  attacks: StatBlockAttack[];
  /** The attacks one Multiattack makes, by name, as often as made. */
  multiattack?: string[];
}

export interface StatBlockAttack {
  name: string;
  /** As printed: ability modifier plus proficiency bonus. */
  toHit: number;
  damage: string;
  /** Feet; null for an attack that is only ranged. */
  reach: number | null;
  /** Normal and long range in feet; null for one that is only melee. */
  range: number[] | null;
  text: string;
}

/**
 * Through `unknown` because the compiler infers each block's own exact shape
 * from the JSON (a goblin with no climb speed is not, to it, a record of
 * speeds). The shape is held by `server/src/stat_blocks.rs`, which parses the
 * same file strictly and fails its tests on a block that does not fit.
 */
export const STAT_BLOCKS = file.blocks as unknown as StatBlock[];

/**
 * The vocabulary type a creature's attack is filed under. This system's
 * abilities are spells, feats and enchantments; an attack is the second.
 */
const ATTACK_CLASSIFICATION = "feat";

/**
 * What a previous occupant of the sheet leaves behind that would contradict
 * the block. A level outranks a challenge rating in the rules, so an NPC that
 * was once given a level would keep a character's proficiency bonus.
 */
const CHARACTER_ONLY_TRAITS = [
  "class",
  "subclass",
  "level",
  "race",
  "background",
  "experience",
  "speed_walk",
  "speed_fly",
  "speed_swim",
  "speed_climb",
  "speed_burrow",
  "darkvision",
  "blindsight",
  "tremorsense",
  "truesight",
  "feats",
];

function signed(value: number): string {
  return value < 0 ? `${value}` : `+${value}`;
}

/** `AC 15, 10 hit points, CR 1/4`: what a picker shows beside the name. */
export function summaryOf(block: StatBlock): string {
  return `AC ${block.armorClass}, ${block.hitPoints} hit points, CR ${block.challenge}`;
}

/** The slots a block writes, laid over whatever the actor already has. */
export function slotsFor(
  block: StatBlock,
  current: StatBlockSlots | null,
): StatBlockSlots {
  const kept = { ...(current?.trait_data ?? {}) };
  for (const key of CHARACTER_ONLY_TRAITS) {
    delete kept[key];
  }
  const speeds = Object.fromEntries(
    Object.entries(block.speed).map(([mode, feet]) => [`speed_${mode}`, feet]),
  );

  return {
    ability_data: { ...block.abilities, armor_class: block.armorClass },
    resource_data: {
      max_hp: block.hitPoints,
      current_hp: block.hitPoints,
      temporary_hp: 0,
      hit_dice: block.hitDice,
    },
    proficiency_data: {
      skill_proficiencies: Object.keys(block.skills),
      skill_expertise: Object.keys(block.skills).filter(
        (skill) => block.skills[skill] === "expertise",
      ),
      saving_throw_proficiencies: block.savingThrows,
      languages: block.languages,
    },
    trait_data: {
      ...kept,
      challenge: block.challenge,
      creature_type: block.creatureType,
      alignment: block.alignment,
      size: block.size,
      ...speeds,
      ...block.senses,
      traits: block.traits.map((trait) => `${trait.name}. ${trait.text}`),
    },
  };
}

function describe(attack: StatBlockAttack): string {
  const where = [
    attack.reach === null ? null : `reach ${attack.reach} ft.`,
    attack.range === null
      ? null
      : `range ${attack.range[0]}/${attack.range[1]} ft.`,
  ]
    .filter(Boolean)
    .join(" or ");
  return `${signed(attack.toHit)} to hit, ${where}. Hit: ${attack.damage}. ${attack.text}`;
}

/**
 * The abilities a block's attacks become. Named for the creature, because a
 * hobgoblin's Longsword and a knight's are different arithmetic and a world
 * holds them side by side.
 */
export function attacksFor(block: StatBlock): StatBlockAttackPlan[] {
  const plans: StatBlockAttackPlan[] = block.attacks.map((attack) => ({
    key: attack.name,
    name: `${attack.name} (${block.name})`,
    description: describe(attack),
    classification: ATTACK_CLASSIFICATION,
    attackRoll: `1d20${signed(attack.toHit)}`,
    damage: attack.damage,
    reach: attack.reach,
    rangeNormal: attack.range?.[0] ?? null,
    rangeLong: attack.range?.[1] ?? null,
    parts: [],
  }));

  if (block.multiattack && block.multiattack.length > 0) {
    plans.push({
      key: "Multiattack",
      name: `Multiattack (${block.name})`,
      description: `Makes ${block.multiattack.join(", ")}.`,
      classification: ATTACK_CLASSIFICATION,
      attackRoll: null,
      damage: null,
      reach: null,
      rangeNormal: null,
      rangeLong: null,
      parts: block.multiattack,
    });
  }
  return plans;
}

const source: StatBlockSource = {
  blocks: STAT_BLOCKS.map((block) => ({
    id: block.id,
    name: block.name,
    bestiary: block.bestiary,
    summary: summaryOf(block),
  })),
  plan(id, current): StatBlockPlan | null {
    const block = STAT_BLOCKS.find((candidate) => candidate.id === id);
    if (!block) {
      return null;
    }
    return {
      name: block.name,
      slots: slotsFor(block, current),
      attacks: attacksFor(block),
    };
  },
};

export default source;
