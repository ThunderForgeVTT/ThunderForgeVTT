/**
 * actorSearch.ts
 *
 * Client-side, instant-as-you-type search over a world's actor roster
 * (the NPC catalog on the staging page), backed by FlexSearch
 * (https://github.com/nextapps-de/flexsearch).
 *
 * One `Document` index per world, keyed by world UUID, mounted to
 * IndexedDB (https://github.com/nextapps-de/flexsearch/blob/master/doc/persistent-indexeddb.md)
 * so re-opening the same world's staging page doesn't have to rebuild
 * the index from nothing — FlexSearch replays it from the browser's own
 * IndexedDB store. This is a pure client-side speed layer: the
 * authoritative roster always comes from `getWorldActors` (GraphQL); the
 * index just makes filtering it instant and, via `resolveActors`,
 * disposable if a document ever falls out of sync (Actors search never
 * fabricates rows for ids it doesn't recognize).
 *
 * Paired with the server-side `searchActors(worldId, query)` GraphQL
 * query (`queries/actor.rs`) for callers that don't already have the
 * full roster loaded client-side (or want a ranked/contextual match
 * across a roster too large to mirror locally) — this module intentionally
 * only covers the "already have the data, want instant filtering" half.
 */

import { Document } from "flexsearch";

export interface SearchableActor {
  id: string;
  label: string;
  description?: string | null;
}

interface WorldIndex {
  document: InstanceType<typeof Document>;
  ready: Promise<void>;
}

const indexesByWorld = new Map<string, WorldIndex>();

function createDocument(): InstanceType<typeof Document> {
  return new Document({
    tokenize: "forward",
    document: {
      id: "id",
      index: ["label", "description"],
    },
  });
}

/** Best-effort IndexedDB persistence — mounting can fail (private
 * browsing, disabled storage, non-browser test environments); the index
 * still works perfectly well in-memory-only if it does. */
async function mountPersistence(
  worldId: string,
  document: InstanceType<typeof Document>,
): Promise<void> {
  if (typeof indexedDB === "undefined") {
    return;
  }
  try {
    const { default: IndexedDBStorage } = await import("flexsearch/db/indexeddb");
    const db = new IndexedDBStorage(`actor-search-${worldId}`);
    await document.mount(db);
  } catch {
    // No persistence this session — the in-memory index above still
    // works; it just gets rebuilt from `actors` on next page load.
  }
}

function getOrCreateIndex(worldId: string): WorldIndex {
  let entry = indexesByWorld.get(worldId);
  if (!entry) {
    const document = createDocument();
    entry = { document, ready: mountPersistence(worldId, document) };
    indexesByWorld.set(worldId, entry);
  }
  return entry;
}

/** (Re)indexes the given actors for a world. Safe to call on every
 * roster fetch — FlexSearch's `add` upserts by id. */
export async function indexActors(worldId: string, actors: SearchableActor[]): Promise<void> {
  const { document, ready } = getOrCreateIndex(worldId);
  await ready.catch(() => {});
  for (const actor of actors) {
    // Once a `Document` is mounted to persistent (IndexedDB) storage,
    // FlexSearch's methods return Promises instead of resolving
    // synchronously (they now have to round-trip through the DB) — await
    // unconditionally so this works whether or not persistence mounted.
    await document.add({
      id: actor.id,
      label: actor.label,
      description: actor.description ?? "",
    });
  }
}

export function removeActorFromIndex(worldId: string, actorId: string): void {
  const entry = indexesByWorld.get(worldId);
  entry?.document.remove(actorId);
}

/** Returns the ids of actors matching `query`, ranked by FlexSearch's
 * relevance. Empty/whitespace-only queries return `null` — callers should
 * treat that as "no filter, show everything" rather than "no matches". */
export async function searchActorIds(worldId: string, query: string): Promise<string[] | null> {
  const trimmed = query.trim();
  if (!trimmed) {
    return null;
  }
  const { document, ready } = getOrCreateIndex(worldId);
  await ready.catch(() => {});
  // See `indexActors`'s comment: mounted (persistent) indexes return
  // Promises from `search()` too.
  const results = (await document.search(trimmed, { merge: true })) as Array<{
    id: string | number;
  }>;
  return results.map((r) => String(r.id));
}

/** Clears a world's index (e.g. on sign-out or world deletion) — mainly
 * useful for tests, since a stale in-memory index is otherwise harmless
 * (every entry is re-added on the next `indexActors` call anyway). */
export function clearActorIndex(worldId: string): void {
  indexesByWorld.delete(worldId);
}
