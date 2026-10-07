import { useState } from "react";

import { rollDice } from "@/api/roll";
import { RollResult } from "@/components/world/RollResult";
import { RollVisibilityPicker } from "@/components/world/RollVisibility/RollVisibilityPicker";
import { useRollVisibility } from "@/components/world/RollVisibility/useRollVisibility";
import type { RollResolutionRecord } from "@/types/roll";

import type { CharacterRoll } from "./characterRolls";

export interface CharacterRollButtonsProps {
  worldId: string;
  rolls: CharacterRoll[];
  isGm: boolean;
  /** `in-pane` in the play dock, `sheet` on the sheet page. */
  testIdPrefix: string;
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
 * play view. The total is shown here from the mutation's own answer.
 */
export function CharacterRollButtons({
  worldId,
  rolls,
  isGm,
  testIdPrefix,
  onAttack,
  attackUnavailable = null,
}: CharacterRollButtonsProps) {
  const [visibility, setVisibility] = useRollVisibility(isGm);
  const [rolling, setRolling] = useState<string | null>(null);
  const [result, setResult] = useState<RollResolutionRecord | null>(null);
  const [rollError, setRollError] = useState<string | null>(null);

  const handleRoll = async (roll: CharacterRoll) => {
    // Spec 046: an attack roll is an attack, aimed at something, rolled and
    // judged by the server through `makeAttack`.
    if (roll.attackAbilityId) {
      onAttack?.(roll);
      return;
    }
    setRolling(roll.key);
    setRollError(null);
    setResult(null);
    try {
      setResult(
        await rollDice(worldId, roll.formula, {
          label: roll.label,
          visibility,
        }),
      );
    } catch (error) {
      setRollError(
        error instanceof Error ? error.message : "Failed to roll dice",
      );
    } finally {
      setRolling(null);
    }
  };

  /** Why an attack entry cannot be used here, or `null` if it can. */
  const attackBlocked = (roll: CharacterRoll): string | null => {
    if (roll.attackAbilityId === undefined) return null;
    if (!onAttack) return "Pick the target on the board";
    return attackUnavailable;
  };

  return (
    <>
      <ul className="grid gap-1">
        {rolls.map((roll) => {
          const blocked = attackBlocked(roll);
          return (
            <li key={roll.key}>
              <button
                type="button"
                disabled={rolling !== null || blocked !== null}
                title={blocked ?? undefined}
                onClick={() => void handleRoll(roll)}
                data-testid={`${testIdPrefix}-roll-${roll.key}`}
                data-attack={roll.attackAbilityId ? "true" : undefined}
                className="flex w-full items-center gap-2 rounded border border-border px-2 py-1 text-left text-xs transition-colors hover:bg-muted disabled:opacity-60"
              >
                <span className="min-w-0 flex-1 truncate">{roll.label}</span>
                <span className="text-muted-foreground tabular-nums">
                  {rolling === roll.key
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

      <RollVisibilityPicker
        isGm={isGm}
        value={visibility}
        onChange={setVisibility}
      />

      {rollError ? (
        <p
          className="text-xs text-destructive"
          data-testid={`${testIdPrefix}-roll-error`}
        >
          {rollError}
        </p>
      ) : null}

      {result ? (
        // `RollResult` is the same renderer the dice roller uses, so a roll
        // made from a character reads exactly like one made from the roller —
        // including the Game Master's discrepancy note, which is deliberately
        // never shown to the player rolling (spec 028 FR-067).
        <div data-testid={`${testIdPrefix}-roll-result`}>
          <RollResult resolution={result} />
        </div>
      ) : null}
    </>
  );
}
