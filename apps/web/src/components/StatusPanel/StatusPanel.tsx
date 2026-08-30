import React from "react";
import type { Disclosed } from "@/engine/sdk/Disclosed";
import type { ResourceDefinition } from "@/engine/sdk/ResourceDefinition";
import "@/styles/StatusPanel.scss";

/**
 * The selected token's resources, in a screen corner.
 *
 * # Why this is React and the token bars are not
 *
 * Constitution Principle I splits on spatial versus screen-space. A bar above
 * a token tracks its position and scales with the camera, so the engine draws
 * it. This panel is text in a fixed corner, and drawing it in WebGL would mean
 * reimplementing text layout, focus handling and screen-reader support to obey
 * a principle that explicitly permits panels in React. See ADR-053.
 *
 * What keeps this from becoming a second source of truth: it **reads** the
 * resolved status the engine already holds and computes nothing. No values are
 * derived here, no disclosure decision is made here, and there is nothing this
 * component could do to widen what a viewer sees — the coarsening happened on
 * the server, and a withheld figure is not in the browser at all.
 */

export type PanelCorner =
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right";

export interface PanelResource {
  definition: ResourceDefinition;
  disclosed: Disclosed;
}

export interface StatusPanelProps {
  /** Resources for the currently selected token, or null when nothing is selected. */
  resources: PanelResource[] | null;
  /** Name shown as the panel's heading. */
  title?: string;
  corner: PanelCorner;
  onCornerChange?: (corner: PanelCorner) => void;
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

const CORNERS: { value: PanelCorner; label: string }[] = [
  { value: "top-left", label: "Top left" },
  { value: "top-right", label: "Top right" },
  { value: "bottom-left", label: "Bottom left" },
  { value: "bottom-right", label: "Bottom right" },
];

export function StatusPanel({
  resources,
  title,
  corner,
  onCornerChange,
}: StatusPanelProps): React.ReactElement | null {
  // Nothing selected means no panel — not an empty panel holding the last
  // token's numbers, which would be actively misleading mid-fight.
  if (!resources || resources.length === 0) {
    return null;
  }

  const ordered = [...resources].sort(
    (a, b) => a.definition.order - b.definition.order,
  );

  return (
    <aside
      className={`status-panel status-panel--${corner}`}
      aria-label="Selected token status"
    >
      <header className="status-panel__head">
        <h2 className="status-panel__title">{title ?? "Selected"}</h2>
        {onCornerChange && (
          <label className="status-panel__corner">
            <span className="visually-hidden">Panel position</span>
            <select
              value={corner}
              aria-label="Panel position"
              onChange={(e) => onCornerChange(e.target.value as PanelCorner)}
            >
              {CORNERS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
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
