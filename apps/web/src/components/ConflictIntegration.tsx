import React, { useEffect } from 'react';
import { useConflictsOptional } from '@/contexts/ConflictContext';
import { ConflictNotification, ConflictPanel } from '@/components/ConflictNotification';

/**
 * Phase 4.9.C.3: Integration Example
 * 
 * Shows how to integrate conflict detection with the world view.
 * This component displays notifications and a conflict panel.
 * 
 * Usage in App.tsx or World view:
 * ```tsx
 * <ConflictProvider worldId={worldId}>
 *   <WorldView />
 *   <ConflictNotificationStack />
 * </ConflictProvider>
 * ```
 * 
 * Then in any child component, use:
 * ```tsx
 * const { lastConflict, dismissConflict, conflicts, unresolvedCount } = useConflicts();
 * ```
 */

export function ConflictNotificationStack() {
  const conflicts = useConflictsOptional();

  if (!conflicts || !conflicts.hasConflicts) {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 max-w-md space-y-2">
      {/* Show most recent conflict as a toast */}
      {conflicts.lastConflict && (
        <ConflictNotification
          conflict={conflicts.lastConflict}
          onDismiss={() => conflicts.dismissConflict(conflicts.lastConflict!.eventId)}
          onViewDetails={() => {
            // Could open a modal or panel here
            console.log('View conflict details:', conflicts.lastConflict);
          }}
        />
      )}
    </div>
  );
}

/**
 * Compact conflict badge for the header
 * Shows count of unresolved conflicts
 */
export function ConflictBadge() {
  const conflicts = useConflictsOptional();

  if (!conflicts || conflicts.unresolvedCount === 0) {
    return null;
  }

  return (
    <button
      className="inline-flex items-center gap-1 rounded-full bg-amber-200 px-3 py-1 text-sm font-semibold text-amber-900 hover:bg-amber-300"
      title="Click to view conflict history"
    >
      ⚡ {conflicts.unresolvedCount} conflict{conflicts.unresolvedCount !== 1 ? 's' : ''}
    </button>
  );
}

/**
 * Example: Hook into worldEventCreated subscription to feed events
 * This would be called from the world sync layer
 */
export function useWorldEventSubscription(worldId: string | null) {
  const conflicts = useConflictsOptional();

  useEffect(() => {
    if (!conflicts || !worldId) return;

    // Example: Hook into GraphQL subscription
    // In real implementation, this would connect to the actual worldEventCreated subscription
    
    console.log('🔌 [Phase4.9.C3] Conflict detection connected to world:', worldId);

    // Simulate subscription (for demo)
    // In production, this would be integrated with the actual GraphQL subscription
    // const unsubscribe = subscribeToWorldEvents(worldId, (event) => {
    //   conflicts.processWorldEvent(event);
    // });

    // return unsubscribe;
  }, [conflicts, worldId]);
}

/**
 * Example: Show conflicts in a sidebar panel
 */
export function ConflictHistoryPanel() {
  const conflicts = useConflictsOptional();

  if (!conflicts || conflicts.conflicts.length === 0) {
    return null;
  }

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4">
      <h3 className="mb-3 font-semibold text-gray-900">Conflict History</h3>
      <ConflictPanel
        conflicts={conflicts.conflicts}
        onDismiss={(eventId) => conflicts.dismissConflict(eventId)}
        onClearAll={() => conflicts.clearAllConflicts()}
      />
    </div>
  );
}
