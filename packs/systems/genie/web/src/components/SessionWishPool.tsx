import React, { useState } from 'react';
import {
  cardClass,
  cardTitleClass,
  hintClass,
  primaryButtonClass,
  sectionHeadingClass,
  textareaClass,
} from './styles';

/**
 * Spec 018 (Genie) User Story 7 — the Session Wish Pool (FR-013).
 *
 * Props-driven, like this pack's other components (`CharacterSheet.tsx`):
 * the host page owns the GraphQL `genieSession(worldId)` query and the
 * `spendWish` mutation (`contracts/genie-session-loop.md`); this
 * component only renders the pool and, for the GM, a control that calls
 * back with the GM-adjudicated narrative effect text (FR-014).
 */

export type GenieSessionStatus = 'ACTIVE' | 'WON' | 'LOST';

export interface SessionWishPoolProps {
  wishesRemaining: number;
  status?: GenieSessionStatus;
  /** Whether the current viewer is this world's GM (only the GM may spend a wish, FR-013/research.md R8). */
  isGm?: boolean;
  /** Called with the GM-authored narrative effect (FR-014) when "Spend a Wish" is confirmed. */
  onSpendWish?: (narrativeEffect: string) => void | Promise<void>;
}

const TOTAL_WISHES = 3;

/**
 * Renders as `wishesRemaining` filled wish icons out of `TOTAL_WISHES`
 * (the Session Wish Pool always starts at 3, FR-013) plus, for the GM,
 * an inline form to spend one.
 */
export const SessionWishPool: React.FC<SessionWishPoolProps> = ({
  wishesRemaining,
  status = 'ACTIVE',
  isGm = false,
  onSpendWish,
}) => {
  const [narrativeEffect, setNarrativeEffect] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const canSpend = isGm && status === 'ACTIVE' && wishesRemaining > 0 && !!onSpendWish;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSpend || !narrativeEffect.trim()) return;
    setSubmitting(true);
    try {
      await onSpendWish?.(narrativeEffect.trim());
      setNarrativeEffect('');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className={cardClass} data-testid="session-wish-pool">
      <h2 className={cardTitleClass}>Session Wish Pool</h2>
      <div
        className="mt-3 flex items-center gap-2"
        aria-label={`${wishesRemaining} of ${TOTAL_WISHES} wishes remaining`}
      >
        {Array.from({ length: TOTAL_WISHES }).map((_, i) => (
          <span
            key={i}
            className={`text-2xl leading-none ${i < wishesRemaining ? 'text-violet-500' : 'text-muted-foreground/25'}`}
            aria-hidden="true"
          >
            ✦
          </span>
        ))}
        <span className={`ml-1 ${hintClass}`}>
          {wishesRemaining} / {TOTAL_WISHES} remaining
        </span>
      </div>

      {isGm && (
        <form onSubmit={handleSubmit} className="mt-4 flex flex-col gap-2 border-t border-border pt-4">
          {/* FR-014: a Wish is GM-adjudicated narration, never a dice roll —
            * hence free text plus the hint below, not a roll control. */}
          <label htmlFor="wish-narrative-effect" className={sectionHeadingClass}>
            Wish Effect
          </label>
          <p className={hintClass}>Narrated by the GM — not a dice roll.</p>
          <textarea
            id="wish-narrative-effect"
            className={textareaClass}
            rows={2}
            placeholder="e.g. Undo that failed roll's consequence, reveal a hidden clue, remove an obstacle..."
            value={narrativeEffect}
            onChange={(e) => setNarrativeEffect(e.target.value)}
            disabled={!canSpend || submitting}
          />
          <button
            type="submit"
            className={`self-start ${primaryButtonClass}`}
            disabled={!canSpend || submitting || !narrativeEffect.trim()}
          >
            {submitting ? 'Spending…' : 'Spend a Wish'}
          </button>
          {wishesRemaining === 0 && (
            <p className="text-sm text-destructive">No wishes remaining in the pool.</p>
          )}
        </form>
      )}
    </div>
  );
};

export default SessionWishPool;
