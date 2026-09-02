/**
 * The renderer for an interface pack's layout (spec 032, T041 and T042).
 *
 * Walks a `LayoutDeclaration` and produces React elements, resolving generic
 * constructs against what the *system* declares — in the system's own order —
 * and specific constructs by identifier.
 *
 * # Four rules this file exists to keep
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
 *    no input at all and carries `aria-readonly`. The rule holds for every
 *    representation: a derived track's marks are not buttons, and a derived
 *    ladder's rungs are not selectable.
 *
 * 4. **The layout says where; the value says what.** There is no node kind
 *    per kind of value — `tracker` and `slotGrid` are gone. `value` and
 *    `block` name an identifier, and what arrives decides how it draws: a
 *    `fraction` is a bar, a `track` is a run of marks, a `state` is a ladder
 *    with a rung marked, and anything else is its text. The difference
 *    between `value` and `block` is space, not meaning.
 *
 * # Never parse the rendered string
 *
 * Every one of those decisions reads a structured field. Deciding from the
 * text was a real bug (T019a): a system writing "4 of 7" instead of "4 / 7"
 * silently lost its bar, with nothing failing anywhere. `value` is for
 * showing, never for branching on.
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
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { declarationsFrom, valuesIn } from "./declarations";
import {
  rendersAnything,
  resolutionFrom,
  shapeOf,
  stateReading,
  unitsOf,
  unitReading,
  type Resolution,
  type ValueUnit,
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
   *
   * `all` is the system's full published set, and it is what `other` is
   * computed from: without it, a value no named set claims would fall off
   * the bottom of the sheet (FR-034, SC-012).
   */
  declarations: Partial<SheetDeclarations>;
  /**
   * Called when a player edits a stored value. Never called for a derived
   * one — a derived value is never given a control to edit.
   */
  onValueChange?: (id: string, next: string) => void;
  className?: string;
}

/** What every representation below needs. */
interface ValueProps {
  value: SheetValue;
  onValueChange?: (id: string, next: string) => void;
  className?: string;
}

// ---------------------------------------------------------------------------
// Text: the representation everything that is not a pool, track or ladder gets
// ---------------------------------------------------------------------------

/**
 * A value's text, editable exactly when the system stored it.
 *
 * The `data-origin` attribute is not decoration: it is how a test — and a
 * person reading the DOM — can see that rule 3 held, without inferring it
 * from the absence of something.
 *
 * `multiline` is the only thing `block` changes about a text value. It is a
 * claim about space, not about the value: a block naming a number gets a
 * number in a wide box.
 */
function ValueText({
  value,
  onValueChange,
  className,
  multiline = false,
}: ValueProps & { multiline?: boolean }) {
  const inputId = useId();

  if (value.origin === "derived") {
    return (
      <output
        data-slot="declared-value"
        data-origin="derived"
        data-value-id={value.id}
        aria-readonly="true"
        aria-label={value.label}
        className={cn(
          "text-sm font-semibold tabular-nums",
          multiline && "w-full text-left font-normal whitespace-pre-line",
          className,
        )}
      >
        {value.value}
      </output>
    );
  }

  const shared = {
    id: inputId,
    "data-slot": "declared-value",
    "data-origin": "stored" as const,
    "data-value-id": value.id,
    "aria-label": value.label,
    defaultValue: value.value,
    onChange: onValueChange
      ? (event: { target: { value: string } }) =>
          onValueChange(value.id, event.target.value)
      : undefined,
  };

  return multiline ? (
    <Textarea {...shared} className={cn("w-full text-sm", className)} />
  ) : (
    <Input
      {...shared}
      className={cn("h-7 text-center text-sm tabular-nums", className)}
    />
  );
}

// ---------------------------------------------------------------------------
// Pool: a proportion, drawn as a bar
// ---------------------------------------------------------------------------

/**
 * The bar for a pool, or nothing.
 *
 * Read, never parsed. And nothing for a counter: a pool with no maximum is
 * not a pool that is empty — Blades in the Dark's coin counts up with nothing
 * to be a proportion of, and a bar would have to invent the thing it fills.
 */
function ValueMeter({ value }: { value: SheetValue }) {
  const fraction = value.fraction;
  const max = fraction?.max ?? null;
  if (!fraction || max === null || max <= 0) return null;
  const filled = Math.max(0, Math.min(1, fraction.current / max));
  return (
    <div
      data-slot="value-meter"
      role="meter"
      aria-label={value.label}
      aria-valuenow={fraction.current}
      aria-valuemin={0}
      aria-valuemax={max}
      className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
    >
      <div
        className="h-full rounded-full bg-primary"
        style={{ width: `${filled * 100}%` }}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Track: a bounded run of marks
// ---------------------------------------------------------------------------

/**
 * `filled` of `of` marks, ticked.
 *
 * Not a bar, though the two numbers look alike. A pool is a quantity and the
 * numbers are the point; a track is a set of marks and the count is the
 * point — Fate's stress is eight boxes a player ticks, and drawing it as a
 * bar gives them nothing to tick. There is no notion of rows here for the
 * same reason there is none upstream: two tracks is what two tracks are.
 */
function ValueMarks({ value, onValueChange, className }: ValueProps) {
  const track = value.track;
  if (!track) return null;
  const total = Math.max(0, Math.trunc(track.of));
  const filled = Math.max(0, Math.min(total, Math.trunc(track.filled)));
  const editable = value.origin === "stored";

  return (
    <div
      data-slot="track"
      data-value-id={value.id}
      data-origin={value.origin}
      data-track-filled={filled}
      data-track-of={total}
      role="group"
      aria-label={value.label}
      className={cn("flex flex-wrap gap-1", className)}
    >
      {Array.from({ length: total }, (_, index) => {
        const checked = index < filled;
        const shared = {
          "data-slot": "track-mark",
          "aria-checked": checked,
          "aria-label": `${value.label} ${index + 1}`,
          className: cn(
            "size-4 rounded-[4px] border border-input",
            checked && "bg-primary",
          ),
        } as const;
        // A derived track is a readout, not a control: it never becomes a
        // button, because a button invites a click that would have to be
        // refused. Ticking mark n means the track now reads n, which is the
        // only edit a run of marks can express.
        return editable ? (
          <button
            key={index}
            type="button"
            role="checkbox"
            onClick={
              onValueChange
                ? () =>
                    onValueChange(
                      value.id,
                      String(
                        checked && index + 1 === filled ? index : index + 1,
                      ),
                    )
                : undefined
            }
            {...shared}
          />
        ) : (
          <span key={index} role="img" aria-readonly="true" {...shared} />
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// State: an ordered ladder with one rung current
// ---------------------------------------------------------------------------

/**
 * Every rung the system declares, with the current one marked.
 *
 * The whole ladder travels, not just the position, because a sheet shows what
 * comes next — a Cypher damage track a player can read ahead on.
 *
 * A stored state that is *not* among the options renders as unknown and marks
 * no rung. The failure that prevents is specific and bad: a saved character
 * whose condition was renamed silently reading as the first option, which on
 * a damage track means healed. `data-state-unknown` puts that in the DOM so
 * it is assertable rather than a matter of trust.
 */
function ValueLadder({ value, onValueChange, className }: ValueProps) {
  const reading = stateReading(value);
  if (!reading) return null;
  const { options, current, unknown } = reading;
  const editable = value.origin === "stored";

  return (
    <div
      data-slot="state-ladder"
      data-value-id={value.id}
      data-origin={value.origin}
      data-state-current={current ?? undefined}
      data-state-unknown={unknown ? "true" : undefined}
      role="group"
      aria-label={value.label}
      className={cn("flex flex-wrap items-center gap-1", className)}
    >
      {unknown ? (
        <span
          data-slot="state-unknown"
          title={`${value.label}: ${current}`}
          className="rounded-md border border-dashed border-destructive px-2 py-0.5 text-xs text-destructive"
        >
          {`Unknown state: ${current}`}
        </span>
      ) : null}
      {options.map((option) => {
        const isCurrent = !unknown && option === current;
        const shared = {
          "data-slot": "state-option",
          "data-state-option": option,
          "data-current": isCurrent ? "true" : undefined,
          "aria-checked": isCurrent,
          className: cn(
            "rounded-md border border-input px-2 py-0.5 text-xs",
            isCurrent && "border-primary bg-primary text-primary-foreground",
          ),
        } as const;
        return editable ? (
          <button
            key={option}
            type="button"
            role="radio"
            onClick={
              onValueChange ? () => onValueChange(value.id, option) : undefined
            }
            {...shared}
          >
            {option}
          </button>
        ) : (
          <span key={option} role="img" aria-readonly="true" {...shared}>
            {option}
          </span>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// One value, whatever kind it is
// ---------------------------------------------------------------------------

/**
 * The control area for a value: whichever of the four representations its
 * structured fields ask for.
 *
 * A pool keeps its number here and gets its bar from `ValueMeter` alongside,
 * because a bar without the numbers is a proportion a player cannot type into.
 */
function ValueBody({
  value,
  onValueChange,
  className,
  multiline = false,
}: ValueProps & { multiline?: boolean }) {
  switch (shapeOf(value)) {
    case "track":
      return (
        <ValueMarks
          value={value}
          onValueChange={onValueChange}
          className={className}
        />
      );
    case "state":
      return (
        <ValueLadder
          value={value}
          onValueChange={onValueChange}
          className={className}
        />
      );
    case "pool":
    case "text":
      return (
        <ValueText
          value={value}
          onValueChange={onValueChange}
          className={className}
          multiline={multiline}
        />
      );
  }
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

/** Label beside body, with a bar beneath when the value is a pool. */
function ValueLine({
  value,
  onValueChange,
  short = false,
  multiline = false,
}: ValueProps & { short?: boolean; multiline?: boolean }) {
  return (
    <div
      data-slot="value-line"
      data-value-id={value.id}
      className={cn(
        "flex gap-1",
        multiline ? "w-full flex-col" : "flex-col justify-between",
      )}
    >
      <div
        className={cn(
          "flex gap-2",
          multiline ? "flex-col" : "flex-row items-center justify-between",
        )}
      >
        <ValueLabel value={value} short={short} />
        <ValueBody
          value={value}
          onValueChange={onValueChange}
          multiline={multiline}
        />
      </div>
      <ValueMeter value={value} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

/**
 * The values of one unit, rendered as one thing.
 *
 * A group is a single frame with its members inside, in the system's own
 * order within the group. A Fate consequence's severity and the aspect
 * written into it are one line on a paper sheet, and two unrelated rows here
 * would be the renderer contradicting the system (FR-033).
 */
function Unit({
  unit,
  at,
  short = false,
}: {
  unit: ValueUnit;
  at: Resolution;
  short?: boolean;
}) {
  if (unit.group === null) {
    const [value] = unit.values;
    return (
      <ValueLine value={value} onValueChange={at.onValueChange} short={short} />
    );
  }
  // T019g: the group's name and its headline member are the system's to
  // state. `unitReading` falls back to the first member when it did not,
  // which is what this component used to do unconditionally.
  const reading = unitReading(unit);

  return (
    <div
      data-slot="value-group"
      data-group={unit.group}
      data-group-headline={reading.headline?.id}
      role="group"
      aria-label={reading.label}
      className="flex flex-col gap-1 rounded-lg border border-border p-2"
    >
      {unit.values.map((value) => (
        <ValueLine
          key={value.id}
          value={value}
          onValueChange={at.onValueChange}
          short={short}
        />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Generic constructs
// ---------------------------------------------------------------------------

function BadgeGrid({
  values,
  columns,
  at,
}: {
  values: readonly SheetValue[];
  columns?: number | null;
  at: Resolution;
}) {
  const across = columns && columns > 0 ? columns : 3;
  return (
    <div
      data-slot="badge-grid"
      className="grid gap-2"
      style={{ gridTemplateColumns: `repeat(${across}, minmax(0, 1fr))` }}
    >
      {unitsOf(values).map((unit) =>
        unit.group === null ? (
          <div
            key={unit.key}
            data-slot="badge"
            data-value-id={unit.values[0].id}
            className="flex flex-col items-center gap-1 rounded-lg border border-border p-2"
          >
            <ValueLabel value={unit.values[0]} short />
            <ValueBody
              value={unit.values[0]}
              onValueChange={at.onValueChange}
            />
            <ValueMeter value={unit.values[0]} />
          </div>
        ) : (
          <Unit key={unit.key} unit={unit} at={at} short />
        ),
      )}
    </div>
  );
}

function BarStack({
  values,
  at,
}: {
  values: readonly SheetValue[];
  at: Resolution;
}) {
  return (
    <div data-slot="bar-stack" className="flex flex-col gap-2">
      {unitsOf(values).map((unit) => (
        <div key={unit.key} data-slot="bar">
          <Unit unit={unit} at={at} />
        </div>
      ))}
    </div>
  );
}

function RowList({
  values,
  at,
}: {
  values: readonly SheetValue[];
  at: Resolution;
}) {
  return (
    <ul data-slot="row-list" className="flex flex-col gap-1">
      {unitsOf(values).map((unit) => (
        <li
          key={unit.key}
          data-slot="row"
          className="border-b border-border/50 py-1 last:border-b-0"
        >
          <Unit unit={unit} at={at} />
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

function Node({ node, at }: { node: LayoutNode; at: Resolution }) {
  switch (node.kind) {
    case "section":
      // T019b. `collapsed` reached the DOM as `data-collapsed` and collapsed
      // nothing, so a pack author could declare it and watch it do nothing at
      // all. The format already defines what it means — "starts collapsed, and
      // a reader who opens it stays opened, and nothing here can force it shut
      // again" — and `<details>` is that sentence, natively, with the keyboard
      // and screen-reader behaviour already right and no state to persist.
      //
      // Only a *titled* section can collapse: the title is the summary, and a
      // collapsed section with nothing to click would be a section a reader
      // cannot open. `collapsed` on a titleless one is ignored rather than
      // honoured into an unreachable sheet.
      if (node.collapsed && node.title) {
        return (
          <details
            data-slot="layout-section"
            data-collapsed="true"
            className="flex flex-col gap-2"
          >
            <summary className="cursor-pointer text-sm font-semibold tracking-wide">
              {node.title}
            </summary>
            <Children nodes={node.children} at={at} />
          </details>
        );
      }
      return (
        <section data-slot="layout-section" className="flex flex-col gap-2">
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
          at={at}
        />
      );
    case "barStack":
      return <BarStack values={valuesIn(at.declarations, node.of)} at={at} />;
    case "rowList":
      return <RowList values={valuesIn(at.declarations, node.of)} at={at} />;
    case "value": {
      const value = at.byId.get(node.id);
      if (!value) return null;
      return (
        <div data-slot="layout-value" className="flex flex-col gap-1">
          <ValueLine value={value} onValueChange={at.onValueChange} />
        </div>
      );
    }
    case "block": {
      const value = at.byId.get(node.id);
      if (!value) return null;
      // The same value a `value` would render, given the width and the room
      // to wrap. Space, not meaning.
      return (
        <div data-slot="layout-block" className="flex w-full flex-col gap-1">
          <ValueLine value={value} onValueChange={at.onValueChange} multiline />
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
              <ValueBody value={left} onValueChange={at.onValueChange} />
            </div>
          ) : null}
          {right ? (
            <div className="flex flex-col items-center gap-1">
              <ValueLabel value={right} short />
              <ValueBody value={right} onValueChange={at.onValueChange} />
            </div>
          ) : null}
        </div>
      );
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
    case "block":
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
