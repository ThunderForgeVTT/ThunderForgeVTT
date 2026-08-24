import React from 'react';
import { resolveConditions } from '../conditions';

export interface ConditionTrackProps {
  /** The character/NPC's `trait_data.active_conditions` list (raw keys). */
  activeConditions?: string[];
  /**
   * `'sheet'` (default) renders the full condition track for the character
   * sheet: one row per active condition with its label and description.
   * `'token'` renders a compact badge strip suitable for use as a token
   * status indicator — the same `activeConditions` data, laid over/near a
   * token wherever the canvas renders one (spec 018 US4 Acceptance
   * Scenario 1: "appears on the character sheet's condition track and as
   * a token status indicator"). This app has no dedicated token-status
   * overlay component yet to plug into (searched apps/web/src for an
   * existing "condition"/status-effect indicator on tokens — none found;
   * `TokenRecord.metadata` is the only open extension point), so this
   * variant is the reusable unit a future token-overlay integration
   * renders directly, rather than a component wired into the canvas
   * itself.
   */
  variant?: 'sheet' | 'token';
  className?: string;
}

/**
 * Genie ConditionTrack — spec 018 User Story 4 (T051).
 *
 * Resolves `active_conditions` keys (from `trait_data`, validated server-side
 * by `validate_trait_data` in packs/systems/genie/server/src/validators.rs)
 * to their manifest definitions via `resolveConditions` (src/conditions.ts)
 * and renders them either as a full sheet track or a compact token badge
 * strip. An empty/undefined list renders nothing distinct per variant (a
 * "no conditions" row on the sheet, nothing at all for the token so a
 * healthy token isn't cluttered).
 */
export const ConditionTrack: React.FC<ConditionTrackProps> = ({
  activeConditions = [],
  variant = 'sheet',
  className,
}) => {
  const resolved = resolveConditions(activeConditions);

  if (variant === 'token') {
    if (resolved.length === 0) return null;
    return (
      <div
        className={`flex gap-1 ${className ?? ''}`}
        data-testid="genie-condition-track-token"
        role="status"
        aria-label="Active conditions"
      >
        {resolved.map((condition) => (
          <span
            key={condition.key}
            title={`${condition.label}: ${condition.description}`}
            aria-label={condition.label}
            className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-purple-700 text-white text-[10px] font-bold uppercase"
          >
            {condition.label.slice(0, 1)}
          </span>
        ))}
      </div>
    );
  }

  return (
    <div className={`space-y-2 ${className ?? ''}`} data-testid="genie-condition-track-sheet">
      <h3 className="text-sm font-semibold text-gray-600">Conditions</h3>
      {resolved.length === 0 ? (
        <p className="text-sm text-gray-500">No active conditions.</p>
      ) : (
        <ul className="space-y-1">
          {resolved.map((condition) => (
            <li
              key={condition.key}
              className="flex flex-col border rounded px-3 py-2 bg-purple-50"
            >
              <span className="font-semibold">{condition.label}</span>
              <span className="text-sm text-gray-600">{condition.description}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export default ConditionTrack;
