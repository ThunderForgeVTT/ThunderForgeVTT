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
