// Play-view Chat (src/server/src/graphql/mutations_chat.rs).
//
// GM-only messages are filtered server-side, so anything this module
// receives is already safe to render for the current viewer — there is no
// client-side visibility rule to keep in sync with the server's.

import { postGraphQL } from "@/api/graphqlClient";
import type { ChatMessageRecord } from "@/types/chat";

const CHAT_MESSAGE_FIELDS = `
  id
  worldId
  sceneId
  authorUserId
  authorLabel
  body
  gmOnly
  createdAt
`;

type WorldChatMessagesQuery = { worldChatMessages: ChatMessageRecord[] };
type SendChatMessageMutation = { sendChatMessage: ChatMessageRecord };

/** This world's backscroll, oldest-first (the server orders it). */
export function getWorldChatMessages(
  worldId: string,
  limit?: number,
): Promise<ChatMessageRecord[]> {
  return postGraphQL<WorldChatMessagesQuery>(
    `
      query WorldChatMessages($worldId: UUID!, $limit: Int) {
        worldChatMessages(worldId: $worldId, limit: $limit) {
          ${CHAT_MESSAGE_FIELDS}
        }
      }
    `,
    { worldId, limit },
  ).then((data) => data.worldChatMessages);
}

export function sendChatMessage(input: {
  worldId: string;
  sceneId?: string | null;
  body: string;
  gmOnly?: boolean;
}): Promise<ChatMessageRecord> {
  return postGraphQL<SendChatMessageMutation>(
    `
      mutation SendChatMessage($input: SendChatMessageInput!) {
        sendChatMessage(input: $input) {
          ${CHAT_MESSAGE_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.sendChatMessage);
}
