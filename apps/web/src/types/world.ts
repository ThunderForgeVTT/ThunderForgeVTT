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
