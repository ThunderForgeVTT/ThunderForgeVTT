import type { WorldRollRecord } from "../../types/roll";
import { buildDiceThrow } from "./diceThrow";
import { dicePlayed } from "./engineProbe";
import { getWasmModule } from "./wasmModule";

/**
 * Spec 083: throws a roll the server already resolved onto the board. This
 * never asks the engine to decide an outcome (FR-015). Not a
 * `WorldCommand`/`worldStore` event: like `setIsGameMaster`, it is a
 * one-shot local trigger, not world state to sync or broadcast.
 */
export async function triggerDiceRollAnimation(
  roll: WorldRollRecord,
): Promise<void> {
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "trigger_dice_roll", roll: buildDiceThrow(roll) }),
  );
  dicePlayed.push(roll.resolution.dice.map((die) => die.finalValue));
}

/** Spec 083: whether the viewer prefers reduced motion, for the next throw. */
export async function setReducedMotion(reduced: boolean): Promise<void> {
  const module = await getWasmModule();
  module.apply_world_command?.(
    JSON.stringify({ type: "set_reduced_motion", reduced }),
  );
}
