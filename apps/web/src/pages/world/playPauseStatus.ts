import type { PlayPauseSpan, WorldPlayState } from "@/api/playPause";

/**
 * Spec 051 US5 (FR-050, SC-004): what a world's members are told about a
 * pause outside the notice — *that* play was paused, by an operator, and
 * *when*. Never why, never who.
 *
 * Everything here is built from `worldPlayState` alone, whose type has no
 * field a reason, a trigger or an operator could be put in. These helpers
 * could not leak one if they tried; keeping them to times is what keeps the
 * surfaces that use them honest.
 */

/** How long a moment is written: in full on a page, briefly on a card. */
export type MomentLength = "long" | "short";

/** A moment as the reader writes it, in their own locale and time zone. */
export function formatPauseMoment(
  iso: string,
  length: MomentLength = "long",
): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: length === "long" ? "long" : "medium",
    timeStyle: "short",
  });
}

/**
 * When the pause in force began, or `null` when play is not paused.
 *
 * `pausedAt` is the contract's answer. The open span in the history is the
 * fallback, so a state read mid-change still says *since when* rather than
 * dropping the time.
 */
export function pausedSince(state: WorldPlayState | null): string | null {
  if (!state?.paused) return null;
  return (
    state.pausedAt ??
    state.history.find((span) => span.liftedAt === null)?.pausedAt ??
    null
  );
}

/**
 * The world's pauses, newest first, for its settings.
 *
 * The server already sends them newest first. They are ordered again here
 * because a history read out of order would tell a Game Master a false
 * story, and ordering costs nothing next to that.
 */
export function pauseHistoryRows(
  state: WorldPlayState | null,
): PlayPauseSpan[] {
  if (!state) return [];
  return [...state.history].sort(
    (a, b) => Date.parse(b.pausedAt) - Date.parse(a.pausedAt),
  );
}
