import { postGraphQL } from "@/api/graphqlClient";
import type {
  CreateWorldInput,
  DeleteWorldResult,
  MyWorldEntry,
  WorldRecord,
} from "@/types/world";

const WORLD_FIELDS = `
  id
  name
  description
  gameSystemId
  interfacePackId
  scenes
  actors
  tokens
  events
  gameSystem
  interfacePack
  createdBy
  updatedBy
  createdAt
  updatedAt
  sessionNotes
  allowPlayerCreatedActors
  genieResourceCarryoverEnabled
  defaultSceneGridType
  activeSceneId
`;

type MyWorldsQuery = {
  myWorlds: WorldRecord[];
};

type MyWorldsWithRoleQuery = {
  myWorldsWithRole: MyWorldEntry[];
};

type AllWorldsQuery = {
  allWorlds: WorldRecord[];
};

type WorldQuery = {
  world: WorldRecord | null;
};

type CreateWorldMutation = {
  createWorld: WorldRecord;
};

type DeleteWorldMutation = {
  deleteWorld: DeleteWorldResult;
};

export function getMyWorlds(): Promise<WorldRecord[]> {
  return postGraphQL<MyWorldsQuery>(`
    query MyWorlds {
      myWorlds {
        ${WORLD_FIELDS}
      }
    }
  `).then((data) => data.myWorlds);
}

/** Worlds the caller owns OR is an accepted member of (any role), each
 * paired with their role — unlike `getMyWorlds`, which is owned-only. */
export function getMyWorldsWithRole(): Promise<MyWorldEntry[]> {
  return postGraphQL<MyWorldsWithRoleQuery>(`
    query MyWorldsWithRole {
      myWorldsWithRole {
        role
        world {
          ${WORLD_FIELDS}
        }
      }
    }
  `).then((data) => data.myWorldsWithRole);
}

export function getAllWorlds(): Promise<WorldRecord[]> {
  return postGraphQL<AllWorldsQuery>(`
    query AllWorlds {
      allWorlds {
        ${WORLD_FIELDS}
      }
    }
  `).then((data) => data.allWorlds);
}

export function getWorld(id: string): Promise<WorldRecord | null> {
  return postGraphQL<WorldQuery>(
    `
      query WorldDashboard($id: UUID!) {
        world(id: $id) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { id },
  ).then((data) => data.world);
}

export function createWorld(input: CreateWorldInput): Promise<WorldRecord> {
  return postGraphQL<CreateWorldMutation>(
    `
      mutation CreateWorld($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createWorld);
}

export function deleteWorld(id: string): Promise<DeleteWorldResult> {
  return postGraphQL<DeleteWorldMutation>(
    `
      mutation DeleteWorld($id: UUID!) {
        deleteWorld(id: $id) {
          id
          status
          message
        }
      }
    `,
    { id },
  ).then((data) => data.deleteWorld);
}

type UpdateWorldSessionNotesMutation = {
  updateWorldSessionNotes: WorldRecord;
};

/** DM/GM-only (FR-012). Saving an empty string is a valid, explicit save
 * (FR-013), not a no-op. */
export function updateWorldSessionNotes(
  worldId: string,
  notes: string,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldSessionNotesMutation>(
    `
      mutation UpdateWorldSessionNotes($input: UpdateWorldSessionNotesInput!) {
        updateWorldSessionNotes(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, notes } },
  ).then((data) => data.updateWorldSessionNotes);
}

type UpdateWorldAllowPlayerCreatedActorsMutation = {
  updateWorldAllowPlayerCreatedActors: WorldRecord;
};

/**
 * Spec 017 (FR-007): DM/GM-only. Gates the Actor Selection screen's
 * "create your own character" option, re-checked server-side on every
 * `createAndClaimActor` call regardless of this cached value.
 */
export function updateWorldAllowPlayerCreatedActors(
  worldId: string,
  allow: boolean,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldAllowPlayerCreatedActorsMutation>(
    `
      mutation UpdateWorldAllowPlayerCreatedActors($input: UpdateWorldAllowPlayerCreatedActorsInput!) {
        updateWorldAllowPlayerCreatedActors(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, allow } },
  ).then((data) => data.updateWorldAllowPlayerCreatedActors);
}

type WorldMemberQuery = {
  worldMember: { role: string } | null;
};

/**
 * Spec 017 (research.md §5): a direct, synchronous GraphQL read of the
 * caller's own membership role — used by `useActorClaimGate` instead of
 * the RxDB-backed `useWorldRole` hook, since a member who has *just*
 * joined via `joinWorld` (the exact moment this gate matters) cannot
 * assume RxDB's world-member collection has replicated their own new row
 * yet; this query hits the server's `world_members` table directly.
 */
export function getMyWorldMemberRole(
  worldId: string,
  userId: string,
): Promise<string | null> {
  return postGraphQL<WorldMemberQuery>(
    `
      query MyWorldMemberRole($worldId: ID!, $userId: ID!) {
        worldMember(worldId: $worldId, userId: $userId) {
          role
        }
      }
    `,
    { worldId, userId },
  ).then((data) => data.worldMember?.role ?? null);
}

/**
 * Spec 027 (FR-010): whether a link works right now, and if not, why.
 * Derived server-side from the row, never stored, so it cannot drift.
 */
export type WorldAccessLinkState =
  | "ACTIVE"
  | "EXPIRED"
  | "EXHAUSTED"
  | "REVOKED";

export interface WorldInviteRecord {
  id: string;
  worldId: string;
  inviteCode: string;
  maxUses: number;
  usedCount: number;
  expiresAt?: string | null;
  createdBy: string;
  createdAt: string;
  updatedAt: string;

  /** Spec 027 (FR-010): supersedes `status`, which could not express revoked. */
  state: WorldAccessLinkState;
  /** Uses left, or `null` when the link is uncapped. */
  remainingUses?: number | null;
  /** The link this one replaced, when created by rotation. */
  rotatedFrom?: string | null;

  /**
   * @deprecated Spec 027: a `"3/10 uses"` string cannot express revocation, so
   * a revoked link rendered identically to a working one. Use `state`.
   */
  status: string;
}

/** Every field the panel needs, in one place so the three operations agree. */
const INVITE_FIELDS = `
  id
  worldId
  inviteCode
  maxUses
  usedCount
  expiresAt
  createdBy
  createdAt
  updatedAt
  state
  remainingUses
  rotatedFrom
`;

type WorldInvitesQuery = {
  worldInvites: WorldInviteRecord[];
};

/**
 * Fetches a world's active invite codes directly via GraphQL. RxDB has
 * been hard cut from this layer entirely (see `getWorldMembers`'s doc
 * comment in `api/worldMembers.ts` for why) — `useWorldInvites` now calls
 * this function directly instead of subscribing to a local RxDB
 * collection. Mirrors `CampaignSettingsPanel.tsx`'s own direct-query
 * pattern for the same data.
 */
export function getWorldInvites(worldId: string): Promise<WorldInviteRecord[]> {
  return postGraphQL<WorldInvitesQuery>(
    `
      query WorldInvites($worldId: ID!) {
        worldInvites(worldId: $worldId) {
          ${INVITE_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldInvites);
}

/**
 * Issues a new invite link for a world. Owner/GM only.
 *
 * Spec 027: moved here from an inline query inside `CampaignSettingsPanel`,
 * which carried its own private `postGraphQL` copy — one the transport
 * consolidation missed because it lived in a component rather than `src/api/`.
 * That copy called `response.json()` unconditionally, surfaced only the first
 * error, and had no timeout.
 */
export function generateInviteCode(
  worldId: string,
  maxUses: number,
  expiresAt?: string | null,
): Promise<WorldInviteRecord> {
  return postGraphQL<{ generateInviteCode: WorldInviteRecord }>(
    `
      mutation GenerateInviteCode($input: GenerateInviteCodeInput!) {
        generateInviteCode(input: $input) {
          ${INVITE_FIELDS}
        }
      }
    `,
    { input: { worldId, maxUses, expiresAt: expiresAt ?? null } },
  ).then((data) => data.generateInviteCode);
}

/**
 * Spec 027 (FR-002): permanently retire a link, with no replacement.
 *
 * Irreversible — a revoked code never becomes usable again. Anyone who already
 * joined with it keeps their membership; this governs future joins only.
 */
export function revokeInviteCode(inviteId: string): Promise<WorldInviteRecord> {
  return postGraphQL<{ revokeInviteCode: WorldInviteRecord }>(
    `
      mutation RevokeInviteCode($inviteId: UUID!) {
        revokeInviteCode(inviteId: $inviteId) {
          ${INVITE_FIELDS}
        }
      }
    `,
    { inviteId },
  ).then((data) => data.revokeInviteCode);
}

/**
 * Spec 027 (FR-003): retire a link and issue its replacement atomically.
 *
 * Returns the **new** link. The old code stops working on its very next use —
 * this is what makes a leaked link recoverable. The replacement carries the
 * same use cap and the same chosen lifetime, both measured afresh.
 */
export function rotateInviteCode(inviteId: string): Promise<WorldInviteRecord> {
  return postGraphQL<{ rotateInviteCode: WorldInviteRecord }>(
    `
      mutation RotateInviteCode($inviteId: UUID!) {
        rotateInviteCode(inviteId: $inviteId) {
          ${INVITE_FIELDS}
        }
      }
    `,
    { inviteId },
  ).then((data) => data.rotateInviteCode);
}

type UpdateWorldGameSystemMutation = {
  updateWorldGameSystem: WorldRecord;
};

/** Spec 016 (T009): assigns/changes a world's active system pack.
 * DM/GM-only, server-enforced. */
export function updateWorldGameSystem(
  worldId: string,
  gameSystemId: string,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldGameSystemMutation>(
    `
      mutation UpdateWorldGameSystem($input: UpdateWorldGameSystemInput!) {
        updateWorldGameSystem(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, gameSystemId } },
  ).then((data) => data.updateWorldGameSystem);
}

type UpdateWorldInterfacePackMutation = {
  updateWorldInterfacePack: WorldRecord;
};

/**
 * Spec 032 (FR-010): sets or clears the world's interface pack. DM/GM-only,
 * server-enforced.
 *
 * `null` clears the binding, which means "the base pack" — a real thing a Game
 * Master may want, and the reason this id is nullable where
 * `updateWorldGameSystem`'s is not.
 */
export function updateWorldInterfacePack(
  worldId: string,
  interfacePackId: string | null,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldInterfacePackMutation>(
    `
      mutation UpdateWorldInterfacePack($input: UpdateWorldInterfacePackInput!) {
        updateWorldInterfacePack(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, interfacePackId } },
  ).then((data) => data.updateWorldInterfacePack);
}

type UpdateWorldGenieResourceCarryoverMutation = {
  updateWorldGenieResourceCarryover: WorldRecord;
};

/** Spec 020 (FR-003, research.md R1): DM/GM-only, server-enforced. */
export function updateWorldGenieResourceCarryover(
  worldId: string,
  enabled: boolean,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldGenieResourceCarryoverMutation>(
    `
      mutation UpdateWorldGenieResourceCarryover($input: UpdateWorldGenieResourceCarryoverInput!) {
        updateWorldGenieResourceCarryover(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, enabled } },
  ).then((data) => data.updateWorldGenieResourceCarryover);
}

type UpdateWorldDefaultSceneGridTypeMutation = {
  updateWorldDefaultSceneGridType: WorldRecord;
};

/** Spec 022 (FR-014): DM/GM-only, server-enforced. `gridType` must be
 * one of "square" | "hex" | "gridless". */
export function updateWorldDefaultSceneGridType(
  worldId: string,
  gridType: string,
): Promise<WorldRecord> {
  return postGraphQL<UpdateWorldDefaultSceneGridTypeMutation>(
    `
      mutation UpdateWorldDefaultSceneGridType($input: UpdateWorldDefaultSceneGridTypeInput!) {
        updateWorldDefaultSceneGridType(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input: { worldId, gridType } },
  ).then((data) => data.updateWorldDefaultSceneGridType);
}
