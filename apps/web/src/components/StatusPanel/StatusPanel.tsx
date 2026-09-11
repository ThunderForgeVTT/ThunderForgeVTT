import React from "react";
import type { Disclosed } from "@/engine/sdk/Disclosed";
import type { ResourceDefinition } from "@/engine/sdk/ResourceDefinition";
import "@/styles/StatusPanel.scss";
import { keepOnScreen, type PanelPosition } from "./keepOnScreen";

export type { PanelPosition } from "./keepOnScreen";

/**
 * A token's resources, pinned on screen — spec 029 FR-011a, playtest
 * 2026-09-10 P6.
 *
 * Selecting a token no longer opens this: the token's own bars, drawn above
 * it by the engine, are its display (FR-010a). A viewer who wants the figures
 * in words, out of the way of the map, double-clicks the token, and this is
 * pinned — dragged by its header wherever it does not cover what they are
 * reading, kept while they select other things, and closed when they are
 * done (FR-012a).
 *
 * # Why this is React and the token bars are not
 *
 * Constitution Principle I splits on spatial versus screen-space. A bar above
 * a token tracks its position and scales with the camera, so the engine draws
 * it. This panel is text at a place on the screen, and drawing it in WebGL
 * would mean reimplementing text layout, focus handling and screen-reader
 * support to obey a principle that explicitly permits panels in React. See
 * ADR-053.
 *
 * What keeps this from becoming a second source of truth: it **reads** the
 * resolved status the engine already holds and computes nothing. No values are
 * derived here, no disclosure decision is made here, and there is nothing this
 * component could do to widen what a viewer sees — the coarsening happened on
 * the server, and a withheld figure is not in the browser at all.
 *
 * # No hooks, on purpose
 *
 * Position lives with whoever pins the panel, and a drag is a pointer capture
 * on the header with listeners that remove themselves — so this stays a pure
 * function of its props, which is what lets its tests render it without a DOM.
 */

export interface PanelResource {
  definition: ResourceDefinition;
  disclosed: Disclosed;
}

export interface StatusPanelProps {
  /** Resources for the pinned token, or null when nothing is pinned. */
  resources: PanelResource[] | null;
  /** Name shown as the panel's heading. */
  title?: string;
  position: PanelPosition;
  /** Where a drag or a nudge has put the panel. Omitted: it cannot move. */
  onMove?: (position: PanelPosition) => void;
  /** Unpin. Omitted: no close control. */
  onClose?: () => void;
}

/** Pixels an arrow key moves the panel; with Shift, four times that. */
const NUDGE = 16;

function currentViewport(): { width: number; height: number } | null {
  return typeof window === "undefined"
    ? null
    : { width: window.innerWidth, height: window.innerHeight };
}

/** Human-readable summary of one resource, given what we are allowed to know. */
function describe(disclosed: Disclosed): {
  text: string;
  fraction: number | null;
  exact: boolean;
} {
  switch (disclosed.disclosure) {
    case "visible": {
      const current = disclosed.entries.reduce((sum, e) => sum + e.current, 0);
      const max = disclosed.entries.reduce((sum, e) => sum + (e.max ?? 0), 0);
      // A resource with no maximum anywhere is a counter, not a bar, and
      // showing "3 / 0" would be nonsense.
      if (max <= 0) {
        return { text: String(current), fraction: null, exact: true };
      }
      return {
        text: `${current} / ${max}`,
        fraction: current / max,
        exact: true,
      };
    }
    case "percentage":
      // Deliberately no absolute figures: none were sent.
      return {
        text: `${Math.round(disclosed.proportion * 100)}%`,
        fraction: disclosed.proportion,
        exact: false,
      };
    case "chunked":
      return {
        text: `${disclosed.quarter} of 4`,
        fraction: disclosed.quarter / 4,
        exact: false,
      };
    case "greyed":
      // Said in words rather than shown as an empty bar. "Unknown" and "zero"
      // are different facts, and a blank bar asserts the second.
      return { text: "Not disclosed", fraction: null, exact: false };
  }
}

/**
 * Follow a pointer from `event` until it lets go, reporting where the panel
 * should be. Captured on the header, so the pointer can leave it — or the
 * window — mid-drag without the drag being dropped.
 */
function startDrag(
  event: React.PointerEvent<HTMLElement>,
  from: PanelPosition,
  onMove: (position: PanelPosition) => void,
): void {
  if (event.button !== 0) return;
  // A press on the close button is a click, not a drag.
  if ((event.target as HTMLElement).closest("button")) return;
  event.preventDefault();
  const handle = event.currentTarget;
  const start = { x: event.clientX, y: event.clientY };
  handle.setPointerCapture(event.pointerId);

  const move = (moved: PointerEvent) => {
    onMove(
      keepOnScreen(
        {
          x: from.x + moved.clientX - start.x,
          y: from.y + moved.clientY - start.y,
        },
        currentViewport(),
      ),
    );
  };
  const end = () => {
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", end);
    handle.removeEventListener("pointercancel", end);
  };
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
}

/** The arrow keys move the panel too — a drag is not the only way to move it. */
function nudge(
  event: React.KeyboardEvent<HTMLElement>,
  from: PanelPosition,
  onMove: (position: PanelPosition) => void,
): void {
  const step = event.shiftKey ? NUDGE * 4 : NUDGE;
  const delta: Record<string, [number, number]> = {
    ArrowLeft: [-step, 0],
    ArrowRight: [step, 0],
    ArrowUp: [0, -step],
    ArrowDown: [0, step],
  };
  const d = delta[event.key];
  if (!d) return;
  event.preventDefault();
  onMove(
    keepOnScreen({ x: from.x + d[0], y: from.y + d[1] }, currentViewport()),
  );
}

export function StatusPanel({
  resources,
  title,
  position,
  onMove,
  onClose,
}: StatusPanelProps): React.ReactElement | null {
  // Nothing pinned means no panel — not an empty frame, which would read as
  // "this token has nothing left".
  if (!resources || resources.length === 0) {
    return null;
  }

  const ordered = [...resources].sort(
    (a, b) => a.definition.order - b.definition.order,
  );

  return (
    <aside
      className="status-panel"
      aria-label="Pinned token status"
      data-testid="status-panel"
      style={{ left: position.x, top: position.y }}
    >
      <header
        className={
          onMove
            ? "status-panel__head status-panel__drag"
            : "status-panel__head"
        }
        aria-label={onMove ? "Move status panel" : undefined}
        tabIndex={onMove ? 0 : undefined}
        onPointerDown={
          onMove ? (event) => startDrag(event, position, onMove) : undefined
        }
        onKeyDown={
          onMove ? (event) => nudge(event, position, onMove) : undefined
        }
      >
        <h2 className="status-panel__title">{title ?? "Token"}</h2>
        {onClose && (
          <button
            type="button"
            className="status-panel__close"
            aria-label="Unpin status panel"
            onClick={onClose}
          >
            ×
          </button>
        )}
      </header>

      <dl className="status-panel__resources">
        {ordered.map((resource) => {
          const { text, fraction, exact } = describe(resource.disclosed);
          return (
            <div
              className="status-panel__resource"
              key={resource.definition.id}
            >
              <dt>{resource.definition.label}</dt>
              <dd
                className={
                  exact
                    ? "status-panel__value"
                    : "status-panel__value status-panel__value--approximate"
                }
                // An estimate is announced as one. A screen reader user should
                // not be told "2 of 4" as though it were a reading.
                aria-label={
                  exact
                    ? `${resource.definition.label}: ${text}`
                    : `${resource.definition.label}: approximately ${text}`
                }
              >
                {text}
              </dd>
              {fraction !== null && (
                <div
                  className="status-panel__track"
                  role="meter"
                  aria-valuenow={Math.round(fraction * 100)}
                  aria-valuemin={0}
                  aria-valuemax={100}
                >
                  <div
                    className={
                      exact
                        ? "status-panel__fill"
                        : "status-panel__fill status-panel__fill--approximate"
                    }
                    style={{
                      width: `${Math.max(0, Math.min(1, fraction)) * 100}%`,
                    }}
                  />
                </div>
              )}
            </div>
          );
        })}
      </dl>
    </aside>
  );
}
