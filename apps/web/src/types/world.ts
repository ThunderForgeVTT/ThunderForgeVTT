export interface WorldRecord {
  id: string;
  name: string;
  description: string | null;
  gameSystemId: string | null;
  interfacePackId: string | null;
  scenes: string[];
  actors: string[];
  tokens: string[];
  events: string[];
  gameSystem: string | null;
  interfacePack: string | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
  sessionNotes: string | null;
  /** Spec 017 (FR-007): gates the Actor Selection screen's
   * "create your own character" option. */
  allowPlayerCreatedActors: boolean;
  /** Spec 020 (FR-003): when true, Genie Session Resource holdings carry
   * over into the next session instead of resetting to 0. */
  genieResourceCarryoverEnabled: boolean;
  /** Spec 022 (FR-014/FR-015): default grid type ("square" | "hex" |
   * "gridless") applied to a newly created scene when its own gridType
   * isn't explicitly set. */
  defaultSceneGridType: string;
  /** Spec 022 (FR-002a/FR-002b, ADR-046): the world's server-authoritative
   * currently-launched scene for Play. Null = nothing launched yet. */
  activeSceneId: string | null;
}

/**
 * A person's standing in one world, as `world_members.role` stores it.
 *
 * The one role type in the app. It mirrors `thunderforge_authz::Role` on the
 * server, and the list below is in the same order, lowest first: the order
 * *is* the ranking, so a comparison of two roles is a comparison of their
 * positions here and never a chain of string equalities.
 *
 * ADR-099 added `TrustedPlayer` between `Player` and `GM`. Rank kept every
 * "at least a Game Master" gate correct without touching it. What rank could
 * not keep correct was a check for *equality* with `"Player"`, which is why
 * callers ask the predicates below rather than comparing strings: a Trusted
 * Player is a Player everywhere except the book list and adoption, and a
 * component that wrote `role === "Player"` would silently disagree.
 */
export const WORLD_ROLES = ["Player", "TrustedPlayer", "GM", "Owner"] as const;

export type WorldMemberRole = (typeof WORLD_ROLES)[number];

/** Whether a string from the server is a role this build understands.
 * Anything else is treated as no role at all, as the server treats it. */
export function isWorldMemberRole(role: string): role is WorldMemberRole {
  return (WORLD_ROLES as readonly string[]).includes(role);
}

/** Position in the ranking; higher outranks lower. No role ranks below all. */
export function roleRank(role: WorldMemberRole | null): number {
  return role === null ? -1 : WORLD_ROLES.indexOf(role);
}

/** Owner or Game Master: the people who run the table. A Trusted Player does
 * not, and is hidden from everything this gates. */
export function runsTheWorld(role: WorldMemberRole | null): boolean {
  return roleRank(role) >= roleRank("GM");
}

/** Owner, Game Master or Trusted Player: who may arrange the world's book
 * material (spec 050 decision 8). Nothing else — see `runsTheWorld`. */
export function managesContent(role: WorldMemberRole | null): boolean {
  return roleRank(role) >= roleRank("TrustedPlayer");
}

/** What a person reads for a role. The stored spellings are not words. */
export function roleLabel(role: WorldMemberRole): string {
  switch (role) {
    case "Owner":
      return "Owner";
    case "GM":
      return "Game Master";
    case "TrustedPlayer":
      return "Trusted Player";
    case "Player":
      return "Player";
  }
}

/** One entry in `myWorldsWithRole` — a world the caller owns or is an
 * accepted member of, paired with their role. Left a string because it is
 * the server's raw value; read it through `isWorldMemberRole`. */
export interface MyWorldEntry {
  world: WorldRecord;
  role: string;
}

export interface CreateWorldInput {
  name: string;
  description?: string;
  gameSystemId?: string | null;
  interfacePackId?: string | null;
}

export interface DeleteWorldResult {
  id: string;
  status: string;
  message: string;
}
