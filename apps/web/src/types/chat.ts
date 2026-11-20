/** One persisted chat message (`world_chat_messages`). */
export interface ChatMessageRecord {
  id: string;
  worldId: string;
  sceneId: string | null;
  authorUserId: string;
  /** Display name captured at send time, so history survives a rename. */
  authorLabel: string;
  body: string;
  /** True only on messages the server decided this viewer may see. */
  gmOnly: boolean;
  createdAt: string;
  /** Spec 103: the token the line was said as; null when said as the author. */
  speakerTokenId?: string | null;
  /** Spec 103: that token's name, or "Someone" where this viewer may not read it. */
  speakerLabel?: string | null;
}
