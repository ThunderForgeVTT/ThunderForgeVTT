/**
 * What a creature type looks like, and what a named creature looks like.
 *
 * This is the whole of the bestiary's knowledge, as data. Two tables, because
 * a statblock gives two different kinds of clue and they are worth different
 * amounts:
 *
 * - `FAMILIES` is keyed by the creature type word every 5e statblock carries
 *   on its descriptor line — "Gargantuan dragon, chaotic evil". It is the
 *   floor: a creature nobody has ever heard of still draws as something,
 *   because its type alone says a great deal.
 * - `CREATURES` is keyed by a word in the creature's *name*, and refines a
 *   family into the thing people picture when they hear "troll". A name that
 *   matches nothing here is not a failure; it just falls back to its family.
 *
 * Neither table holds a statistic, a name from a book, or anything but our
 * own drawing choices (spec 047 FR-050): a goblin here is a palette and a set
 * of parts, not a publisher's goblin.
 */
import { MONSTER_TONES, SKIN_TONES } from "./spec.ts";
import type { ResolvedHero, SizeCategory } from "./spec.ts";

/** Parts rolled from a seed: one value per field, per creature. */
export type Varies = {
  [K in keyof Omit<ResolvedHero, "name">]?: readonly ResolvedHero[K][];
};

export interface CreatureKind {
  /** What size the thing is when the source did not say. */
  size: SizeCategory;
  /** The parts that make it this creature, and never vary. */
  always: Partial<Omit<ResolvedHero, "name">>;
  /** The parts that tell one of them from the next. */
  sometimes: Varies;
}

/** A named creature, as a difference from its family. */
export interface CreatureEntry extends Partial<CreatureKind> {
  family: FamilyName;
}

const GREENS = [
  MONSTER_TONES.goblinGreen,
  MONSTER_TONES.orcMoss,
  "#6f9c46",
] as const;

export const FAMILIES = {
  humanoid: {
    size: "medium",
    always: {},
    sometimes: {
      skin: Object.values(SKIN_TONES),
      hair: ["short", "long", "bald", "braids", "spiky"],
      hairColor: ["#3b2a1f", "#2b2233", "#8a5635", "#d9d2c5"],
      headgear: ["none", "helm", "hood", "headband", "cap"],
      outfit: ["#6b5a44", "#5a6a7a", "#7a4a3a", "#4a5a3a"],
      prop: ["sword", "axe", "dagger", "bow", "club"],
      mouth: ["smirk", "snarl", "grin"],
      glow: ["#c9d6e8", "#d8cfc0"],
    },
  },
  beast: {
    size: "medium",
    always: {
      build: "hunched",
      muzzle: "snout",
      hide: "fur",
      ears: "pointed",
      eyeStyle: "beady",
      hair: "bald",
      prop: "claws",
      tail: "tuft",
    },
    sometimes: {
      skin: [MONSTER_TONES.beastBrown, "#6b5a4a", "#9a8a72", "#4a4038"],
      eyes: ["#e0a63c", "#7a3b1f", "#c9a96e"],
      outfit: ["#7a5c40", "#5f5145", "#8a7458"],
      glow: ["#cfe0b8", "#d8cfc0"],
    },
  },
  dragon: {
    size: "large",
    always: {
      build: "sinuous",
      muzzle: "maw",
      hide: "scales",
      headgear: "crest",
      wings: "bat",
      tail: "spiked",
      eyeStyle: "slit",
      ears: "pointed",
      hair: "bald",
      prop: "none",
    },
    sometimes: {
      skin: [
        MONSTER_TONES.dragonScarlet,
        MONSTER_TONES.dragonEmerald,
        MONSTER_TONES.dragonSapphire,
        "#3a3340",
        "#e2e8ee",
      ],
      eyes: ["#f4c542", "#e0602f", "#8fd3e8"],
      outfit: ["#e0b840", "#d9c8a0", "#c9a96e"],
      glow: ["#ffd9a3", "#c9b8ff", "#b8e0f0"],
    },
  },
  undead: {
    size: "medium",
    always: {
      hide: "bone",
      eyeStyle: "hollow",
      hair: "bald",
      mouth: "snarl",
      hideColor: "#6c6a5c",
    },
    sometimes: {
      skin: [MONSTER_TONES.boneWhite, MONSTER_TONES.graveGrey, "#b9b8a4"],
      eyes: ["#8fd3e8", "#7bb04a", "#e0602f"],
      outfit: ["#4a4458", "#5a5346", "#3d4a46"],
      headgear: ["none", "boneCrown", "hood"],
      prop: ["none", "dagger", "club", "sword"],
      glow: ["#9aa79a", "#a8a0c0"],
    },
  },
  fiend: {
    size: "medium",
    always: {
      eyeStyle: "slit",
      mouth: "snarl",
      tail: "spiked",
      hair: "bald",
      hide: "scales",
      hideColor: "#4a1f28",
    },
    sometimes: {
      skin: [MONSTER_TONES.fiendCrimson, "#b8322f", "#5a2f4a"],
      headgear: ["horned", "tiefling", "crest"],
      wings: ["bat", "none"],
      prop: ["claws", "flame", "sword"],
      eyes: ["#f4c542", "#ff9a3c"],
      outfit: ["#3b2a4a", "#5a2b2b"],
      glow: ["#ffb38a", "#d98a8a"],
    },
  },
  giant: {
    size: "large",
    always: { build: "hulking", prop: "club", beard: true },
    sometimes: {
      skin: [MONSTER_TONES.ogreClay, "#b98a62", MONSTER_TONES.trollStone],
      hair: ["bald", "long", "spiky"],
      hairColor: ["#8a5635", "#d9d2c5", "#3b2a1f"],
      outfit: ["#8a6a4a", "#6b5a44", "#7a4a3a"],
      mouth: ["snarl", "grin", "gape"],
      glow: ["#e6c8a0", "#cfd8c0"],
    },
  },
  fey: {
    size: "small",
    always: { ears: "pointed", wings: "feathered" },
    sometimes: {
      skin: [SKIN_TONES.porcelain, SKIN_TONES.sage, "#e8c6d8"],
      hair: ["long", "curly", "braids"],
      hairColor: ["#e8d27a", "#7bb04a", "#f4a3c0"],
      headgear: ["leaves", "circlet", "none"],
      prop: ["vine", "dagger", "none"],
      outfit: ["#6a8f3a", "#4e8a3a", "#a86ac4"],
      glow: ["#cfe8a0", "#e8c6f0"],
    },
  },
  celestial: {
    size: "medium",
    always: {
      wings: "feathered",
      eyeStyle: "hollow",
      eyes: "#fff3c4",
      wingColor: "#f7f3e4",
    },
    sometimes: {
      skin: [SKIN_TONES.porcelain, SKIN_TONES.honey, "#f0e0c0"],
      headgear: ["circlet", "none", "helm"],
      hair: ["long", "short", "bald"],
      hairColor: ["#e8b04a", "#f2efe6"],
      prop: ["sword", "hammer", "none"],
      outfit: ["#f2efe6", "#d9a441"],
      glow: ["#fff0b3", "#ffe8a3"],
    },
  },
  construct: {
    size: "medium",
    always: {
      eyeStyle: "beady",
      hair: "bald",
      mouth: "smirk",
      emblem: "gear",
      // Plates, which the scale hide draws: a construct with bare skin gets
      // the blush a hero's face carries, and a golem does not blush.
      hide: "scales",
      hideColor: "#6a747e",
    },
    sometimes: {
      skin: ["#9aa4ae", "#a89070", "#7e8a94"],
      eyes: ["#4ec3e0", "#f4c542", "#ff9a3c"],
      headgear: ["helm", "goggles", "none"],
      prop: ["hammer", "wrench", "fists"],
      outfit: ["#5a6a7a", "#6b5a44"],
      glow: ["#b8e0f0", "#d8cfc0"],
    },
  },
  elemental: {
    size: "large",
    always: {
      build: "hulking",
      mouth: "gape",
      eyeStyle: "hollow",
      hair: "bald",
    },
    sometimes: {
      skin: ["#e0602f", "#3a6fb0", "#8a7a62", "#cfe0f0"],
      eyes: ["#ffe066", "#8fd3e8"],
      hide: ["scales", "warts"],
      prop: ["flame", "fists", "none"],
      outfit: ["#a83a1f", "#2f5d8a", "#6b5a44"],
      glow: ["#ffb38a", "#b8e0f0", "#d8cfc0"],
    },
  },
  ooze: {
    size: "large",
    always: {
      build: "hulking",
      hide: "warts",
      eyeStyle: "beady",
      mouth: "gape",
      hair: "bald",
      prop: "none",
    },
    sometimes: {
      skin: [MONSTER_TONES.oozeViolet, "#6a9c5a", "#a8a03a", "#4a6a7a"],
      eyes: ["#2b2233", "#f4c542"],
      outfit: ["#5a4480", "#4a7a46", "#7a7430"],
      glow: ["#c9b8ff", "#cfe0b8"],
    },
  },
  plant: {
    size: "large",
    always: {
      build: "hulking",
      headgear: "leaves",
      prop: "vine",
      hair: "bald",
    },
    sometimes: {
      skin: ["#6a8f3a", "#7a6a3a", "#5f7a4a"],
      hide: ["warts", "fur", "none"],
      eyes: ["#f4c542", "#3a6a2a"],
      outfit: ["#5a4a2a", "#4a5a2a"],
      glow: ["#cfe8a0", "#d8cfa0"],
    },
  },
  aberration: {
    size: "large",
    always: {
      eyeStyle: "hollow",
      muzzle: "maw",
      hide: "warts",
      hair: "bald",
      tail: "reptile",
      prop: "none",
    },
    sometimes: {
      skin: [MONSTER_TONES.oozeViolet, "#4a4458", "#7a4a6a"],
      eyes: ["#8fd3e8", "#f4c542"],
      build: ["hunched", "sinuous"],
      outfit: ["#3b2a4a", "#2f3a4a"],
      glow: ["#c9b8ff", "#a8c0d8"],
    },
  },
  monstrosity: {
    // The fallback family, so the safe size rather than the dramatic one: a
    // creature nobody could identify takes one square until something says
    // otherwise, and a token drawn too small is a smaller lie on the board
    // than one that swallows four squares it has no claim to.
    size: "medium",
    always: {
      build: "hunched",
      hide: "scales",
      eyeStyle: "slit",
      tail: "reptile",
      hair: "bald",
      prop: "claws",
    },
    sometimes: {
      skin: ["#7a8f5a", "#8a6242", "#6a7a8a", MONSTER_TONES.trollStone],
      muzzle: ["snout", "maw"],
      eyes: ["#f4c542", "#e0602f"],
      outfit: ["#6b5a44", "#5a6a4a"],
      glow: ["#cfd8b8", "#d8c8b8"],
    },
  },
} satisfies Record<string, CreatureKind>;

export type FamilyName = keyof typeof FAMILIES;

/** The creatures a low-level dungeon is made of, plus the big ones a table
 * asks for by name. Each is a difference from its family, never a fresh
 * start: that is what keeps a gnoll and a wolf looking like they came from
 * the same hand. */
export const CREATURES = {
  goblin: {
    family: "humanoid",
    size: "small",
    always: {
      ears: "pointed",
      mouth: "snarl",
      hide: "warts",
      eyeStyle: "beady",
      hair: "bald",
      build: "hunched",
    },
    sometimes: {
      skin: GREENS,
      eyes: ["#f4c542", "#e0602f"],
      headgear: ["none", "hood", "cap", "helm"],
      prop: ["dagger", "club", "bow", "sword"],
      outfit: ["#6b4a2f", "#4a5a3a", "#7a3b2f"],
      glow: ["#cfe0a8", "#d8c8a0"],
    },
  },
  hobgoblin: {
    family: "humanoid",
    always: {
      ears: "pointed",
      mouth: "snarl",
      eyeStyle: "beady",
      hair: "spiky",
      hairColor: "#2b2233",
      headgear: "helm",
    },
    sometimes: {
      skin: ["#c9673c", "#b8563c", "#d97a4a"],
      prop: ["sword", "bow", "axe"],
      outfit: ["#7a2f2f", "#5a3a2f"],
      glow: ["#f0c0a0"],
    },
  },
  bugbear: {
    family: "humanoid",
    always: {
      build: "hunched",
      hide: "fur",
      ears: "pointed",
      mouth: "snarl",
      hair: "bald",
      tail: "tuft",
    },
    sometimes: {
      skin: ["#8a7a4a", "#9a8a5a", "#7a6a3a"],
      hideColor: ["#5a4a2a", "#6a5a3a"],
      prop: ["club", "axe", "dagger"],
      outfit: ["#5a4434", "#6a5240"],
      glow: ["#d8cfa0"],
    },
  },
  kobold: {
    family: "humanoid",
    size: "small",
    always: {
      build: "hunched",
      muzzle: "snout",
      hide: "scales",
      eyeStyle: "slit",
      hair: "bald",
      headgear: "crest",
      ears: "pointed",
    },
    sometimes: {
      skin: ["#b8563c", "#c97a3c", "#8a4a3a"],
      headgearColor: ["#8a3b2f", "#a8642f"],
      prop: ["dagger", "club", "bow"],
      outfit: ["#6b4a2f", "#7a5a3a"],
      glow: ["#f0c8a0"],
    },
  },
  gnoll: {
    family: "beast",
    always: {
      build: "upright",
      headgear: "none",
      mouth: "snarl",
      tail: "tuft",
      prop: "axe",
    },
    sometimes: {
      skin: ["#b8a068", "#a8905a", "#c9b078"],
      hideColor: ["#6a5a2a", "#7a6a3a"],
      outfit: ["#6b4a2f", "#7a3b2f"],
      prop: ["axe", "bow", "club"],
      glow: ["#e0d0a0"],
    },
  },
  orc: {
    family: "humanoid",
    always: {
      tusks: true,
      mouth: "snarl",
      ears: "pointed",
      eyes: "#7a3b1f",
      build: "hulking",
    },
    sometimes: {
      skin: GREENS,
      hair: ["bald", "spiky", "braids"],
      hairColor: ["#2b2233", "#3b2a1f"],
      headgear: ["none", "horned", "helm", "headband"],
      prop: ["axe", "club", "sword"],
      outfit: ["#8a4a2f", "#6b4a2f", "#5a4a3a"],
      glow: ["#e6b88f", "#cfd8a8"],
    },
  },
  ogre: {
    family: "giant",
    always: { tusks: true, mouth: "grin", hide: "warts", beard: false },
    sometimes: {
      skin: [MONSTER_TONES.ogreClay, "#bf8a52", "#a8764a"],
      hideColor: ["#8f5f3a", "#7a5a3a"],
      hair: ["bald", "spiky"],
      hairColor: ["#3b2a1f", "#6a5230"],
      outfit: ["#8a6a4a", "#7a5a3a"],
      glow: ["#e6c8a0"],
    },
  },
  troll: {
    family: "giant",
    always: {
      build: "hunched",
      hide: "warts",
      ears: "pointed",
      mouth: "snarl",
      prop: "claws",
      beard: false,
      tusks: true,
    },
    sometimes: {
      skin: [MONSTER_TONES.trollStone, "#7a9a68", "#9aae88"],
      hair: ["long", "spiky", "bald"],
      hairColor: ["#c9a96e", "#8a7a4a"],
      hideColor: ["#5f7a4a", "#6a8f5a"],
      outfit: ["#6a7a5a", "#5a6a4a"],
      glow: ["#cfe0b8"],
    },
  },
  skeleton: {
    family: "undead",
    always: { hide: "bone", skin: MONSTER_TONES.boneWhite, headgear: "none" },
    sometimes: {
      eyes: ["#8fd3e8", "#e0602f"],
      prop: ["sword", "bow", "dagger"],
      outfit: ["#5a5346", "#4a4458"],
      glow: ["#b8bcc0"],
    },
  },
  zombie: {
    family: "undead",
    always: {
      build: "hunched",
      hide: "warts",
      mouth: "gape",
      skin: MONSTER_TONES.graveGrey,
      prop: "none",
    },
    sometimes: {
      hideColor: ["#6a7a5a", "#7a6a5a"],
      eyes: ["#d9d2c5", "#8fd3e8"],
      outfit: ["#5a5346", "#4a4a3a"],
      glow: ["#9aa79a"],
    },
  },
  ghoul: {
    family: "undead",
    always: {
      build: "hunched",
      muzzle: "maw",
      prop: "claws",
      ears: "pointed",
      hide: "warts",
    },
    sometimes: {
      skin: ["#b9b8a4", "#a8a890", "#c0b8a0"],
      eyes: ["#e0602f", "#f4c542"],
      outfit: ["#4a4458", "#3d4a46"],
      glow: ["#a8a0c0"],
    },
  },
  wolf: {
    family: "beast",
    always: { mouth: "snarl", tail: "tuft" },
    sometimes: {
      skin: ["#6b5a4a", "#8a8078", "#4a4038"],
      hideColor: ["#4a4038", "#3a3430", "#9a9088"],
      eyes: ["#e0a63c", "#8fd3e8"],
      outfit: ["#5f5145", "#6a6058"],
      glow: ["#cfd8e0"],
    },
  },
  bear: {
    family: "beast",
    size: "large",
    always: { build: "hulking", mouth: "gape", muzzle: "snout" },
    sometimes: {
      skin: [MONSTER_TONES.beastBrown, "#5a4a3a", "#2b2233"],
      hideColor: ["#6a5240", "#3a3028"],
      outfit: ["#7a5c40", "#4a4038"],
      glow: ["#d8cfb8"],
    },
  },
  owlbear: {
    family: "monstrosity",
    always: {
      build: "hulking",
      muzzle: "beak",
      hide: "fur",
      eyeStyle: "beady",
      prop: "claws",
      tail: "none",
    },
    sometimes: {
      skin: ["#8a6242", "#7a5a42", "#9a7a5a"],
      hideColor: ["#5a4030", "#6a5240"],
      eyes: ["#f4c542"],
      outfit: ["#6b5a44"],
      glow: ["#d8c8a8"],
    },
  },
  minotaur: {
    family: "humanoid",
    size: "large",
    always: {
      build: "hulking",
      muzzle: "snout",
      hide: "fur",
      headgear: "horned",
      mouth: "snarl",
      prop: "axe",
      tail: "tuft",
      hair: "bald",
    },
    sometimes: {
      skin: ["#8a6242", "#6b5a4a", "#a8825a"],
      hideColor: ["#5a4030", "#4a4038"],
      eyes: ["#e0602f", "#f4c542"],
      outfit: ["#7a4a2f", "#5a4434"],
      glow: ["#d8c0a0"],
    },
  },
  dragon: { family: "dragon" },
  drake: { family: "dragon", size: "medium", always: { wings: "none" } },
  wyvern: { family: "dragon", always: { headgear: "horned", tail: "spiked" } },
  lich: {
    family: "undead",
    always: {
      headgear: "boneCrown",
      prop: "staff",
      hide: "bone",
      skin: MONSTER_TONES.boneWhite,
    },
    sometimes: {
      eyes: ["#7bb04a", "#8fd3e8"],
      outfit: ["#3b2a4a", "#2f3a4a"],
      glow: ["#a8ffd0", "#c9b8ff"],
    },
  },
  imp: {
    family: "fiend",
    size: "tiny",
    always: { wings: "bat", prop: "none" },
  },
  demon: { family: "fiend", size: "large", always: { build: "hulking" } },
  devil: { family: "fiend", always: { headgear: "horned", prop: "flame" } },
  golem: {
    family: "construct",
    size: "large",
    always: { build: "hulking", prop: "fists" },
  },
  treant: {
    family: "plant",
    size: "huge",
    always: { beard: true, beardColor: "#6a8f3a", mouth: "gape" },
  },
  elemental: { family: "elemental" },
  spider: {
    family: "beast",
    always: {
      muzzle: "none",
      mouth: "snarl",
      tusks: true,
      hide: "fur",
      tail: "none",
      ears: "round",
      eyeStyle: "beady",
    },
    sometimes: {
      skin: ["#3a3430", "#4a3a30", "#2b2233"],
      hideColor: ["#6a5a4a", "#7a3b2f"],
      eyes: ["#c0392b", "#f4c542"],
      outfit: ["#3a3430", "#4a4038"],
      glow: ["#a8a0b8"],
    },
  },
} satisfies Record<string, CreatureEntry>;

export type CreatureName = keyof typeof CREATURES;

/** Words in a name that say what colour the thing is. A book names half its
 * monsters by their hide — "Red Dragon", "Frost Giant", "Black Bear" — and
 * that is free information no family table could hold, so it is read off the
 * name and applied over whatever the family rolled. */
export const TINTS: Record<string, string> = {
  red: "#c0392b",
  green: MONSTER_TONES.dragonEmerald,
  blue: MONSTER_TONES.dragonSapphire,
  black: "#3a3340",
  white: "#e2e8ee",
  gold: "#e0b840",
  golden: "#e0b840",
  silver: "#c8d0d8",
  bronze: "#a9743a",
  brass: "#c39a3c",
  copper: "#b5651d",
  grey: "#8f959c",
  gray: "#8f959c",
  brown: MONSTER_TONES.beastBrown,
  frost: "#9fd8ee",
  ice: "#9fd8ee",
  fire: "#e0602f",
  flame: "#e0602f",
  shadow: "#4a4458",
  stone: MONSTER_TONES.trollStone,
  bone: MONSTER_TONES.boneWhite,
  crimson: "#a8272f",
  emerald: MONSTER_TONES.dragonEmerald,
};
