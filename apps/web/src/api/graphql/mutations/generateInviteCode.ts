import { gql } from "@apollo/client";

/**
 * GraphQL mutation to generate a new invite code for a world.
 * 
 * Only callable by world Owner or GM.
 * Server automatically assigns created_by from auth context.
 * 
 * Response contains the generated invite code and metadata.
 * This mutation triggers a NOTIFY event which is broadcast to all
 * connected clients via the worldEventCreated subscription.
 */
export const GENERATE_INVITE_CODE = gql`
  mutation generateInviteCode($worldId: ID!, $maxUses: Int!) {
    generateInviteCode(worldId: $worldId, maxUses: $maxUses) {
      id
      worldId
      inviteCode
      maxUses
      usedCount
      expiresAt
      createdBy
      createdAt
      updatedAt
    }
  }
`;
