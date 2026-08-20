import { createTokenDeltaCoalescer } from "./coalescer";
import { getWorldDatabase } from "./database";
import type {
  DurableMutation,
  StartWorldSyncOptions,
  TokenDelta,
  WorldSyncSession,
} from "./types";
import { createNoopWorldSyncTransport, tokenToDoc } from "./types";

export { createNoopWorldSyncTransport } from "./types";
export { createGraphQLWorldSyncTransport } from "./transport";
export {
  startActorDataReplication,
  useActorSystemDataSubscription,
  prepareActorDataOptimisticUpdate,
  applyActorDataOptimisticUpdate,
  type ReplicationOptions,
  type ActorSystemDataEvent,
} from "./replication";
export {
  applyWallWorldEvent,
  startWallEventSync,
  loadWallsIntoStore,
  startWallMutationBridge,
} from "./walls";

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function toWorldTokenDocRecord(doc: unknown): Record<string, unknown> | null {
  if (!isObject(doc)) {
    return null;
  }

  if (typeof doc.id !== "string") {
    return null;
  }

  return doc;
}

export async function startWorldSync(
  options: StartWorldSyncOptions,
): Promise<WorldSyncSession> {
  const transport = options.transport ?? createNoopWorldSyncTransport();
  const flushIntervalMs = options.flushIntervalMs ?? 100;
  const maxDeltaBatchSize = options.maxDeltaBatchSize ?? 64;
  const db = await getWorldDatabase();

  await transport.start?.(options.worldId);

  const tokenCollection = db.collections.world_tokens;
  const durableMutationsBuffer: DurableMutation[] = [];

  const tokenDocs = await tokenCollection
    .find({
      selector: {
        worldId: {
          $eq: options.worldId,
        },
      },
    })
    .exec();

  for (const rawDoc of tokenDocs) {
    const doc = toWorldTokenDocRecord(rawDoc);
    if (!doc) {
      continue;
    }

    const id = doc.id;
    const x = doc.x;
    const y = doc.y;
    const z = doc.z;
    const label = doc.label;

    if (
      typeof id === "string" &&
      typeof x === "number" &&
      typeof y === "number" &&
      typeof z === "number" &&
      (typeof label === "string" || label === undefined)
    ) {
      options.worldStore.dispatch(
        {
          type: "upsert_token",
          token: { id, x, y, z, label },
        },
        "sync",
      );
    }
  }

  const deltaCoalescer = createTokenDeltaCoalescer({
    flushIntervalMs,
    maxBatchSize: maxDeltaBatchSize,
    onFlush: async (deltas: TokenDelta[]) => {
      await transport.sendTokenDeltas?.(deltas);
    },
  });

  const unsubscribe = options.worldStore.subscribe((event) => {
    if (event.source === "sync") {
      return;
    }

    if (event.command.type === "upsert_token") {
      const doc = tokenToDoc(options.worldId, event.command.token);
      void tokenCollection.upsert(doc);
      durableMutationsBuffer.push({ type: "upsert_token", doc });
      void deltaCoalescer.enqueue({
        worldId: options.worldId,
        tokenId: doc.id,
        x: doc.x,
        y: doc.y,
        z: doc.z,
        at: doc.updatedAt,
      });
      return;
    }

    if (event.command.type === "remove_token") {
      const tokenId = event.command.tokenId;
      void tokenCollection
        .findOne(tokenId)
        .exec()
        .then((doc) => doc?.remove());
      durableMutationsBuffer.push({
        type: "remove_token",
        worldId: options.worldId,
        tokenId,
      });
    }
  });

  return {
    async flush() {
      await deltaCoalescer.flush();

      if (durableMutationsBuffer.length > 0) {
        const mutations = durableMutationsBuffer.splice(
          0,
          durableMutationsBuffer.length,
        );
        await transport.pushDurableMutations?.(mutations);
      }
    },

    async stop() {
      unsubscribe();
      await deltaCoalescer.stop();

      if (durableMutationsBuffer.length > 0) {
        const mutations = durableMutationsBuffer.splice(
          0,
          durableMutationsBuffer.length,
        );
        await transport.pushDurableMutations?.(mutations);
      }

      await transport.stop?.(options.worldId);
    },
  };
}
