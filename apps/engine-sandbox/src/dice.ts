/**
 * Spec 083: throw a roll on the board, for tuning the dice by eye.
 *
 * The roll is made with the dice crate itself (`@thunderforge/dice`, the
 * same wasm the demo rolls with), then sent through `apply_world_command`
 * as `@thunderforge/dice-throw` builds it, which the landing shares.
 * Nothing here totals or decides a face.
 */
import initDice, { roll } from "@thunderforge/dice";
import { diceCommand, type Resolution } from "@thunderforge/dice-throw";

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
      const detail = JSON.parse(
        roll(formula.value, "{}", crypto.getRandomValues(new Uint32Array(4))),
      ) as Resolution;
      const command = diceCommand(detail, { rollerName: "Sandbox" });
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
