import type { Advantage } from "@/types/roll";

export type { Advantage };

export const ADVANTAGE_CHOICES: readonly Advantage[] = [
  "NORMAL",
  "ADVANTAGE",
  "DISADVANTAGE",
];

export const ADVANTAGE_LABEL: Record<Advantage, string> = {
  NORMAL: "Normal",
  ADVANTAGE: "Advantage",
  DISADVANTAGE: "Disadvantage",
};
