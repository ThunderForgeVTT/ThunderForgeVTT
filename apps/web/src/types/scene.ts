export type SceneRecord = {
  sceneId: string;
  worldId: string;
  name: string;
  description: string | null;
  type: string;
  gridSize: number;
  gridType: string;
  width: number;
  height: number;
  backgroundImagePath: string | null;
  ownerId: string;
  createdAt: string;
  updatedAt: string;
};
