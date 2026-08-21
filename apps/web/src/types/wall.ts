export type DoorState = "NONE" | "OPEN" | "CLOSED";

export interface WallRecord {
  wallId: string;
  sceneId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: DoorState;
  metadata: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}

export interface CreateWallInput {
  sceneId: string;
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
