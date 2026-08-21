export interface LightRecord {
  lightId: string;
  sceneId: string;
  x: number;
  y: number;
  radius: number;
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
  intensity?: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows?: boolean;
  metadata?: Record<string, unknown>;
}

export interface UpdateLightInput {
  x?: number;
  y?: number;
  radius?: number;
  intensity?: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows?: boolean;
  metadata?: Record<string, unknown>;
}
