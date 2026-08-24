// Spec 014: TS mirrors of contracts/graphql-roll.md's wire shapes.

export type DieSidesKind = "NUMERIC" | "FATE" | "COIN";

export interface DieOutcomeRecord {
  sidesKind: DieSidesKind;
  /** Set iff sidesKind === "NUMERIC" (e.g. 20 for a d20). */
  numericSides: number | null;
  /** Full chain: original roll + every reroll/explosion of this die. */
  rolls: number[];
  kept: boolean;
  finalValue: number;
}

export type RollResultKind = "TOTAL" | "SUCCESS_COUNT";

export interface RollResolutionRecord {
  formula: string;
  dice: DieOutcomeRecord[];
  resultKind: RollResultKind;
  resultValue: number;
}

export interface RollRecordRecord {
  id: string;
  worldId: string;
  triggeredBy: string;
  resolution: RollResolutionRecord;
  createdAt: string;
}

export interface PlaceholderBinding {
  name: string;
  value: number;
}
