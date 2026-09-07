import { postGraphQL } from "@/api/graphqlClient";
import type { InstanceAccessPolicy } from "@/types/auth";

/**
 * Spec 035 / ADR-072: the administrator's view of instance admission.
 *
 * Every call here is admin-only. The signed-out surface learns the policy from
 * `/authentication/setup/status` instead (FR-003), because it must work with
 * no session at all — and because nothing else about access may reach an
 * unauthenticated caller.
 */

export interface InstanceAccessSettings {
  policy: "OPEN" | "INVITE_ONLY" | "CLOSED";
  updatedAt: string;
  updatedBy: string | null;
}

export interface InstanceAccessEvent {
  id: string;
  eventType: string;
  occurredAt: string;
  actorUserId: string | null;
  previousPolicy: string | null;
  newPolicy: string | null;
  attemptedRoute: string | null;
  policyAtAttempt: string | null;
}

export interface InstanceInvitationRedemption {
  userId: string;
  username: string;
  redeemedAt: string;
  route: string;
}

export interface InstanceInvitation {
  id: string;
  inviteCode: string;
  maxUses: number;
  usedCount: number;
  remainingUses: number;
  state: "ACTIVE" | "REVOKED" | "EXPIRED" | "EXHAUSTED";
  expiresAt: string | null;
  note: string | null;
  createdBy: string;
  createdAt: string;
  redemptions: InstanceInvitationRedemption[];
}

const SETTINGS_FIELDS = `
  policy
  updatedAt
  updatedBy
`;

const INVITATION_FIELDS = `
  id
  inviteCode
  maxUses
  usedCount
  remainingUses
  state
  expiresAt
  note
  createdBy
  createdAt
  redemptions {
    userId
    username
    redeemedAt
    route
  }
`;

export function getInstanceAccessSettings(): Promise<InstanceAccessSettings> {
  return postGraphQL<{ instanceAccessSettings: InstanceAccessSettings }>(
    `query InstanceAccessSettings { instanceAccessSettings { ${SETTINGS_FIELDS} } }`,
    {},
  ).then((data) => data.instanceAccessSettings);
}

export function setInstanceAccessPolicy(
  policy: InstanceAccessSettings["policy"],
): Promise<InstanceAccessSettings> {
  return postGraphQL<{ setInstanceAccessPolicy: InstanceAccessSettings }>(
    `
      mutation SetInstanceAccessPolicy($policy: InstanceAccessPolicy!) {
        setInstanceAccessPolicy(policy: $policy) { ${SETTINGS_FIELDS} }
      }
    `,
    { policy },
  ).then((data) => data.setInstanceAccessPolicy);
}

export function getInstanceAccessEvents(
  limit = 50,
): Promise<InstanceAccessEvent[]> {
  return postGraphQL<{ instanceAccessEvents: InstanceAccessEvent[] }>(
    `
      query InstanceAccessEvents($limit: Int) {
        instanceAccessEvents(limit: $limit) {
          id
          eventType
          occurredAt
          actorUserId
          previousPolicy
          newPolicy
          attemptedRoute
          policyAtAttempt
        }
      }
    `,
    { limit },
  ).then((data) => data.instanceAccessEvents);
}

export function getInstanceInvitations(): Promise<InstanceInvitation[]> {
  return postGraphQL<{ instanceInvitations: InstanceInvitation[] }>(
    `query InstanceInvitations { instanceInvitations { ${INVITATION_FIELDS} } }`,
    {},
  ).then((data) => data.instanceInvitations);
}

export function createInstanceInvitation(input: {
  maxUses?: number;
  expiresInHours?: number;
  note?: string;
}): Promise<InstanceInvitation> {
  return postGraphQL<{ createInstanceInvitation: InstanceInvitation }>(
    `
      mutation CreateInstanceInvitation($input: CreateInstanceInvitationInput!) {
        createInstanceInvitation(input: $input) { ${INVITATION_FIELDS} }
      }
    `,
    { input },
  ).then((data) => data.createInstanceInvitation);
}

export function revokeInstanceInvitation(
  invitationId: string,
): Promise<boolean> {
  return postGraphQL<{ revokeInstanceInvitation: boolean }>(
    `
      mutation RevokeInstanceInvitation($invitationId: UUID!) {
        revokeInstanceInvitation(invitationId: $invitationId)
      }
    `,
    { invitationId },
  ).then((data) => data.revokeInstanceInvitation);
}

/** The policy as the signed-out surface sees it, mapped to the admin enum. */
export function policyFromStatus(
  policy: InstanceAccessPolicy,
): InstanceAccessSettings["policy"] {
  switch (policy) {
    case "invite_only":
      return "INVITE_ONLY";
    case "closed":
      return "CLOSED";
    default:
      return "OPEN";
  }
}
