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
  /**
   * `list` stacks the rolls; `hotbar` lays them in one row of thumb-sized
   * chips that scrolls sideways, for the phone's sheet (spec 102).
   */
  layout?: "list" | "hotbar";
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
  layout = "list",
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

  const hotbar = layout === "hotbar";
  return (
    <ul
      className={
        hotbar
          ? "-mx-4 flex snap-x gap-2 overflow-x-auto overscroll-x-contain px-4 pb-1"
          : "grid gap-1"
      }
      data-testid={`${testIdPrefix}-roll-list`}
      data-layout={layout}
    >
      {rolls.map((roll) => {
        const blocked = attackBlocked(roll);
        return (
          <li key={roll.key} className={hotbar ? "shrink-0 snap-start" : ""}>
            <button
              type="button"
              disabled={roller.rolling !== null || blocked !== null}
              title={blocked ?? undefined}
              onClick={() => handleRoll(roll)}
              data-testid={`${testIdPrefix}-roll-${roll.key}`}
              data-attack={roll.attackAbilityId ? "true" : undefined}
              className={cn(
                "border border-border text-left transition-colors hover:bg-muted disabled:opacity-60",
                hotbar
                  ? "flex min-h-11 max-w-48 flex-col justify-center rounded-xl px-3 py-1 text-sm active:bg-muted"
                  : "flex w-full items-center gap-2 rounded px-2 py-1 text-xs",
                hotbar && roll.attackAbilityId && "border-primary/50",
              )}
            >
              <span className="min-w-0 flex-1 truncate">{roll.label}</span>
              <span
                className={cn(
                  "text-muted-foreground tabular-nums",
                  hotbar && "text-xs",
                )}
              >
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
        <div data-testid={`${testIdPrefix}-roll-result`}>
          <RollResult resolution={roller.result} />
        </div>
      ) : null}
    </>
  );
}
