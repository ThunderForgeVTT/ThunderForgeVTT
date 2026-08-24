import React, { useState } from 'react';

/** A single `PlaceholderBindingInput` entry, matching the `rollDice`
 * mutation's `bindings` field shape (`src/server/src/graphql/mutations_roll.rs`'s
 * `PlaceholderBindingInput { name, value }`; see `apps/web/src/types/roll.ts`'s
 * `PlaceholderBinding` for the app-side equivalent). Declared locally
 * rather than imported from `apps/web` so this system pack has no
 * dependency on the host app (the host app depends on system packs, not
 * the reverse). */
export interface ManifestationBinding {
  name: string;
  value: number;
}

/** Whatever a resolved Manifestation roll came back as — deliberately
 * loose (`unknown`) here since the real shape is `GraphQLRollResolution`
 * (spec 014), owned by the host app, not this package. */
export type ManifestationRollResult = unknown;

export interface ManifestationRollButtonProps {
  /** Label for the skill being rolled (display only, e.g. "Cunning (Lockpicking)"). */
  skillLabel: string;
  /** The skill rating — how many d6s go in the pool. */
  skillRating: number;
  /** How many of the pool's dice to keep. Defaults to the full pool
   * (equivalent to no keep/drop) when omitted. */
  keep?: number;
  /** Lets the caller pick the keep count interactively; omit to fix it
   * via the `keep` prop instead. */
  allowKeepSelection?: boolean;
  disabled?: boolean;
  /**
   * Performs the actual `rollDice` GraphQL mutation
   * (`src/server/src/graphql/mutations_roll.rs`, wired client-side via
   * `apps/web/src/api/roll.ts`'s `rollDice(worldId, formula, bindings)`).
   * Injected rather than called directly so this package never imports
   * from `apps/web` — the host app supplies the real mutation call.
   */
  onRoll: (formula: string, bindings: ManifestationBinding[]) => Promise<ManifestationRollResult>;
  onResult?: (result: ManifestationRollResult) => void;
  onError?: (error: unknown) => void;
}

/**
 * Genie Manifestation Roll button — spec 018 User Story 1 (T018).
 *
 * Builds `packs/systems/genie/system.json`'s `manifestationRoll.formula`
 * template for this character's skill rating and chosen keep count, then
 * hands it to the caller-supplied `onRoll` (the actual `rollDice`
 * mutation call).
 *
 * Formula shape: `(skill)d6kh{keep}x=6cs>=4` — `crates/thunderforge-dice`'s
 * real notation (`kh`/`x`/`cs`, confirmed against `parser.rs`), not the
 * illustrative `k`/`!`-style string in the spec's contract doc. `skill`
 * is a genuine placeholder resolved via the mutation's `bindings`
 * (parenthesized so the parser accepts a placeholder in dice-count
 * position); `keep`'s count is substituted directly into the formula
 * text because `kh{n}`'s count is a parse-time literal in this grammar,
 * not a runtime-bound placeholder.
 */
export const ManifestationRollButton: React.FC<ManifestationRollButtonProps> = ({
  skillLabel,
  skillRating,
  keep,
  allowKeepSelection = false,
  disabled = false,
  onRoll,
  onResult,
  onError,
}) => {
  const [keepCount, setKeepCount] = useState<number>(keep ?? skillRating);
  const [rolling, setRolling] = useState(false);

  const effectiveKeep = Math.max(1, Math.min(keepCount, Math.max(skillRating, 1)));

  const handleRoll = async () => {
    if (disabled || rolling || skillRating <= 0) return;
    setRolling(true);
    try {
      const formula = `(skill)d6kh${effectiveKeep}x=6cs>=4`;
      const bindings: ManifestationBinding[] = [{ name: 'skill', value: skillRating }];
      const result = await onRoll(formula, bindings);
      onResult?.(result);
    } catch (error) {
      onError?.(error);
    } finally {
      setRolling(false);
    }
  };

  return (
    <div className="flex items-center gap-2">
      {allowKeepSelection && (
        <label className="flex items-center gap-1 text-sm">
          Keep
          <input
            type="number"
            min={1}
            max={Math.max(skillRating, 1)}
            value={effectiveKeep}
            onChange={(e) => setKeepCount(Number(e.target.value))}
            className="w-14 border rounded text-center"
          />
        </label>
      )}
      <button
        type="button"
        onClick={handleRoll}
        disabled={disabled || rolling || skillRating <= 0}
        className="px-4 py-2 rounded bg-blue-600 text-white font-semibold disabled:opacity-50"
      >
        {rolling ? 'Rolling…' : `Roll ${skillLabel} (${skillRating}d6 kh${effectiveKeep})`}
      </button>
    </div>
  );
};

export default ManifestationRollButton;
