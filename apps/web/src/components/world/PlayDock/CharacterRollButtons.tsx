import { RollResult } from "@/components/world/RollResult";
import { RollVisibilityPicker } from "@/components/world/RollVisibility/RollVisibilityPicker";
import { cn } from "@/lib/utils";

import type { CharacterRoll } from "./characterRolls";
import type { CharacterRoller } from "./useCharacterRoller";

export interface CharacterRollButtonsProps {
  rolls: CharacterRoll[];
  roller: CharacterRoller;
  /** `in-pane` in the play dock, `sheet` on the sheet page. */
  testIdPrefix: string;
  /** Thumb-sized rows, for the phone's sheet (spec 102). */
  touch?: boolean;
  /**
   * Opens the attack flow for an attack entry. Left out where there is no
   * board to pick a target on, and the entry then says so (FR-014).
   */
  onAttack?: (roll: CharacterRoll) => void;
  /** Why an attack cannot be made here even with a board, if it cannot. */
  attackUnavailable?: string | null;
}

/**
 * Spec 081: a character's rolls, shared by the in-pane sheet and the sheet
 * page. A roll goes out through `rollDice` with its label and the picked
 * visibility, and reaches every board as a world event — this sheet animates
 * nothing itself, which is what lets a sheet in another tab roll onto the
 * play view. The total is shown by `RollFeedback` from the mutation's own
 * answer.
 */
export function CharacterRollButtons({
  rolls,
  roller,
  testIdPrefix,
  touch = false,
  onAttack,
  attackUnavailable = null,
}: CharacterRollButtonsProps) {
  const handleRoll = (roll: CharacterRoll) => {
    // Spec 046: an attack roll is an attack, aimed at something, rolled and
    // judged by the server through `makeAttack`.
    if (roll.attackAbilityId) {
      onAttack?.(roll);
      return;
    }
    void roller.roll(roll.key, roll.label, roll.formula);
  };

  /** Why an attack entry cannot be used here, or `null` if it can. */
  const attackBlocked = (roll: CharacterRoll): string | null => {
    if (roll.attackAbilityId === undefined) return null;
    if (!onAttack) return "Pick the target on the board";
    return attackUnavailable;
  };

  return (
    <ul className="grid gap-1" data-testid={`${testIdPrefix}-roll-list`}>
      {rolls.map((roll) => {
        const blocked = attackBlocked(roll);
        return (
          <li key={roll.key}>
            <button
              type="button"
              disabled={roller.rolling !== null || blocked !== null}
              aria-busy={roller.rolling === roll.key}
              title={blocked ?? undefined}
              onClick={() => handleRoll(roll)}
              data-testid={`${testIdPrefix}-roll-${roll.key}`}
              data-attack={roll.attackAbilityId ? "true" : undefined}
              className={cn(
                "flex w-full items-center gap-2 border border-border text-left transition-colors hover:bg-muted disabled:opacity-60",
                touch
                  ? "min-h-11 touch-manipulation rounded-lg px-3 text-sm active:bg-muted"
                  : "rounded px-2 py-1 text-xs",
              )}
            >
              <span className="min-w-0 flex-1 truncate">{roll.label}</span>
              <span className="text-muted-foreground tabular-nums">
                {roller.rolling === roll.key
                  ? "Rolling…"
                  : !onAttack && blocked
                    ? blocked
                    : roll.formula}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

/** Who sees the next roll, and what the last one came to. */
export function RollFeedback({
  roller,
  testIdPrefix,
}: {
  roller: CharacterRoller;
  testIdPrefix: string;
}) {
  return (
    <>
      <RollVisibilityPicker
        isGm={roller.isGm}
        value={roller.visibility}
        onChange={roller.setVisibility}
      />

      {roller.rollError ? (
        <p
          className="text-xs text-destructive"
          data-testid={`${testIdPrefix}-roll-error`}
        >
          {roller.rollError}
        </p>
      ) : null}

      {roller.result ? (
        // `RollResult` is the same renderer the dice roller uses, so a roll
        // made from a character reads exactly like one made from the roller —
        // including the Game Master's discrepancy note, which is deliberately
        // never shown to the player rolling (spec 028 FR-067).
        <div
          className="flex flex-wrap items-baseline gap-x-2"
          data-testid={`${testIdPrefix}-roll-result`}
        >
          {roller.resultLabel ? (
            <span className="text-sm font-medium">{roller.resultLabel}</span>
          ) : null}
          <RollResult resolution={roller.result} />
        </div>
      ) : null}
    </>
  );
}
