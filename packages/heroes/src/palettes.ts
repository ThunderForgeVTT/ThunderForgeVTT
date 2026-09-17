/**
 * The swatches a builder offers for each colour field, and the only colours
 * the dice pick from. Any `#rrggbb` is still a valid colour; a palette is a
 * starting point, not a rule.
 */
import { HERO_COLORS, MONSTER_TONES, SKIN_TONES } from "./spec.ts";

const METALS = ["#b8c2cc", "#8a8f98", "#d4a441", "#b87333", "#4a4a55"];
const CLOTH = [
  "#3d6fd1",
  "#6a4bc4",
  "#2f5d46",
  "#8b2f3a",
  "#c77d2e",
  "#4a4a55",
  "#e8e2d0",
  "#2b5d8f",
  "#7a8f3a",
];
const HAIR = [
  "#2b2233",
  "#3b2a1f",
  "#6b4226",
  "#c98a3c",
  "#e8c872",
  "#b0413e",
  "#d8d8d8",
  "#f2f2f2",
];

export const HERO_PALETTES: Record<
  (typeof HERO_COLORS)[number],
  readonly string[]
> = {
  skin: Object.values(SKIN_TONES),
  eyes: ["#3b2a1f", "#5a3bb0", "#2f6f9f", "#3f8f4f", "#8a6242", "#c0392b"],
  hairColor: HAIR,
  beardColor: HAIR,
  headgearColor: [...METALS, ...CLOTH],
  accent: ["#f4c542", "#d94a4a", "#4ec3e0", "#7bd88f", "#e07bd4"],
  outfit: CLOTH,
  trim: ["#2b2233", "#d4a441", "#b8c2cc", "#6b4226", "#e8e2d0"],
  glow: ["#c9d6e8", "#9fc3ff", "#c9b8ff", "#ffd59f", "#b8e8c0", "#ffb8b8"],
  ring: [...CLOTH.slice(0, 6), "#d4a441"],
  hideColor: Object.values(MONSTER_TONES),
  wingColor: [
    MONSTER_TONES.dragonScarlet,
    MONSTER_TONES.dragonEmerald,
    MONSTER_TONES.dragonSapphire,
    MONSTER_TONES.beastBrown,
    "#4a4a55",
    "#e8e2d0",
  ],
};
