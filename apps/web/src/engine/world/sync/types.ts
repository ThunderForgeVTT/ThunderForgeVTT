import type { WorldStore, WorldStoreEvent } from "../store";
import type { WorldToken } from "../types";

export type DurableWorldTokenDoc = {
  id: string;
  worldId: string;
  x: number;
  y: number;
  z: number;
  label?: string;
  updatedAt: string;
  version: number;
};

export type WorldSnapshotDoc = {
  id: string;
  worldId: string;
  sceneVersion: number;
  updatedAt: string;
  payload: Record<string, unknown>;
};

export type TokenDelta = {
  worldId: string;
  tokenId: string;
  x: number;
  y: number;
  z: number;
  at: string;
};

export type DurableMutation =
  | { type: "upsert_token"; doc: DurableWorldTokenDoc }
  | { type: "remove_token"; worldId: string; tokenId: string }
  | { type: "snapshot"; doc: WorldSnapshotDoc };

export type WorldSyncTransport = {
  start?: (worldId: string) => Promise<void>;
  stop?: (worldId: string) => Promise<void>;
  pushDurableMutations?: (mutations: DurableMutation[]) => Promise<void>;
  sendTokenDeltas?: (deltas: TokenDelta[]) => Promise<void>;
};

export type StartWorldSyncOptions = {
  worldId: string;
  worldStore: WorldStore;
  transport?: WorldSyncTransport;
  flushIntervalMs?: number;
  maxDeltaBatchSize?: number;
};

export type WorldSyncSession = {
  flush: () => Promise<void>;
  stop: () => Promise<void>;
};

export type WorldStoreSyncEvent = WorldStoreEvent & {
  source: "sync" | WorldStoreEvent["source"];
};

export function tokenToDoc(
  worldId: string,
  token: WorldToken
): DurableWorldTokenDoc {
  return {
    id: token.id,
    worldId,
    x: token.x,
    y: token.y,
    z: token.z,
    label: token.label,
    updatedAt: new Date().toISOString(),
    version: Date.now(),
  };
}

export function createNoopWorldSyncTransport(): WorldSyncTransport {
  return {
    async start() {
      return;
    },
    async stop() {
      return;
    },
    async pushDurableMutations() {
      return;
    },
    async sendTokenDeltas() {
      return;
    },
  };
}
