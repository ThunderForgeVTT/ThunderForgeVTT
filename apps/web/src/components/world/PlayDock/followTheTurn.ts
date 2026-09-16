/**
 * Whether this person's camera follows the turn (owner decision 2026-09-15).
 *
 * # Per person, not per table
 *
 * "That should be a toggleable feature" — and it is one person's answer, not
 * the encounter's. A Game Master watching the whole board and a player waiting
 * for their turn want different things out of the same fight, and neither
 * should be able to move the other's camera. So it is remembered in this
 * browser and never sent anywhere.
 *
 * # Off by default
 *
 * A camera that moves on its own is startling the first time, and the first
 * time is always mid-fight. Following is something you turn on once you know
 * what it does; it is not something that should happen to you. The toggle sits
 * in the combat tracker, where the turns are, so finding it costs nothing.
 *
 * # Storage
 *
 * `localStorage` throws outright in a private window with site data blocked,
 * and comes back empty after a clear, so every read and write is guarded and
 * a failure means "off" — the same answer as never having set it.
 */

const STORAGE_PREFIX = "thunderforge.followTheTurn.";

/** Per world: a person may follow in one campaign and not another. */
function key(worldId: string): string {
  return `${STORAGE_PREFIX}${worldId}`;
}

export function readFollowTheTurn(worldId: string): boolean {
  try {
    return window.localStorage.getItem(key(worldId)) === "on";
  } catch {
    return false;
  }
}

export function writeFollowTheTurn(worldId: string, enabled: boolean): void {
  try {
    window.localStorage.setItem(key(worldId), enabled ? "on" : "off");
  } catch {
    // A browser that will not store it still follows for this session; the
    // preference is a convenience, not state anything depends on.
  }
}
