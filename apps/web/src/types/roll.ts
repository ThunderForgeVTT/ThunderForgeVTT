// Spec 014: TS mirrors of contracts/graphql-roll.md's wire shapes.

import type { RollDiscrepancyRecord } from "@/components/world/rollDiscrepancy";

export type DieSidesKind = "NUMERIC" | "FATE" | "COIN";

export interface DieOutcomeRecord {
  sidesKind: DieSidesKind;
  /** Set iff sidesKind === "NUMERIC" (e.g. 20 for a d20). */
  numericSides: number | null;
  /** Full chain: original roll + every reroll/explosion of this die. */
  rolls: number[];
  /**
   * Spec 083: why each value after the first was rolled; `steps[i]` explains
   * `rolls[i + 1]`. Empty on a roll stored before spec 083.
   */
  steps: DieStep[];
  kept: boolean;
  finalValue: number;
}

export type DieStep = "REROLL" | "EXPLODE";

export type RollResultKind = "TOTAL" | "SUCCESS_COUNT";

export interface RollResolutionRecord {
  formula: string;
  dice: DieOutcomeRecord[];
  resultKind: RollResultKind;
  resultValue: number;
  /**
   * Spec 028 (FR-064): set only where the server independently determined a
   * different value for a result a client reported. Absent for every ordinary
   * roll, and absent wherever the server had no basis to compare (FR-068).
   */
  discrepancy?: RollDiscrepancyRecord | null;
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

/** Spec 083: a value the server put in for one of the formula's placeholders. */
export interface RollBindingRecord {
  placeholder: string;
  value: number;
}

/** Spec 081: who sees a roll. Players may pick `GM_EYES`, the GM `GM_ONLY`. */
export type RollVisibility = "EVERYONE" | "GM_EYES" | "GM_ONLY";

/** Spec 081: a roll the viewer may see whole. */
export interface WorldRollRecord {
  __typename: "WorldRoll";
  id: string;
  rollerId: string;
  rollerName: string;
  label: string | null;
  formula: string;
  /** Spec 083: sorted by placeholder; empty when the formula has none. */
  bindings: RollBindingRecord[];
  resolution: RollResolutionRecord;
  visibility: RollVisibility;
  createdAt: string;
  revealedAt: string | null;
  revealedByName: string | null;
  /** Spec 084: the facets that shaped this roll, such as Advantage. */
  facets: RollFacetRecord[];
}

/** Spec 084: a facet a roll carries, named as its system names it. */
export interface RollFacetRecord {
  id: string;
  label: string;
}

/**
 * Spec 081: a roll made for the GM's eyes, as another player sees it. It has
 * no field for the dice, the formula or the label — there is nothing in it to
 * leak.
 */
export interface MaskedRollRecord {
  __typename: "MaskedRoll";
  id: string;
  rollerName: string;
  createdAt: string;
  visibility: RollVisibility;
}

export type WorldRollEntry = WorldRollRecord | MaskedRollRecord;

/** Spec 084: how a d20 test is rolled. Matches the GraphQL `Advantage` enum. */
export type Advantage = "NORMAL" | "ADVANTAGE" | "DISADVANTAGE";
