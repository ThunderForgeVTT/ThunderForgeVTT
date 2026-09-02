/**
 * Deciding what a layout node resolves to, before anything is rendered.
 *
 * Separate from `SheetLayout.tsx` for two reasons. The mechanical one is that
 * a module exporting both a component and a plain function cannot fast
 * refresh. The real one is rule 2 of the renderer: *a set the system declares
 * as empty renders nothing, and a section whose children all render nothing
 * renders nothing itself*. React cannot be asked after the fact whether a
 * subtree came out empty, so emptiness has to be a question that can be
 * answered about a node without rendering it. That question lives here.
 *
 * The other questions here are of the same shape — what *kind* of thing a
 * value is, and which values belong together — and they are answered by
 * reading structured fields, never by reading the rendered string. Parsing
 * the string to decide was a real bug: a system writing "4 of 7" instead of
 * "4 / 7" silently lost its bar (spec 032 T019a).
 */

import { indexById, valuesIn } from "./declarations";
import type { LayoutNode, ResolvedDeclarations, SheetValue } from "./types";

/** What a node needs in order to resolve anything it addresses. */
export interface Resolution {
  declarations: ResolvedDeclarations;
  byId: ReadonlyMap<string, SheetValue>;
  onValueChange?: (id: string, next: string) => void;
}

export function resolutionFrom(
  declarations: ResolvedDeclarations,
  onValueChange?: (id: string, next: string) => void,
): Resolution {
  return { declarations, byId: indexById(declarations), onValueChange };
}

// ---------------------------------------------------------------------------
// What kind of thing a value is
// ---------------------------------------------------------------------------

/**
 * Which representation a value asks for.
 *
 * Decided by which structured field the server sent, in the order the wire
 * type makes them mutually exclusive. `text` is not a failure case: it is
 * what a score, a name, a Fate ladder rung and a proficiency all are.
 */
export type ValueShape = "pool" | "track" | "state" | "text";

export function shapeOf(value: SheetValue): ValueShape {
  if (value.fraction) return "pool";
  if (value.track) return "track";
  if (value.state) return "state";
  return "text";
}

/** How a state ladder reads: its rungs, which one is current, and whether it is known. */
export interface StateReading {
  options: readonly string[];
  /** The stored state, or null for "none of them" — a real answer. */
  current: string | null;
  /**
   * True when a state is stored that the system no longer declares.
   *
   * The failure this exists to prevent: a saved character whose condition was
   * renamed reading as the *first* option — which on a damage track means
   * silently healed. An unknown state is shown as unknown.
   */
  unknown: boolean;
}

export function stateReading(value: SheetValue): StateReading | null {
  const state = value.state;
  if (!state) return null;
  const options = state.options ?? [];
  const current = state.current === "" ? null : (state.current ?? null);
  return {
    options,
    current,
    unknown: current !== null && !options.includes(current),
  };
}

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

/**
 * One thing on the sheet: a lone value, or the several that share a group.
 *
 * A Fate consequence is a severity and the aspect written into it; a Cypher
 * stat is a current value, a pool and an edge. Those are one thing a player
 * reads, and rendering them as unrelated rows loses exactly the fact the
 * `group` field was added to carry (FR-033).
 */
export interface ValueUnit {
  /** The group's identifier, or null for a value that is in no group. */
  group: string | null;
  /** In declaration order within the group. The system's order, never ours. */
  values: readonly SheetValue[];
  key: string;
}

/**
 * Split a set into the units it renders as.
 *
 * A group takes the position of its *first* member, so grouping never
 * reorders a set: it only pulls later members up to join the first. Members
 * need not be adjacent in the system's declaration list — nothing requires a
 * system to declare them together — but their order among themselves is the
 * system's own.
 */
export function unitsOf(values: readonly SheetValue[]): ValueUnit[] {
  const units: { group: string | null; values: SheetValue[]; key: string }[] =
    [];
  const byGroup = new Map<string, { values: SheetValue[] }>();
  for (const value of values) {
    const group = value.group ?? null;
    if (group === null) {
      units.push({ group: null, values: [value], key: value.id });
      continue;
    }
    const existing = byGroup.get(group);
    if (existing) {
      existing.values.push(value);
      continue;
    }
    const unit = { group, values: [value], key: `group:${group}` };
    byGroup.set(group, unit);
    units.push(unit);
  }
  return units;
}

// ---------------------------------------------------------------------------
// Emptiness
// ---------------------------------------------------------------------------

/**
 * Kinds already complained about, so an unrecognised node costs one line in
 * the console rather than one per node per render.
 */
const warned = new Set<string>();

function warnUnknown(kind: string): void {
  if (warned.has(kind)) return;
  warned.add(kind);
  console.warn(`[layout] ignoring unknown node kind: ${kind}`);
}

/** Test seam: forget what has already been warned about. */
export function resetUnknownKindWarnings(): void {
  warned.clear();
}

/**
 * Whether this node would put anything on the screen.
 *
 * An unknown kind answers `false` — a sheet authored against a newer pack
 * format loses the nodes this build does not understand and keeps the rest,
 * rather than failing to render at all.
 */
export function rendersAnything(node: LayoutNode, at: Resolution): boolean {
  switch (node.kind) {
    case "section":
    case "column":
    case "row":
      return node.children.some((child) => rendersAnything(child, at));
    case "badgeGrid":
    case "barStack":
    case "rowList":
      return valuesIn(at.declarations, node.of).length > 0;
    case "value":
    case "block":
      return at.byId.has(node.id);
    case "pair":
      return at.byId.has(node.value) || at.byId.has(node.beside);
    default:
      warnUnknown((node as { kind?: string }).kind ?? "(missing kind)");
      return false;
  }
}
