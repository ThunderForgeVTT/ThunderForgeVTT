/**
 * Whether the canvas shows its performance and connection readout.
 *
 * # Why a service rather than component state
 *
 * The readout lives on the canvas overlay and the switch that controls it
 * lives in the Play dock's Settings panel, which is unmounted whenever the
 * dock is collapsed — which is most of the time. State owned by either one
 * would be destroyed by the other's ordinary lifecycle. It also has to
 * persist across reloads, because a preference about what your screen looks
 * like that resets every session is not a preference.
 *
 * # Off by default
 *
 * The opposite call from `peerTransfer`, and for the opposite reason: that
 * one is a disclosure the user is owed whether or not they asked for it,
 * while this is diagnostics. Numbers nobody asked for sitting permanently
 * over the map are exactly the "massive banner" this is meant not to be.
 * Someone who wants them turns them on once and they stay on.
 */

const STORAGE_KEY = "thunderforge:engine-monitor-visible";

type Listener = (visible: boolean) => void;

const listeners = new Set<Listener>();

function readVisible(): boolean {
  try {
    // Absent means never set, which is the default: off.
    return window.localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    // A profile with site data blocked still draws a canvas.
    return false;
  }
}

let visible = typeof window === "undefined" ? false : readVisible();

/** Whether the readout should be on screen. */
export function isEngineMonitorVisible(): boolean {
  return visible;
}

/** Show or hide the readout, and remember which. */
export function setEngineMonitorVisible(next: boolean): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, next ? "true" : "false");
  } catch {
    // Applies to this session; it just will not persist.
  }
  if (next === visible) return;
  visible = next;
  for (const listener of listeners) listener(visible);
}

/** Observe the setting. Returns an unsubscribe. */
export function subscribeToEngineMonitor(listener: Listener): () => void {
  listeners.add(listener);
  listener(visible);
  return () => {
    listeners.delete(listener);
  };
}

/** Reset module state. Tests only. */
export function resetEngineMonitorForTests(): void {
  listeners.clear();
  visible = false;
}
