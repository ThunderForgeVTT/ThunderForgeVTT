/**
 * Spec 090 (moved from the engine sandbox, spec 083): a roll the dice crate
 * resolved, as the engine's `trigger_dice_roll` command.
 *
 * The command carries the whole `WorldRoll` in the shape `apps/web`'s
 * `buildDiceThrow` sends: every die's `rolls`, `steps`, `kept` and
 * `finalValue`. The engine animates exactly those faces, so the thrown dice
 * show the roll the page already printed. Nothing here rolls, totals or
 * decides a face; it only renames.
 */

/** `thunderforge_dice::RollResolution`, as serde writes it. */
export interface Resolution {
  formula: string;
  dice: Array<{
    sides: { Numeric: number } | "Fate" | "Coin";
    rolls: number[];
    steps?: Array<"Reroll" | "Explode">;
    kept: boolean;
    final_value: number;
  }>;
  kind: { Total: number } | { SuccessCount: number };
}

export interface ThrowOptions {
  /** Shown by the engine beside the dice. */
  rollerName: string;
  /** A fresh id when left out. */
  id?: string;
}

/** The `trigger_dice_roll` command for one resolved roll. */
export function diceCommand(
  detail: Resolution,
  { rollerName, id = crypto.randomUUID() }: ThrowOptions,
): Record<string, unknown> {
  const total = "Total" in detail.kind;
  return {
    type: "trigger_dice_roll",
    roll: {
      id,
      rollerName,
      label: null,
      formula: detail.formula,
      bindings: [],
      resultKind: total ? "TOTAL" : "SUCCESS_COUNT",
      resultValue:
        "Total" in detail.kind ? detail.kind.Total : detail.kind.SuccessCount,
      dice: detail.dice.map((die) => ({
        sidesKind:
          typeof die.sides === "object" ? "NUMERIC" : die.sides.toUpperCase(),
        numericSides: typeof die.sides === "object" ? die.sides.Numeric : null,
        rolls: die.rolls,
        steps: (die.steps ?? []).map((step) =>
          step === "Explode" ? "EXPLODE" : "REROLL",
        ),
        kept: die.kept,
        finalValue: die.final_value,
      })),
    },
  };
}
