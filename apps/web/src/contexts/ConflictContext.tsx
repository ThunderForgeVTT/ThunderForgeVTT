import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  ReactNode,
} from "react";
import {
  useConflictDetection,
  type ConflictDetectionState,
} from "@/hooks/useConflictDetection";

/**
 * Phase 4.9.C.3: Conflict Detection Context
 *
 * Provides conflict detection state and handlers throughout the app.
 * Integrates with worldEventCreated subscription to detect conflicts in real-time.
 */

interface ConflictContextValue extends ConflictDetectionState {
  processWorldEvent: (event: any) => void;
  dismissConflict: (eventId: number) => void;
  clearAllConflicts: () => void;
}

const ConflictContext = createContext<ConflictContextValue | undefined>(
  undefined,
);

interface ConflictProviderProps {
  worldId: string | null;
  children: ReactNode;
  onSubscriptionEvent?: (event: any) => void; // Hook for tests/debugging
}

/**
 * Provider component that wraps the app
 * Listens to worldEventCreated subscription and detects conflicts
 */
export function ConflictProvider({
  worldId,
  children,
  onSubscriptionEvent,
}: ConflictProviderProps) {
  const conflictDetection = useConflictDetection(worldId);

  /**
   * Handle incoming world events from subscription
   * Call this whenever worldEventCreated fires
   */
  const handleWorldEvent = useCallback(
    (event: any) => {
      if (onSubscriptionEvent) {
        onSubscriptionEvent(event);
      }
      conflictDetection.processWorldEvent(event);
    },
    [conflictDetection, onSubscriptionEvent],
  );

  const value: ConflictContextValue = {
    ...conflictDetection,
    processWorldEvent: handleWorldEvent,
  };

  return (
    <ConflictContext.Provider value={value}>
      {children}
    </ConflictContext.Provider>
  );
}

/**
 * Hook to use conflict detection in components
 */
export function useConflicts(): ConflictContextValue {
  const context = useContext(ConflictContext);
  if (!context) {
    throw new Error("useConflicts must be used within ConflictProvider");
  }
  return context;
}

/**
 * Alternative: Standalone hook for components that don't have provider
 * (e.g., tests, simple integrations)
 */
export function useConflictsOptional(): ConflictContextValue | undefined {
  return useContext(ConflictContext);
}
