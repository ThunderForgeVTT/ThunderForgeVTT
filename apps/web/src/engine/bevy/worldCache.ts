import { startPeerTransfer } from "./peer";
import { getWasmModule } from "./wasmModule";

/**
 * What one world-open sync did, as reported by the engine.
 *
 * Read-only telemetry. Note what is *not* here: no list of cached items, no
 * fingerprints, nothing that would let this side form a second opinion about
 * what the cache holds. Per research.md R1 and Constitution Principle I,
 * cache policy has exactly one owner and it is the Rust side — TypeScript
 * asks for a sync and reads the outcome, and that is the whole of its
 * involvement.
 */
export interface WorldCacheSyncSummary {
  /** `"synced"` when the plan was applied; `"degraded"` otherwise. */
  status: "synced" | "degraded";
  /** Why it degraded. Absent on success. */
  reason?: string;
  /** Items the client already held and told the server about. */
  held?: number;
  /** Items the server asked for. */
  fetch?: number;
  /** Index rows discarded, and blob files that went with them. */
  evicted?: number;
  blobsRemoved?: number;
  evictFailures?: number;
  /**
   * The FR-019 integrity pass, reported with the plan it rides with: index
   * rows dropped because their blob was gone or unreadable, and orphaned
   * blobs reclaimed. Surfaced because FR-051 asks the diagnostics view for
   * "any integrity repairs performed" — a store that silently repaired
   * itself is exactly the thing a user should be able to see happened.
   */
  rowsRepaired?: number;
  blobsReclaimed?: number;
  repairFailures?: number;
  /**
   * FR-024: even releasing everything permissible left no room, so the cache
   * is serving what it holds and has stopped adding. Not a failure, but the
   * one cache state that explains an otherwise inexplicable readout — a
   * warm-looking world that keeps fetching.
   */
  budgetInsufficient?: boolean;
  /** Assets being pulled ahead of demand, in the background. */
  prefetching?: number;
  canonicalVersion?: number;
  /**
   * Whether anyone else was live in this world at sync time (spec 028 T086).
   * Reachability, not holdings — it never means a peer has any given bytes,
   * and a `false` never suppresses a server fetch.
   */
  peerAvailable?: boolean;
}

/**
 * Spec 028 (US1): bring this browser's cache into agreement with the server
 * for the world being opened.
 *
 * One call does everything — derives the OPFS scope from the authenticated
 * user id, sends the manifest, applies the returned plan, and points the
 * engine's asset read path at the result. That is deliberate: the manifest
 * and the fetch/evict decisions live in `thunderforge-cache-browser`, where
 * the index that produces them lives, and never cross this boundary.
 *
 * **Cannot fail.** Resolves to `null` when the engine has no cache entry
 * point at all, and to a `"degraded"` summary for every runtime failure —
 * no OPFS, no session key, an unreachable server, a malformed plan. In every
 * one of those cases the app behaves exactly as it did before this feature
 * existed: assets load from the network. A cache problem must never stop a
 * world from opening, so this swallows rather than rethrows.
 */
export async function syncWorldCache(
  worldId: string,
  userId: string,
): Promise<WorldCacheSyncSummary | null> {
  try {
    const module = await getWasmModule();
    if (!module.sync_world_cache) {
      return null;
    }
    // Before the sync, not after, and this is the only ordering that works:
    // `sync_world_cache` is what hands the peer module its entitlement scope
    // (`plan.fetch`) and its servable set, and it can only hand them to a
    // module that is already running. Started after, the client would spend a
    // whole world open unable to ask any peer for anything.
    //
    // Awaited, but cheap — it resolves once the engine can talk to peers, and
    // leaves the roster round trip running behind it. Failure is invisible on
    // purpose: server-only transfer is a supported way to run (SC-013).
    await startPeerTransfer(worldId);

    const summary = await module.sync_world_cache(worldId, userId);
    return JSON.parse(summary) as WorldCacheSyncSummary;
  } catch {
    return null;
  }
}

/** One queued change, as the engine hands it over. */
export interface QueuedChangeWire {
  localId: string;
  command: unknown;
}

/**
 * Queue an edit made while disconnected (US7, FR-037).
 *
 * Returns `false` when the change could not be stored — no engine, an older
 * bundle, a failed write. **The caller must not report the edit as accepted in
 * that case.** This is the one path in the cache where a failure has to reach
 * the user, because unlike a missing blob it cannot be recovered by fetching
 * again: the work exists nowhere else.
 */
export async function queueOfflineChange(
  worldId: string,
  localId: string,
  command: unknown,
  isGameMaster: boolean,
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    if (!module.queue_offline_change) return false;
    const result = await module.queue_offline_change(
      worldId,
      localId,
      JSON.stringify(command),
      isGameMaster,
    );
    return (JSON.parse(result) as { queued?: boolean }).queued === true;
  } catch {
    return false;
  }
}

/** Everything queued for a world, in replay order. Empty on any failure. */
export async function readQueuedChanges(
  worldId: string,
): Promise<QueuedChangeWire[]> {
  try {
    const module = await getWasmModule();
    if (!module.read_queued_changes) return [];
    return JSON.parse(
      await module.read_queued_changes(worldId),
    ) as QueuedChangeWire[];
  } catch {
    return [];
  }
}

/**
 * Drop the queued changes the server answered for, keeping the rest.
 *
 * Returns how many are still queued, or `null` if the drain could not run —
 * which is not a failure worth surfacing, because everything simply stays
 * queued and is answered for on the next reconnect.
 */
export async function forgetReconciledChanges(
  outcomes: { localId: string; applied: boolean }[],
): Promise<number | null> {
  try {
    const module = await getWasmModule();
    if (!module.forget_reconciled_changes) return null;
    const result = await module.forget_reconciled_changes(
      JSON.stringify(outcomes),
    );
    const remaining = (JSON.parse(result) as { remaining?: number }).remaining;
    return typeof remaining === "number" && remaining >= 0 ? remaining : null;
  } catch {
    return null;
  }
}
