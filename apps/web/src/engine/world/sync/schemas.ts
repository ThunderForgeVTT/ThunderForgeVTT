export const worldTokensSchema = {
  title: "world tokens schema",
  version: 0,
  primaryKey: "id",
  type: "object",
  properties: {
    id: {
      type: "string",
      maxLength: 128,
    },
    worldId: {
      type: "string",
      maxLength: 128,
      index: true,
    },
    x: {
      type: "number",
      minimum: -100000,
      maximum: 100000,
    },
    y: {
      type: "number",
      minimum: -100000,
      maximum: 100000,
    },
    z: {
      type: "number",
      minimum: -1000,
      maximum: 1000,
    },
    label: {
      type: "string",
      maxLength: 256,
    },
    updatedAt: {
      type: "string",
      format: "date-time",
      maxLength: 64,
    },
    version: {
      type: "number",
      minimum: 0,
    },
  },
  required: ["id", "worldId", "x", "y", "z", "updatedAt", "version"],
} as const;

export const worldSnapshotsSchema = {
  title: "world snapshots schema",
  version: 0,
  primaryKey: "id",
  type: "object",
  properties: {
    id: {
      type: "string",
      maxLength: 128,
    },
    worldId: {
      type: "string",
      maxLength: 128,
      index: true,
    },
    sceneVersion: {
      type: "number",
      minimum: 0,
    },
    updatedAt: {
      type: "string",
      format: "date-time",
      maxLength: 64,
    },
    payload: {
      type: "object",
      additionalProperties: true,
    },
  },
  required: ["id", "worldId", "sceneVersion", "updatedAt", "payload"],
} as const;

export const worldActorSystemDataSchema = {
  title: "world actor system data schema",
  version: 0,
  primaryKey: "id",
  type: "object",
  properties: {
    id: {
      type: "string",
      maxLength: 128,
    },
    actor_id: {
      type: "string",
      maxLength: 128,
      index: true,
    },
    game_system_id: {
      type: "string",
      maxLength: 128,
      index: true,
    },
    ability_data: {
      type: "object",
      additionalProperties: true,
    },
    resource_data: {
      type: "object",
      additionalProperties: true,
    },
    proficiency_data: {
      type: "object",
      additionalProperties: true,
    },
    trait_data: {
      type: "object",
      additionalProperties: true,
    },
    spell_data: {
      type: "object",
      additionalProperties: true,
    },
    created_by: {
      type: "string",
      maxLength: 128,
    },
    updated_by: {
      type: "string",
      maxLength: 128,
    },
    created_at: {
      type: "string",
      format: "date-time",
      maxLength: 64,
    },
    updated_at: {
      type: "string",
      format: "date-time",
      maxLength: 64,
    },
    _optimistic: {
      type: "boolean",
      default: false,
    },
    _lastServerData: {
      type: "object",
      additionalProperties: true,
    },
  },
  required: [
    "id",
    "actor_id",
    "game_system_id",
    "created_by",
    "updated_by",
    "created_at",
    "updated_at",
  ],
  indexes: ["actor_id", ["game_system_id", "updated_at"]],
} as const;

// Phase 4.10.C: Campaign invites and membership collections
export { worldInvitesSchema, computeInviteDerivedData } from "@/db/collections/worldInvitesCollection";
export type { WorldInviteDoc } from "@/db/collections/worldInvitesCollection";

export {
  worldMembersSchema,
  canManageRole,
  canGenerateInvites,
  sortMembersByRole,
  filterMembersByRole,
  findMember,
  isMemberWithRole,
} from "@/db/collections/worldMembersCollection";
export type { WorldMemberDoc } from "@/db/collections/worldMembersCollection";
