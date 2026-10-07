/**
 * Spec 081 research R8: who a roll is for, remembered per browser so a player
 * who rolls for the GM's eyes all evening does not pick it every time.
 *
 * A convenience only. Storage can be missing or throw (a private window,
 * blocked site data), and then every roll is simply in the open until the
 * player picks otherwise; the server decides who may see what regardless.
 */

import type { RollVisibility } from "@/types/roll";

export const ROLL_VISIBILITY_KEY = "thunderforge.rollVisibility";

/** What each role may choose. A player never hides a roll from the GM. */
export function choicesFor(isGm: boolean): RollVisibility[] {
  return isGm ? ["EVERYONE", "GM_ONLY"] : ["EVERYONE", "GM_EYES"];
}

export const CHOICE_LABEL: Record<RollVisibility, string> = {
  EVERYONE: "Everyone",
  GM_EYES: "GM's eyes",
  GM_ONLY: "GM only",
};

/**
 * The stored choice if this role may make it, else `EVERYONE`. One key for
 * both roles: a GM who is a player at another table gets that table's default
 * rather than a choice it would refuse.
 */
export function readChoice(isGm: boolean, storage?: Storage): RollVisibility {
  try {
    const stored = (storage ?? window.localStorage).getItem(
      ROLL_VISIBILITY_KEY,
    );
    const allowed = choicesFor(isGm);
    return allowed.find((choice) => choice === stored) ?? "EVERYONE";
  } catch {
    return "EVERYONE";
  }
}

export function writeChoice(choice: RollVisibility, storage?: Storage): void {
  try {
    (storage ?? window.localStorage).setItem(ROLL_VISIBILITY_KEY, choice);
  } catch {
    // Not remembered; this roll still goes as chosen.
  }
}
