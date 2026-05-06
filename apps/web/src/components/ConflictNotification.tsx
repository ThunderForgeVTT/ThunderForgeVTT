import React, { useState } from 'react';
import type { ConflictRecord } from '@/hooks/useConflictDetection';
import { extractVersionInfo } from '@/hooks/useConflictDetection';

/**
 * Phase 4.9.C.3: Conflict Notification Component
 * 
 * Displays conflict alerts with:
 * - What changed (token moved to new position)
 * - Who caused it (user ID)
 * - Version mismatch details
 * - Actions: Dismiss, View Details
 */

interface ConflictNotificationProps {
  conflict: ConflictRecord;
  onDismiss: () => void;
  onViewDetails?: () => void;
}

export function ConflictNotification({
  conflict,
  onDismiss,
  onViewDetails,
}: ConflictNotificationProps) {
  const [showDetails, setShowDetails] = useState(false);
  const versionInfo = extractVersionInfo(conflict);

  const formatTime = (isoString: string | undefined) => {
    if (!isoString) return 'unknown';
    try {
      const date = new Date(isoString);
      return date.toLocaleTimeString();
    } catch {
      return isoString;
    }
  };

  return (
    <div className="rounded-lg border border-amber-300 bg-amber-50 p-4 shadow-md" role="alert">
      {/* Header */}
      <div className="mb-3 flex items-start justify-between">
        <div className="flex items-center gap-2">
          <span className="text-2xl">⚡</span>
          <div>
            <h3 className="font-semibold text-amber-900">
              Conflict Resolved (Last-Write-Wins)
            </h3>
            <p className="text-sm text-amber-800">
              Token <code className="font-mono text-amber-900">{conflict.tokenId}</code> was updated
              by another player
            </p>
          </div>
        </div>
        <button
          onClick={onDismiss}
          className="text-amber-600 hover:text-amber-900 focus:outline-none"
          aria-label="Dismiss notification"
        >
          ✕
        </button>
      </div>

      {/* Details Toggle */}
      <button
        onClick={() => {
          setShowDetails(!showDetails);
          if (!showDetails && onViewDetails) {
            onViewDetails();
          }
        }}
        className="mb-2 text-sm font-medium text-amber-700 hover:text-amber-900 underline"
      >
        {showDetails ? '▼' : '▶'} Version Details
      </button>

      {/* Expanded Details */}
      {showDetails && (
        <div className="mt-3 space-y-2 rounded-md bg-amber-100 p-3 text-sm font-mono">
          <div>
            <span className="font-semibold text-amber-900">Your version:</span>
            <span className="ml-2 text-amber-700">{formatTime(versionInfo.clientVersion)}</span>
          </div>
          <div>
            <span className="font-semibold text-amber-900">Server version:</span>
            <span className="ml-2 text-amber-700">{formatTime(versionInfo.serverVersion)}</span>
          </div>
          <div>
            <span className="font-semibold text-amber-900">Time diff:</span>
            <span className="ml-2 text-amber-700">{versionInfo.timeDiffMs}ms</span>
          </div>
          {conflict.appliedData && (
            <div>
              <span className="font-semibold text-amber-900">Applied data:</span>
              <pre className="mt-1 overflow-x-auto rounded bg-white p-2 text-xs text-gray-700">
                {JSON.stringify(conflict.appliedData, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* Action Buttons */}
      <div className="mt-3 flex gap-2">
        <button
          onClick={onDismiss}
          className="inline-flex items-center gap-1 rounded-md bg-amber-200 px-3 py-1.5 text-sm font-medium text-amber-900 hover:bg-amber-300 focus:outline-none"
        >
          ✓ Understood
        </button>
      </div>
    </div>
  );
}

/**
 * Phase 4.9.C.3: Conflict Panel Component
 * 
 * Shows all recent conflicts in a collapsible panel
 */

interface ConflictPanelProps {
  conflicts: ConflictRecord[];
  onDismiss: (eventId: number) => void;
  onClearAll: () => void;
}

export function ConflictPanel({
  conflicts,
  onDismiss,
  onClearAll,
}: ConflictPanelProps) {
  const unresolvedCount = conflicts.filter((c) => !c.dismissed).length;
  const [isOpen, setIsOpen] = useState(unresolvedCount > 0);

  if (conflicts.length === 0) {
    return null;
  }

  return (
    <div className="rounded-lg border border-amber-300 bg-amber-50 p-4">
      <div className="flex items-center justify-between">
        <button
          onClick={() => setIsOpen(!isOpen)}
          className="flex items-center gap-2 font-semibold text-amber-900 hover:text-amber-700"
        >
          {isOpen ? '▼' : '▶'} Conflicts ({unresolvedCount} unresolved)
        </button>
        {unresolvedCount > 0 && (
          <button
            onClick={onClearAll}
            className="text-sm text-amber-600 hover:text-amber-900 underline"
          >
            Clear All
          </button>
        )}
      </div>

      {isOpen && (
        <div className="mt-3 space-y-2">
          {conflicts.map((conflict) => (
            <div
              key={conflict.eventId}
              className={`rounded border p-2 ${
                conflict.dismissed
                  ? 'border-gray-300 bg-gray-100 opacity-50'
                  : 'border-amber-200 bg-white'
              }`}
            >
              <div className="flex items-center justify-between">
                <div className="text-sm">
                  <strong>{conflict.tokenId}</strong>
                  <span className="ml-2 text-xs text-gray-600">
                    {new Date(conflict.conflictTimestamp).toLocaleTimeString()}
                  </span>
                </div>
                {!conflict.dismissed && (
                  <button
                    onClick={() => onDismiss(conflict.eventId)}
                    className="text-xs font-medium text-amber-600 hover:text-amber-900"
                  >
                    Dismiss
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
