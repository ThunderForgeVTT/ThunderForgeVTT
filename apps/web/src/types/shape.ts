export type ShapeKind = "STROKE" | "RECT" | "ELLIPSE" | "LINE" | "TEXT";

export interface ShapeRecord {
  shapeId: string;
  sceneId: string;
  /** The level of the scene this stands on. */
  levelId: string;
  kind: ShapeKind;
  geometry: Record<string, unknown>;
  text: string | null;
  style: Record<string, unknown> | null;
  visibleToPlayers: boolean;
  metadata: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}

export interface CreateShapeInput {
  sceneId: string;
  /** Omitted, the level this browser is showing; failing that, the entry level. */
  levelId?: string;
  kind: ShapeKind;
  geometry: Record<string, unknown>;
  text?: string;
  style?: Record<string, unknown>;
  visibleToPlayers?: boolean;
  metadata?: Record<string, unknown>;
}

export interface UpdateShapeInput {
  geometry?: Record<string, unknown>;
  text?: string;
  style?: Record<string, unknown>;
  visibleToPlayers?: boolean;
  metadata?: Record<string, unknown>;
}

/** Someone who drew on a scene and does not run its world (spec 082 US4). */
export interface ShapeCreator {
  userId: string;
  /** Their username, as the grants card shows it. */
  displayName: string;
  /** False once they left or were removed from the world. */
  isMember: boolean;
  /** Their shapes on the scene, every level. */
  shapeCount: number;
}
