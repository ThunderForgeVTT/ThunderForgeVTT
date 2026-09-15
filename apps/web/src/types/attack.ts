// Spec 046: TS mirrors of contracts/fight.md's attack and offer shapes.
//
// Everything here arrives already built for this viewer. A party the server
// redacted is `{ tokenId: null, label: "Unknown" }`, and this client must not
// try to recover it from anywhere else — the point of redaction is that it
// has nothing to recover it from.

import type { HitPointChange } from "@/types/combat";
import type { RollResolutionRecord } from "@/types/roll";

export type ActionCost =
  | "ACTION"
  | "BONUS_ACTION"
  | "REACTION"
  | "LEGENDARY"
  | "FREE";

export type AttackOutcome = "HIT" | "MISS" | "NO_DEFENCE" | "NO_TARGET";

export type AttackFlag =
  | "OUT_OF_REACH"
  | "LONG_RANGE"
  | "BEYOND_RANGE"
  | "NO_LINE_OF_SIGHT"
  | "NO_REACH_DECLARED"
  | "OVERSPENT"
  | "LEGENDARY_ON_OWN_TURN";

export type OfferStatus = "PENDING" | "TAKEN" | "DECLINED" | "APPLIED";

/** One side of an attack. `tokenId` null and `label` "Unknown" when redacted. */
export interface AttackPartyRecord {
  tokenId: string | null;
  label: string;
}

export interface OfferRecord {
  id: string;
  sceneId: string;
  attackId: string | null;
  kind: HitPointChange;
  amount: number;
  target: AttackPartyRecord;
  status: OfferStatus;
  /** A name, or "Game Master" when resolved on a player's behalf. */
  resolvedBy: string | null;
  resolvedOnBehalf: boolean;
  /** Whether this viewer may take or decline it. */
  mayResolve: boolean;
  createdAt: string;
}

export interface AttackRecord {
  id: string;
  sceneId: string;
  attacker: AttackPartyRecord;
  target: AttackPartyRecord | null;
  /** Null when the attacker is redacted. */
  abilityName: string | null;
  toHit: RollResolutionRecord;
  damage: RollResolutionRecord | null;
  /** Null when the target has none, or is redacted. */
  defence: number | null;
  outcome: AttackOutcome;
  distance: number | null;
  flags: AttackFlag[];
  actionCost: ActionCost;
  offer: OfferRecord | null;
  multiattackOf: string | null;
  createdAt: string;
}

export interface TurnCheckRecord {
  allowed: boolean;
  activeLabel: string | null;
}

export interface AttackPreviewRecord {
  distance: number | null;
  flags: AttackFlag[];
  turn: TurnCheckRecord;
}

export interface AttackInput {
  attackerTokenId: string;
  abilityId?: string | null;
  itemId?: string | null;
  targetTokenId?: string | null;
  targets?: string[] | null;
  actionCost?: ActionCost | null;
}

/** What an ability or item is as an attack (research R2). */
export interface AttackFields {
  reach: number | null;
  rangeNormal: number | null;
  rangeLong: number | null;
  needsLineOfSight: boolean;
  actionCost: ActionCost;
  legendaryCost: number;
  multiattack: string[];
}
