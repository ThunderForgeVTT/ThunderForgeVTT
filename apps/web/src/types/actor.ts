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
  /** Spec 017 (FR-004): offered on the Actor Selection screen. */
  availableForClaim: boolean;
  /** Spec 017 (FR-012): who currently has this actor claimed, if anyone. */
  claimedBy: ActorClaimMemberRecord | null;
};

export type ActorPermissionRecord = {
  actorId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};

export type ActorClaimMemberRecord = {
  id: string;
  worldId: string;
  userId: string;
  username: string;
};

export type ActorClaimRecord = {
  actorId: string;
  actor: WorldActorRecord;
  worldMemberId: string;
  claimedByUserId: string;
  claimedAt: string;
};
