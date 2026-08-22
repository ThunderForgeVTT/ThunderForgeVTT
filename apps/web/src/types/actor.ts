export type WorldActorRecord = {
  id: string;
  worldId: string;
  sceneId: string;
  actorType: string;
  gameSystemId: string | null;
  label: string;
  isPublic: boolean;
  isNpc: boolean;
  createdBy: string;
  ownedBy: string;
  createdAt: string;
  updatedAt: string;
};
