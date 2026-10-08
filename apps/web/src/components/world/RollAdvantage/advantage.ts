/** Spec 084: how a d20 test is rolled. Matches the GraphQL `Advantage` enum. */
export type Advantage = "NORMAL" | "ADVANTAGE" | "DISADVANTAGE";

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
