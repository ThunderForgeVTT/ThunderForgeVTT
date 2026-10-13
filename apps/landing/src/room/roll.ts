import type { Resolution } from "@thunderforge/dice-throw";
import { dice, seed } from "../dice.ts";

// Spec 091 FR-015: the chest's roll, with the crate the tray rolls with and a
// fresh seed. The page prints the total as soon as it has it; the throw on
// the table only shows it.

export async function rollFormula(formula: string): Promise<Resolution> {
  const m = await dice();
  return JSON.parse(m.roll(formula, "{}", seed())) as Resolution;
}
