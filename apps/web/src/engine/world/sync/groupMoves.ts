/**
 * Spec 085: one message for a group's answers.
 *
 * The engine sends each member of a moved, deleted or hidden group as its own
 * change, stamped with the group's id and the number it sent. Each bridge
 * settles the stamp as its answer arrives; once all of them are in, any
 * refusals are reported together — "2 of 5 could not be moved." — rather
 * than as a toast per member.
 */

import { toast } from "sonner";

import type { GroupStamp } from "../types";

export type GroupVerb = "moved" | "deleted" | "hidden";

/** A tally nobody finishes is forgotten after this, so it cannot leak. */
const STALE_AFTER_MS = 30_000;

type Tally = {
  answered: number;
  refused: number;
  timer: ReturnType<typeof setTimeout>;
};

const tallies = new Map<string, Tally>();

/** Count one answer for `stamp`. An answer without a stamp is not a group's. */
export function settleGroup(
  stamp: GroupStamp | undefined,
  ok: boolean,
  verb: GroupVerb,
): void {
  if (!stamp) {
    return;
  }

  let tally = tallies.get(stamp.id);
  if (!tally) {
    tally = {
      answered: 0,
      refused: 0,
      timer: setTimeout(() => tallies.delete(stamp.id), STALE_AFTER_MS),
    };
    tallies.set(stamp.id, tally);
  }

  tally.answered += 1;
  if (!ok) {
    tally.refused += 1;
  }

  if (tally.answered < stamp.size) {
    return;
  }

  clearTimeout(tally.timer);
  tallies.delete(stamp.id);
  // The board's own refusals were never sent, so they have no answer to
  // wait for; they count toward the one notice all the same.
  const heldBack = stamp.refused ?? 0;
  const refused = tally.refused + heldBack;
  if (refused > 0) {
    toast.warning(
      `${refused} of ${stamp.size + heldBack} could not be ${verb}.`,
    );
  }
}

/** For tests: forget every open tally. */
export function forgetAllGroups(): void {
  for (const tally of tallies.values()) {
    clearTimeout(tally.timer);
  }
  tallies.clear();
}

let localGroupCounter = 0;

/**
 * A stamp for a group the page makes itself — hiding several walls — rather
 * than one the engine stamped.
 */
export function localGroupStamp(size: number): GroupStamp {
  localGroupCounter += 1;
  return { id: `local-${localGroupCounter}`, size };
}
