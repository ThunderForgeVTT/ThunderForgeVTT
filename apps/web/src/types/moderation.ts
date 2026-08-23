export type ModerationEntityType = "WORLD_ACTOR" | "WORLD_ITEM" | "WORLD_LORE_ENTRY";

export type ModerationActionType =
  | "NOTICE_RECEIVED"
  | "NOTICE_REJECTED_INCOMPLETE"
  | "CONTENT_DISABLED"
  | "COUNTER_NOTICE_RECEIVED"
  | "COUNTER_NOTICE_FORWARDED"
  | "CONTENT_RESTORED"
  | "CONTENT_REMAINS_DISABLED";

export interface ModerationActionRecord {
  id: string;
  caseId: string;
  actionType: ModerationActionType;
  entityType: ModerationEntityType;
  entityId: string;
  worldId: string;
  validityResult: string | null;
  missingElements: string[] | null;
  restorationDueAt: string | null;
  createdAt: string;
}

export interface ModerationCaseRecord {
  caseId: string;
  entityType: ModerationEntityType;
  entityId: string;
  worldId: string;
  currentStatus: ModerationActionType;
  events: ModerationActionRecord[];
}

export interface SubmitTakedownNoticeInput {
  entityType: ModerationEntityType;
  entityId: string;
  claimantName: string;
  claimantContact: string;
  copyrightedWorkDescription: string;
  infringingMaterialLocation: string;
  goodFaithStatement: boolean;
  accuracyStatement: boolean;
  signature: string;
}

export interface SubmitCounterNoticeInput {
  caseId: string;
  removedMaterialDescription: string;
  goodFaithMistakeStatement: boolean;
  consentToJurisdiction: boolean;
  contactInformation: string;
  signature: string;
}
