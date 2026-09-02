/**
 * The renderer for an interface pack's layout (spec 032, T041 and T042).
 *
 * Walks a `LayoutDeclaration` and produces React elements, resolving generic
 * constructs against what the *system* declares — in the system's own order —
 * and specific constructs by identifier.
 *
 * # Three rules this file exists to keep
 *
 * 1. **The pack does not choose the order.** `badgeGrid of "attributes"`
 *    renders every declared attribute, in declaration order. Sorting them
 *    here would be a claim about the ruleset.
 *
 * 2. **An empty set renders nothing** — not an empty frame, not a heading
 *    over blank space. Fate Core declares zero abilities; a "Skills" heading
 *    with nothing under it tells a player their character sheet is broken.
 *    So a section whose children all render nothing renders nothing itself,
 *    which is why emptiness is decided by `rendersAnything` *before* a node
 *    is rendered rather than by returning `null` from inside it.
 *
 * 3. **A derived value never gets an editable control.** A 5e Strength score
 *    is typed in and its modifier is not, and a text box over a computed
 *    number invites the two to disagree — with the stored one going stale.
 *    That is the whole reason `origin` is on the wire, and here it is visible
 *    in the DOM: a stored value renders an `<input>`, a derived one renders
 *    no input at all and carries `aria-readonly`.
 *
 * # Why editing is not wired to a mutation
 *
 * T042 is about what a player is *offered*, not about what happens when they
 * type. `onValueChange` is the seam a caller supplies; without one the
 * control is still editable, because whether a number may be edited is a
 * property of the value, not of whether this screen happens to have a
 * mutation hooked up yet.
 */

import { useId } from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { declarationsFrom, parseFraction, valuesIn } from "./declarations";
import {
  rendersAnything,
  resolutionFrom,
  slotLevels,
  type Resolution,
  type SlotLevel,
} from "./resolve";
import type {
  LayoutDeclaration,
  LayoutNode,
  SheetDeclarations,
  SheetValue,
} from "./types";

export interface SheetLayoutProps {
  /** The pack's arrangement, straight from `interface.json`. */
  layout: LayoutDeclaration;
  /**
   * What the system declares, per set, already in the system's own order.
   * Sets a caller omits are treated as sets the system declares empty.
   */
  declarations: Partial<SheetDeclarations>;
  /**
   * Called when a player edits a stored value. Never called for a derived
   * one — a derived value is never given a control to edit.
   */
  onValueChange?: (id: string, next: string) => void;
  className?: string;
}

// ---------------------------------------------------------------------------
// One value
// ---------------------------------------------------------------------------

/**
 * A value's number, editable exactly when the system stored it.
 *
 * The `data-origin` attribute is not decoration: it is how a test — and a
 * person reading the DOM — can see that rule 3 held, without inferring it
 * from the absence of something.
 */
function ValueControl({
  value,
  onValueChange,
  className,
}: {
  value: SheetValue;
  onValueChange?: (id: string, next: string) => void;
  className?: string;
}) {
  const inputId = useId();

  if (value.origin === "derived") {
    return (
      <output
        data-slot="declared-value"
        data-origin="derived"
        data-value-id={value.id}
        aria-readonly="true"
        aria-label={value.label}
        className={cn("text-sm font-semibold tabular-nums", className)}
      >
        {value.value}
      </output>
    );
  }

  return (
    <Input
      id={inputId}
      data-slot="declared-value"
      data-origin="stored"
      data-value-id={value.id}
      aria-label={value.label}
      defaultValue={value.value}
      onChange={
        onValueChange
          ? (event) => onValueChange(value.id, event.target.value)
          : undefined
      }
      className={cn("h-7 text-center text-sm tabular-nums", className)}
    />
  );
}

/** The words next to a value: its abbreviation where the system offers one. */
function ValueLabel({
  value,
  short = false,
}: {
  value: SheetValue;
  short?: boolean;
}) {
  const text = short ? (value.abbreviation ?? value.label) : value.label;
  return (
    <span
      data-slot="declared-value-label"
      title={value.label}
      className="text-xs font-medium tracking-wide text-muted-foreground uppercase"
    >
      {text}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Generic constructs
// ---------------------------------------------------------------------------

function BadgeGrid({
  values,
  columns,
  onValueChange,
}: {
  values: readonly SheetValue[];
  columns?: number | null;
  onValueChange?: (id: string, next: string) => void;
}) {
  const across = columns && columns > 0 ? columns : 3;
  return (
    <div
      data-slot="badge-grid"
      className="grid gap-2"
      style={{ gridTemplateColumns: `repeat(${across}, minmax(0, 1fr))` }}
    >
      {values.map((value) => (
        <div
          key={value.id}
          data-slot="badge"
          data-value-id={value.id}
          className="flex flex-col items-center gap-1 rounded-lg border border-border p-2"
        >
          <ValueLabel value={value} short />
          <ValueControl value={value} onValueChange={onValueChange} />
        </div>
      ))}
    </div>
  );
}

function BarStack({
  values,
  onValueChange,
}: {
  values: readonly SheetValue[];
  onValueChange?: (id: string, next: string) => void;
}) {
  return (
    <div data-slot="bar-stack" className="flex flex-col gap-2">
      {values.map((value) => {
        const fraction = parseFraction(value.value);
        const filled = fraction
          ? Math.max(0, Math.min(1, fraction.current / fraction.max))
          : null;
        return (
          <div
            key={value.id}
            data-slot="bar"
            data-value-id={value.id}
            className="flex flex-col gap-1"
          >
            <div className="flex items-center justify-between gap-2">
              <ValueLabel value={value} />
              <ValueControl value={value} onValueChange={onValueChange} />
            </div>
            {filled === null ? null : (
              <div
                role="meter"
                aria-label={value.label}
                aria-valuenow={fraction?.current}
                aria-valuemin={0}
                aria-valuemax={fraction?.max}
                className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
              >
                <div
                  className="h-full rounded-full bg-primary"
                  style={{ width: `${filled * 100}%` }}
                />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

function RowList({
  values,
  onValueChange,
}: {
  values: readonly SheetValue[];
  onValueChange?: (id: string, next: string) => void;
}) {
  return (
    <ul data-slot="row-list" className="flex flex-col gap-1">
      {values.map((value) => (
        <li
          key={value.id}
          data-slot="row"
          data-value-id={value.id}
          className="flex items-center justify-between gap-3 border-b border-border/50 py-1 last:border-b-0"
        >
          <ValueLabel value={value} />
          <ValueControl value={value} onValueChange={onValueChange} />
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// Specific constructs
// ---------------------------------------------------------------------------

/** How many boxes a tracker shows as filled. */
function filledBoxes(value: SheetValue): number {
  const parsed = Number.parseInt(value.value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function Tracker({
  value,
  boxes,
  rows,
}: {
  value: SheetValue;
  boxes: number;
  rows: number;
}) {
  const filled = filledBoxes(value);
  const editable = value.origin === "stored";
  return (
    <div
      data-slot="tracker"
      data-value-id={value.id}
      data-origin={value.origin}
      className="flex flex-col gap-1"
    >
      <ValueLabel value={value} />
      {Array.from({ length: Math.max(1, rows) }, (_, row) => (
        <div
          key={row}
          role="group"
          aria-label={value.label}
          className="flex gap-1"
        >
          {Array.from({ length: boxes }, (_, column) => {
            const index = row * boxes + column;
            const checked = index < filled;
            const shared = {
              "data-slot": "tracker-box",
              "aria-checked": checked,
              className: cn(
                "size-4 rounded-[4px] border border-input",
                checked && "bg-primary",
              ),
            } as const;
            // A derived tracker is a readout, not a control: it never
            // becomes a button, because a button invites a click that would
            // have to be refused.
            return editable ? (
              <button
                key={column}
                type="button"
                role="checkbox"
                aria-label={`${value.label} ${index + 1}`}
                {...shared}
              />
            ) : (
              <span
                key={column}
                role="img"
                aria-readonly="true"
                aria-label={`${value.label} ${index + 1}`}
                {...shared}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}

function SlotGrid({
  id,
  levels,
  at,
}: {
  id: string;
  levels: SlotLevel[];
  at: Resolution;
}) {
  return (
    <div
      data-slot="slot-grid"
      data-value-id={id}
      className="flex flex-col gap-1"
    >
      {levels.map(({ level, total, spent }) => (
        <div
          key={level}
          data-slot="slot-level"
          data-level={level}
          className="flex items-center gap-2"
        >
          <Badge variant="outline">{level}</Badge>
          {total ? (
            <ValueControl
              value={total}
              onValueChange={at.onValueChange}
              className="w-14"
            />
          ) : null}
          {spent ? (
            <ValueControl
              value={spent}
              onValueChange={at.onValueChange}
              className="w-14"
            />
          ) : null}
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

function Node({ node, at }: { node: LayoutNode; at: Resolution }) {
  switch (node.kind) {
    case "section":
      return (
        <section
          data-slot="layout-section"
          data-collapsed={node.collapsed ? "true" : undefined}
          className="flex flex-col gap-2"
        >
          {node.title ? (
            <h3 className="text-sm font-semibold tracking-wide">
              {node.title}
            </h3>
          ) : null}
          <Children nodes={node.children} at={at} />
        </section>
      );
    case "column":
      return (
        <div data-slot="layout-column" className="flex flex-1 flex-col gap-2">
          <Children nodes={node.children} at={at} />
        </div>
      );
    case "row":
      return (
        <div data-slot="layout-row" className="flex flex-row gap-4">
          <Children nodes={node.children} at={at} />
        </div>
      );
    case "badgeGrid":
      return (
        <BadgeGrid
          values={valuesIn(at.declarations, node.of)}
          columns={node.columns}
          onValueChange={at.onValueChange}
        />
      );
    case "barStack":
      return (
        <BarStack
          values={valuesIn(at.declarations, node.of)}
          onValueChange={at.onValueChange}
        />
      );
    case "rowList":
      return (
        <RowList
          values={valuesIn(at.declarations, node.of)}
          onValueChange={at.onValueChange}
        />
      );
    case "value": {
      const value = at.byId.get(node.id);
      if (!value) return null;
      return (
        <div
          data-slot="layout-value"
          className="flex items-center justify-between gap-2"
        >
          <ValueLabel value={value} />
          <ValueControl value={value} onValueChange={at.onValueChange} />
        </div>
      );
    }
    case "pair": {
      const left = at.byId.get(node.value);
      const right = at.byId.get(node.beside);
      if (!left && !right) return null;
      return (
        <div data-slot="layout-pair" className="flex items-center gap-2">
          {left ? (
            <div className="flex flex-col items-center gap-1">
              <ValueLabel value={left} short />
              <ValueControl value={left} onValueChange={at.onValueChange} />
            </div>
          ) : null}
          {right ? (
            <div className="flex flex-col items-center gap-1">
              <ValueLabel value={right} short />
              <ValueControl value={right} onValueChange={at.onValueChange} />
            </div>
          ) : null}
        </div>
      );
    }
    case "tracker": {
      const value = at.byId.get(node.id);
      if (!value) return null;
      return <Tracker value={value} boxes={node.boxes} rows={node.rows ?? 1} />;
    }
    case "slotGrid": {
      const levels = slotLevels(node.id, node.levels, at);
      if (levels.length === 0) return null;
      return <SlotGrid id={node.id} levels={levels} at={at} />;
    }
    default:
      // Already reported by `rendersAnything`, which runs first.
      return null;
  }
}

function Children({ nodes, at }: { nodes: LayoutNode[]; at: Resolution }) {
  return (
    <>
      {nodes
        .filter((child) => rendersAnything(child, at))
        .map((child, index) => (
          <Node key={keyFor(child, index)} node={child} at={at} />
        ))}
    </>
  );
}

/** A stable-enough key: the node's own address, or its position. */
function keyFor(node: LayoutNode, index: number): string {
  switch (node.kind) {
    case "badgeGrid":
    case "barStack":
    case "rowList":
      return `${node.kind}:${node.of}`;
    case "value":
    case "tracker":
    case "slotGrid":
      return `${node.kind}:${node.id}`;
    case "pair":
      return `pair:${node.value}:${node.beside}`;
    default:
      return `${node.kind}:${index}`;
  }
}

/**
 * An interface pack's sheet, rendered against one actor's declared values.
 *
 * Renders `null` when the whole layout resolves to nothing — a system that
 * declares no values at all gets no frame, for the same reason an empty
 * section gets no heading.
 */
export function SheetLayout({
  layout,
  declarations,
  onValueChange,
  className,
}: SheetLayoutProps) {
  const resolved = declarationsFrom(declarations);
  const at: Resolution = resolutionFrom(resolved, onValueChange);
  const visible = layout.filter((node) => rendersAnything(node, at));
  if (visible.length === 0) return null;
  return (
    <div
      data-slot="sheet-layout"
      className={cn("flex flex-col gap-4", className)}
    >
      {visible.map((node, index) => (
        <Node key={keyFor(node, index)} node={node} at={at} />
      ))}
    </div>
  );
}
