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

/** One entry in `myWorldsWithRole` — a world the caller owns or is an
 * accepted member of, paired with their role ("Owner" | "GM" | "Player"). */
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
