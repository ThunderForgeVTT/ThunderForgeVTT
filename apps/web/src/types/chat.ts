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
}
