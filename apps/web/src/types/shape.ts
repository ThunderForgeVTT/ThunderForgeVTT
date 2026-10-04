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
