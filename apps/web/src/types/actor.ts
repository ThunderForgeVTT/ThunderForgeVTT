export type ActorPermissionLevel = "VIEWER" | "EDITOR" | "OWNER";

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
  myPermissionLevel: ActorPermissionLevel;
  createdAt: string;
  updatedAt: string;
};

export type ActorPermissionRecord = {
  actorId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};
