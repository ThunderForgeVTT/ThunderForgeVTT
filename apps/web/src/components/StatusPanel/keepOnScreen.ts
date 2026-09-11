/**
 * Keeping a pinned status panel reachable — spec 029 FR-011a.
 *
 * In a module of its own rather than beside `StatusPanel`, so that file
 * exports only a component and Vite's fast refresh can hot-swap it.
 */

/** The panel's top-left corner, in viewport pixels. */
export interface PanelPosition {
  x: number;
  y: number;
}

/** How much of the panel must stay on screen, so a drag can never lose it. */
export const KEEP_ON_SCREEN = 48;

/**
 * `position`, moved just enough that at least `KEEP_ON_SCREEN` of the panel —
 * including the top of its header, which is what drags it — stays inside
 * `viewport`. With no viewport to ask (a test, a server render), unchanged.
 */
export function keepOnScreen(
  position: PanelPosition,
  viewport: { width: number; height: number } | null,
): PanelPosition {
  if (!viewport) return position;
  return {
    x: Math.min(
      Math.max(position.x, 0),
      Math.max(0, viewport.width - KEEP_ON_SCREEN),
    ),
    y: Math.min(
      Math.max(position.y, 0),
      Math.max(0, viewport.height - KEEP_ON_SCREEN),
    ),
  };
}
