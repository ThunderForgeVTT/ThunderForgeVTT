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
  /**
   * Spec 046 FR-016: a named individual. Its tokens are placed linked; any
   * other NPC's are placed as unlinked copies with their own hit points.
   */
  isUnique: boolean;
  /**
   * Whether players see this NPC (owner decision 2026-09-15). Hidden by
   * default. The server never sends a player a hidden NPC, so this toggle is
   * the Game Master's; a player reading `false` holds the NPC.
   */
  visibleToPlayers: boolean;
  /** Spec 044 FR-030b: the Game Master has locked this character's look. */
  artLocked: boolean;
  /**
   * Spec 044 B6: whether the caller may change this actor's portrait and
   * token, answered by the same rule `uploadActorImage` enforces — Editor,
   * or the player holding it while the world allows it and it is unlocked.
   */
  myMayChangeImagery: boolean;
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
