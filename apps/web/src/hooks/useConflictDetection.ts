import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Phase 4.9.C.3: Client-side Conflict Detection
 *
 * Listens to worldEventCreated subscription and detects conflicts (event_code=2).
 * Maintains a log of recent conflicts and provides handlers for UI feedback.
 */

/**
 * The `token_event` JSONB an `event_code=2` row carries. Written by whoever
 * emitted the conflict, so every field is optional here and checked before
 * use: nothing in the schema forces the writer to include any of them.
 */
export interface ConflictEventPayload {
  [key: string]: unknown;
  token_id?: unknown;
  client_version?: unknown;
  server_version?: unknown;
}

/**
 * One `world_events` row as the `worldEventCreated` subscription delivers it
 * — `GraphQLWorldEvent` in `src/server/src/graphql/types.rs`, where every
 * column but `token_event` is non-null.
 */
export interface WorldEventRecord {
  id: number;
  world_id: string;
  event_code: number;
  created_by: string;
  created_at: string;
  token_event?: ConflictEventPayload | null;
}

export interface ConflictRecord {
  eventId: number;
  tokenId: string;
  worldId: string;
  mutationUserId: string;
  conflictTimestamp: string;
  clientVersion?: string;
  serverVersion: string;
  appliedData: Record<string, unknown>;
  dismissed: boolean;
}

export interface ConflictDetectionState {
  conflicts: ConflictRecord[];
  unresolvedCount: number;
  lastConflict?: ConflictRecord;
  hasConflicts: boolean;
}

const EVENT_CODE_CONFLICT_LWW = 2;
const MAX_STORED_CONFLICTS = 50;

/**
 * Hook to detect conflicts from world events
 *
 * Usage:
 * ```tsx
 * const { conflicts, unresolvedCount, lastConflict, dismissConflict } = useConflictDetection(worldId);
 *
 * if (lastConflict) {
 *   return <ConflictNotification conflict={lastConflict} onDismiss={() => dismissConflict(lastConflict.eventId)} />
 * }
 * ```
 */
// `_worldId` is deliberate, not an oversight. The events this consumes arrive
// on a per-world channel (`thunderforge-pg-sockets`'s `WorldRouter`), so they
// are already scoped by construction — that crate's docs say callers "have
// nothing to filter, which is the point". The parameter stays because it
// documents what the hook is about and every caller has one.
export function useConflictDetection(_worldId: string | null) {
  const [state, setState] = useState<ConflictDetectionState>({
    conflicts: [],
    unresolvedCount: 0,
    hasConflicts: false,
  });

  const conflictsRef = useRef<Map<number, ConflictRecord>>(new Map());
  const isActiveRef = useRef(true);

  /**
   * Process a world event and extract conflict if present
   * Phase 4.9.C: event_code=2 indicates conflict
   */
  const processWorldEvent = useCallback((event: WorldEventRecord) => {
    if (event.event_code !== EVENT_CODE_CONFLICT_LWW) {
      return; // Not a conflict event
    }

    console.log(
      "🔔 [Phase4.9.C3] Conflict detected in worldEventCreated subscription",
    );

    // The payload is free-form JSONB, so each field is checked rather than
    // trusted: a conflict whose version stamps are missing must still be
    // reported to the player, just without the timestamps to compare.
    const payload: ConflictEventPayload = event.token_event ?? {};
    const tokenId =
      typeof payload.token_id === "string" && payload.token_id
        ? payload.token_id
        : "unknown";
    const clientVersion =
      typeof payload.client_version === "string"
        ? payload.client_version
        : undefined;
    const serverVersion =
      typeof payload.server_version === "string" ? payload.server_version : "";

    const conflict: ConflictRecord = {
      eventId: event.id,
      tokenId,
      worldId: event.world_id,
      mutationUserId: event.created_by,
      conflictTimestamp: event.created_at,
      clientVersion,
      serverVersion,
      appliedData: payload,
      dismissed: false,
    };

    conflictsRef.current.set(event.id, conflict);

    // Keep only last MAX_STORED_CONFLICTS
    if (conflictsRef.current.size > MAX_STORED_CONFLICTS) {
      const oldest = Array.from(conflictsRef.current.entries()).sort((a, b) => {
        const timeA = new Date(a[1].conflictTimestamp).getTime();
        const timeB = new Date(b[1].conflictTimestamp).getTime();
        return timeA - timeB;
      })[0];

      if (oldest) {
        conflictsRef.current.delete(oldest[0]);
      }
    }

    if (!isActiveRef.current) return;

    // Update state
    const allConflicts = Array.from(conflictsRef.current.values());
    const unresolvedCount = allConflicts.filter((c) => !c.dismissed).length;
    const lastConflict = allConflicts[allConflicts.length - 1];

    setState({
      conflicts: allConflicts,
      unresolvedCount,
      lastConflict,
      hasConflicts: unresolvedCount > 0,
    });

    // Log conflict for debugging
    console.warn("⚠️  [Phase4.9.C3] Conflict applied (Last-Write-Wins):", {
      tokenId,
      clientExpected: clientVersion,
      serverActual: serverVersion,
      applied: payload,
    });
  }, []);

  /**
   * Dismiss a conflict from the UI
   */
  const dismissConflict = useCallback((eventId: number) => {
    const conflict = conflictsRef.current.get(eventId);
    if (conflict) {
      conflict.dismissed = true;
      conflictsRef.current.set(eventId, conflict);

      if (!isActiveRef.current) return;

      const allConflicts = Array.from(conflictsRef.current.values());
      const unresolvedCount = allConflicts.filter((c) => !c.dismissed).length;

      setState((prev) => ({
        ...prev,
        unresolvedCount,
        hasConflicts: unresolvedCount > 0,
        lastConflict:
          unresolvedCount > 0
            ? allConflicts.find((c) => !c.dismissed)
            : undefined,
      }));
    }
  }, []);

  /**
   * Get conflict details for a specific token
   */
  const getTokenConflicts = useCallback((tokenId: string): ConflictRecord[] => {
    return Array.from(conflictsRef.current.values()).filter(
      (c) => c.tokenId === tokenId && !c.dismissed,
    );
  }, []);

  /**
   * Clear all conflicts
   */
  const clearAllConflicts = useCallback(() => {
    conflictsRef.current.clear();

    if (!isActiveRef.current) return;

    setState({
      conflicts: [],
      unresolvedCount: 0,
      lastConflict: undefined,
      hasConflicts: false,
    });
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      isActiveRef.current = false;
    };
  }, []);

  return {
    ...state,
    processWorldEvent,
    dismissConflict,
    getTokenConflicts,
    clearAllConflicts,
  };
}

/**
 * Helper to check if an event is a conflict event
 */
export function isConflictEvent(event: unknown): boolean {
  return (
    typeof event === "object" &&
    event !== null &&
    "event_code" in event &&
    event.event_code === EVENT_CODE_CONFLICT_LWW
  );
}

/**
 * Extract version info from conflict event
 */
export function extractVersionInfo(conflict: ConflictRecord) {
  const clientTime = conflict.clientVersion
    ? new Date(conflict.clientVersion).getTime()
    : null;
  const serverTime = new Date(conflict.serverVersion).getTime();
  const timeDiffMs = clientTime ? serverTime - clientTime : 0;

  return {
    clientVersion: conflict.clientVersion,
    serverVersion: conflict.serverVersion,
    timeDiffMs,
    clientWasNewer: clientTime ? clientTime > serverTime : false,
  };
}
