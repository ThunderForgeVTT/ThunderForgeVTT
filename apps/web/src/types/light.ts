export interface LightRecord {
  lightId: string;
  sceneId: string;
  x: number;
  y: number;
  /** How far the light reaches at all — its dim reach — in world units. */
  radius: number;
  /** How far it is bright, in world units; never beyond `radius`. */
  brightRadius: number;
  intensity: number;
  color: string | null;
  attachedTokenId: string | null;
  castsShadows: boolean;
  metadata: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}

export interface CreateLightInput {
  sceneId: string;
  x: number;
  y: number;
  radius: number;
  /** Omitted, half of `radius` (spec 045 FR-062). */
  brightRadius?: number;
  intensity?: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows?: boolean;
  metadata?: Record<string, unknown>;
}

/**
 * What a scene's distances are measured in (`sceneUnits`): one grid square in
 * the system's units, what they are called, and one square in world units.
 */
export interface SceneUnits {
  perCell: number;
  label: string;
  gridSize: number;
}

export interface UpdateLightInput {
  x?: number;
  y?: number;
  radius?: number;
  brightRadius?: number;
  intensity?: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows?: boolean;
  metadata?: Record<string, unknown>;
}
