import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * Phase 4.9.C.3: Client-side Conflict Detection
 * 
 * Listens to worldEventCreated subscription and detects conflicts (event_code=2).
 * Maintains a log of recent conflicts and provides handlers for UI feedback.
 */

export interface ConflictRecord {
  eventId: number;
  tokenId: string;
  worldId: string;
  mutationUserId: string;
  conflictTimestamp: string;
  clientVersion?: string;
  serverVersion: string;
  appliedData: Record<string, any>;
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
export function useConflictDetection(worldId: string | null) {
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
  const processWorldEvent = useCallback((event: any) => {
    if (event.event_code !== EVENT_CODE_CONFLICT_LWW) {
      return; // Not a conflict event
    }

    console.log('🔔 [Phase4.9.C3] Conflict detected in worldEventCreated subscription');

    const payload = event.token_event || {};
    const tokenId = payload.token_id || 'unknown';
    const clientVersion = payload.client_version;
    const serverVersion = payload.server_version;

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
      const oldest = Array.from(conflictsRef.current.entries())
        .sort((a, b) => {
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
    console.warn('⚠️  [Phase4.9.C3] Conflict applied (Last-Write-Wins):', {
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
        lastConflict: unresolvedCount > 0 ? allConflicts.find((c) => !c.dismissed) : undefined,
      }));
    }
  }, []);

  /**
   * Get conflict details for a specific token
   */
  const getTokenConflicts = useCallback((tokenId: string): ConflictRecord[] => {
    return Array.from(conflictsRef.current.values()).filter(
      (c) => c.tokenId === tokenId && !c.dismissed
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
export function isConflictEvent(event: any): boolean {
  return event?.event_code === EVENT_CODE_CONFLICT_LWW;
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
