import { postGraphQL } from "@/api/graphqlClient";

/** Spec 023 (FR-004): the character a member has claimed, if any. */
export interface WorldMemberClaimedActor {
  id: string;
  label: string;
}

export interface WorldMemberRecord {
  id: string;
  userId: string;
  role: string;
  joinedAt: string;
  worldId?: string;
  createdAt?: string;
  updatedAt?: string;
  claimedActor: WorldMemberClaimedActor | null;
}

type WorldMembersQuery = {
  worldMembers: WorldMemberRecord[];
};

/**
 * Fetches a world's membership roster directly via GraphQL. RxDB has been
 * hard cut from this layer entirely (it was fatally broken app-wide — RxDB
 * 17.2.0 rejects the inline `index: true` schema shorthand used throughout,
 * error SC26 — and there was never a live GraphQL subscription transport
 * wired up client-side to make its "reactive" queries actually reactive
 * across tabs/sessions anyway). `useWorldMembers` now calls this function
 * directly. This mirrors `CampaignSettingsPanel.tsx`'s own direct-query
 * pattern for the same data.
 */
export function getWorldMembers(worldId: string): Promise<WorldMemberRecord[]> {
  return postGraphQL<WorldMembersQuery>(
    `
      query WorldMembers($worldId: ID!) {
        worldMembers(worldId: $worldId) {
          id
          worldId
          userId
          role
          joinedAt
          createdAt
          updatedAt
          claimedActor {
            id
            label
          }
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldMembers);
}

type UpdateMemberRoleMutation = {
  updateMemberRole: WorldMemberRecord;
};

/** Spec 023 (T004): promoted out of `CampaignSettingsPanel.tsx`'s inline-only call. */
export function updateMemberRole(
  worldId: string,
  userId: string,
  role: string,
): Promise<WorldMemberRecord> {
  return postGraphQL<UpdateMemberRoleMutation>(
    `
      mutation UpdateMemberRole($input: UpdateMemberRoleInput!) {
        updateMemberRole(input: $input) {
          id
          worldId
          userId
          role
          joinedAt
          createdAt
          updatedAt
          claimedActor {
            id
            label
          }
        }
      }
    `,
    { input: { worldId, userId, role } },
  ).then((data) => data.updateMemberRole);
}

type RemoveMemberMutation = {
  removeMember: boolean;
};

/** Spec 023 (T004): promoted out of `CampaignSettingsPanel.tsx`'s inline-only call. */
export function removeMember(
  worldId: string,
  userId: string,
): Promise<boolean> {
  return postGraphQL<RemoveMemberMutation>(
    `
      mutation RemoveMember($worldId: ID!, $userId: ID!) {
        removeMember(worldId: $worldId, userId: $userId)
      }
    `,
    { worldId, userId },
  ).then((data) => data.removeMember);
}
