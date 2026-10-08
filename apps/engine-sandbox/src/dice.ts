/**
 * Spec 083: throw a roll on the board, for tuning the dice by eye.
 *
 * The roll is made with the dice crate itself (`@thunderforge/dice`, the
 * same wasm the demo rolls with), then sent through `apply_world_command`
 * in the shape `apps/web`'s `buildDiceThrow` sends: the whole `WorldRoll`,
 * every die's `rolls`, `steps`, `kept` and `finalValue`. Nothing here
 * totals or decides a face.
 */
import initDice, { roll } from "@thunderforge/dice";

/** `thunderforge_dice::RollResolution`, as serde writes it. */
interface Resolution {
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

/** The `trigger_dice_roll` command for `formula`, rolled with `seed`. */
export function diceCommand(
  formula: string,
  seed: Uint32Array,
): Record<string, unknown> {
  const detail = JSON.parse(roll(formula, "{}", seed)) as Resolution;
  const total = "Total" in detail.kind;
  return {
    type: "trigger_dice_roll",
    roll: {
      id: crypto.randomUUID(),
      rollerName: "Sandbox",
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

/** Wires the formula field, the Roll button and the reduced-motion box. */
export async function wireDice(
  send: (command: Record<string, unknown>) => void,
  log: (message: string) => void,
): Promise<void> {
  await initDice();
  const formula = document.getElementById("dice-formula") as HTMLInputElement;
  const button = document.getElementById("dice-roll") as HTMLButtonElement;
  const reduced = document.getElementById("dice-reduced") as HTMLInputElement;
  button.addEventListener("click", () => {
    try {
      const command = diceCommand(
        formula.value,
        crypto.getRandomValues(new Uint32Array(4)),
      );
      send(command);
      const thrown = command.roll as { resultValue: number };
      log(`trigger_dice_roll → ${formula.value} = ${thrown.resultValue}`);
    } catch (error) {
      log(`roll refused: ${(error as Error).message ?? error}`);
    }
  });
  reduced.addEventListener("change", () => {
    send({ type: "set_reduced_motion", reduced: reduced.checked });
    log(`set_reduced_motion → ${reduced.checked}`);
  });
}
