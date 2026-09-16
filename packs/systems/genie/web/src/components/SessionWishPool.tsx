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

/** One wish already spent, and what was asked for. */
export interface SpentWish {
  id: string;
  narrativeEffect: string;
  spentByName?: string | null;
  spentAt?: string | null;
}

export interface SessionWishPoolProps {
  wishesRemaining: number;
  status?: GenieSessionStatus;
  /** Whether the current viewer is this world's GM (only the GM may spend a wish, FR-013/research.md R8). */
  isGm?: boolean;
  /**
   * What this session has already asked for, oldest first.
   *
   * FR-014's Wish Effect used to be write-only: the Game Master typed it,
   * `spendWish` recorded it, and no surface ever showed it back (owner,
   * 2026-09-15 — "the asked-for list doesn't actually show anything").
   * Shown to everyone, not just the GM: a wish is spent by group agreement
   * (FR-013), so the group can see what its agreement bought.
   */
  spentWishes?: SpentWish[];
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
  spentWishes = [],
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

      {/* FR-014: what the table asked for, and got. Above the GM's form on
        * purpose — a Game Master about to spend the last wish should be
        * looking at what the first two bought. */}
      <div className="mt-4 border-t border-border pt-4" data-testid="wish-asked-for">
        <h3 className={sectionHeadingClass}>Asked for</h3>
        {spentWishes.length === 0 ? (
          <p className={`mt-2 ${hintClass}`}>No wishes spent yet.</p>
        ) : (
          <ol className="mt-2 flex flex-col gap-2">
            {spentWishes.map((wish) => (
              <li
                key={wish.id}
                className="rounded-lg border border-border bg-muted/40 p-3"
                data-testid="wish-asked-for-entry"
              >
                <p className="text-sm">{wish.narrativeEffect}</p>
                {wish.spentByName ? (
                  <p className={`mt-1 ${hintClass}`}>Granted by {wish.spentByName}</p>
                ) : null}
              </li>
            ))}
          </ol>
        )}
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
