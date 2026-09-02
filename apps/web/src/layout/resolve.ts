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
 */

import { indexById, valuesIn } from "./declarations";
import type { LayoutNode, SheetDeclarations, SheetValue } from "./types";

/** What a node needs in order to resolve anything it addresses. */
export interface Resolution {
  declarations: SheetDeclarations;
  byId: ReadonlyMap<string, SheetValue>;
  onValueChange?: (id: string, next: string) => void;
}

export function resolutionFrom(
  declarations: SheetDeclarations,
  onValueChange?: (id: string, next: string) => void,
): Resolution {
  return { declarations, byId: indexById(declarations), onValueChange };
}

/** One level of a slot grid: its total, its spent count, or one of the two. */
export interface SlotLevel {
  level: number;
  total: SheetValue | null;
  spent: SheetValue | null;
}

/**
 * The counters one slot grid resolves to, level by level.
 *
 * The format gives a slot grid a single identifier and a level count, while
 * the wire carries one value per identifier — so the per-level identifiers
 * have to be recovered by convention (`spellSlots3`, `spellSlots3Spent`, and
 * the dotted equivalents). That is a gap in the layout format rather than a
 * preference expressed here: nothing in `LayoutNode::SlotGrid` says how a
 * level addresses its two numbers.
 *
 * A level neither of whose counters the system declares is omitted, so a
 * caster who has reached level 3 shows three rows rather than nine, and a
 * system with no slots at all shows no grid.
 */
export function slotLevels(
  id: string,
  levels: number,
  at: Resolution,
): SlotLevel[] {
  const found: SlotLevel[] = [];
  for (let level = 1; level <= levels; level += 1) {
    const total =
      at.byId.get(`${id}${level}`) ?? at.byId.get(`${id}.${level}`) ?? null;
    const spent =
      at.byId.get(`${id}${level}Spent`) ??
      at.byId.get(`${id}.${level}.spent`) ??
      null;
    if (total || spent) found.push({ level, total, spent });
  }
  return found;
}

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
      return at.byId.has(node.id);
    case "pair":
      return at.byId.has(node.value) || at.byId.has(node.beside);
    case "tracker":
      return at.byId.has(node.id) && node.boxes > 0;
    case "slotGrid":
      return slotLevels(node.id, node.levels, at).length > 0;
    default:
      warnUnknown((node as { kind?: string }).kind ?? "(missing kind)");
      return false;
  }
}
