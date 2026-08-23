export type ActorPermissionLevel = "VIEWER" | "EDITOR" | "OWNER";

export type LoreLinkSourceRecord = {
  id: string;
  title: string;
  slug: string;
};

export type WorldActorRecord = {
  id: string;
  worldId: string;
  sceneId: string;
  actorType: string;
  gameSystemId: string | null;
  label: string;
  description: string | null;
  isPublic: boolean;
  isNpc: boolean;
  createdBy: string;
  ownedBy: string;
  myPermissionLevel: ActorPermissionLevel;
  createdAt: string;
  updatedAt: string;
  /** Spec 012 (FR-006): every lore entry whose body currently links here. */
  loreLinkedFrom: LoreLinkSourceRecord[];
};

export type ActorPermissionRecord = {
  actorId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};
