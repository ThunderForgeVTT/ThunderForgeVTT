export type DoorState = "NONE" | "OPEN" | "CLOSED";

export interface WallRecord {
  wallId: string;
  sceneId: string;
  /** The level of the scene this stands on. */
  levelId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: DoorState;
  /** Spec 030: who may change the state, not the state itself. */
  locked: boolean;
  /** Spec 030: not drawn for players until revealed. */
  secret: boolean;
  metadata: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}

export interface CreateWallInput {
  sceneId: string;
  /** Omitted, the level this browser is showing; failing that, the entry level. */
  levelId?: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision?: boolean;
  blocksMovement?: boolean;
  doorState?: DoorState;
  metadata?: Record<string, unknown>;
}

export interface UpdateWallInput {
  x1?: number;
  y1?: number;
  x2?: number;
  y2?: number;
  blocksVision?: boolean;
  blocksMovement?: boolean;
  doorState?: DoorState;
  metadata?: Record<string, unknown>;
}
