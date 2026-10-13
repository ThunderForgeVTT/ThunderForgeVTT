import type { PanelPosition } from "@/components/StatusPanel/StatusPanel";

/**
 * Where the viewer last left a pinned status panel (spec 029 FR-011a). Per
 * viewer, per device. Which token was pinned is deliberately not kept: it may
 * be gone, or the scene changed, by the next visit.
 */
export const PANEL_PIN_KEY = "thunderforge.statusPanel.pin";

export function readSavedPin(): PanelPosition | null {
  try {
    const raw = localStorage.getItem(PANEL_PIN_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PanelPosition>;
    return typeof parsed.x === "number" && typeof parsed.y === "number"
      ? { x: parsed.x, y: parsed.y }
      : null;
  } catch {
    // Storage refused, or a value that is not ours: the panel opens where the
    // double-click is, which is the right answer anyway.
    return null;
  }
}

/**
 * The ground a loading panel stands on, over the play field.
 *
 * The canvas is a flat dark plate the theme knows nothing about, so anything
 * drawn over it has to bring its own background or be white-by-hand and
 * unreadable the moment a light map loads behind it. Sized to fit its
 * contents rather than the `min-h-64` a full-page loader wants.
 */
export const LOADING_PLATE =
  // Opaque, not translucent: axe found the muted step text failing contrast
  // once the dark canvas showed through a 95% plate.
  "min-h-0 max-w-[min(20rem,calc(100vw-2rem))] rounded-xl border border-border bg-background shadow-xl";

/**
 * What is still coming, in words a person at a table would use.
 *
 * Named rather than counted: "3 of 5 resources" is a number about the
 * implementation, while "the map image" is the thing the owner was actually
 * watching for. The map image is always said first and said differently,
 * because it is the large one — a scene whose walls and lights have landed
 * and whose art has not is precisely the moment that read as a freeze.
 */
export function describeSceneLoad(pending: readonly string[]): string {
  const waitingOnMap = pending.includes("background");
  const rest = pending.filter((resource) => resource !== "background");
  if (waitingOnMap && rest.length === 0) {
    return "The map image is on its way — a large map takes a moment.";
  }
  if (waitingOnMap) {
    return `Loading the map image, and the scene's ${rest.join(", ")}.`;
  }
  if (rest.length === 0) {
    return "Putting the last of it on the board.";
  }
  return `Loading the scene's ${rest.join(", ")}.`;
}
