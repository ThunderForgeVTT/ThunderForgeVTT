/**
 * The layout half of an interface pack, as the web client sees it.
 *
 * Hand-written rather than generated, because `pack_system_spec::layout` is
 * not exported through ts-rs — `apps/web/src/engine/sdk/` carries only what
 * the engine SDK publishes. These types mirror
 * `crates/pack_system_spec/src/layout.rs` exactly, including its serde
 * representation (`#[serde(tag = "kind", rename_all = "camelCase")]`), so a
 * node arrives on the wire as `{"kind": "badgeGrid", "of": "attributes",
 * "columns": 3}`. If that enum gains a variant, this union is where it lands.
 *
 * Nothing here carries an expression or a condition, and that is deliberate
 * upstream: a pack says *where* a value appears, never *what* it means.
 *
 * # Why there is no node kind per kind of value
 *
 * There was: a `tracker` carrying a box count and a `slotGrid` carrying a
 * level count. Both are gone, upstream and here. A `tracker` with `boxes` was
 * a layout stating what a value *is*, and it was wrong for two of the three
 * systems that have a track. The value that arrives says whether it is a
 * number, a pool, a run of marks or a rung on a ladder; `value` and `block`
 * name an identifier and let the value answer that. The layout says *where*.
 */

/**
 * A set the system fills in directly, by kind.
 *
 * Split out from {@link DeclarationSet} because `other` is not one of these:
 * it is the complement of all of them, computed rather than supplied.
 */
export type NamedDeclarationSet =
  | "attributes"
  | "resources"
  | "skills"
  | "movement"
  | "derived";

/** A set of declarations, addressed by kind rather than by name. */
export type DeclarationSet = NamedDeclarationSet | "other";

/** The sets a caller supplies, in the order the Rust enum lists them. */
export const NAMED_DECLARATION_SETS: readonly NamedDeclarationSet[] = [
  "attributes",
  "resources",
  "skills",
  "movement",
  "derived",
];

/** Every set the format offers, in the order the Rust enum lists them. */
export const DECLARATION_SETS: readonly DeclarationSet[] = [
  ...NAMED_DECLARATION_SETS,
  "other",
];

/** One node of a pack's layout. */
export type LayoutNode =
  // ---- containers -----------------------------------------------------
  | {
      kind: "section";
      title?: string | null;
      collapsed?: boolean;
      children: LayoutNode[];
    }
  | { kind: "column"; children: LayoutNode[] }
  | { kind: "row"; children: LayoutNode[] }
  // ---- generic: addresses a set, names nothing -------------------------
  | { kind: "badgeGrid"; of: DeclarationSet; columns?: number | null }
  | { kind: "barStack"; of: DeclarationSet }
  | { kind: "rowList"; of: DeclarationSet }
  // ---- specific: names identifiers the target system declares ----------
  | { kind: "value"; id: string }
  | { kind: "pair"; value: string; beside: string }
  /**
   * The same value a `value` would render, given room to breathe. The
   * difference is space, not meaning: a `block` naming a number is a number
   * in a wide box, not a claim that the number is prose.
   */
  | { kind: "block"; id: string };

/** A pack's whole arrangement. */
export type LayoutDeclaration = LayoutNode[];

/** Whether a value was read from stored data or computed from it. */
export type ValueOrigin = "stored" | "derived";

/**
 * One value a system publishes, in the shape the server actually sends.
 *
 * This is `GraphQLDeclaredValue` from
 * `src/server/src/graphql/queries/token_attributes.rs`, not the richer
 * `DeclaredValue` of the engine SDK: `value` has already been rendered to a
 * string server-side, on purpose, so that no surface has to invent a
 * formatting of its own.
 *
 * The structured fields below sit *beside* that string rather than replacing
 * it. They are what a renderer switches on. The string is what it shows when
 * none of them is present.
 */
/** A pool's two numbers, as numbers. */
export interface ValueFraction {
  current: number;
  /** Absent for a counter, which has no maximum to be a proportion of. */
  max?: number | null;
}

/**
 * A bounded run of marks, and how many are ticked.
 *
 * Not a pool, though the numbers look alike. A pool is a quantity with a
 * maximum and the numbers are the point; a track is a set of marks and the
 * count is the point. Drawing one as the other gives a bar where a player
 * expects boxes to tick.
 */
export interface ValueTrack {
  filled: number;
  of: number;
}

/**
 * An ordered ladder of named states, of which one is current.
 *
 * The options travel with the current rung because a sheet shows the whole
 * ladder with a position on it. It is also what lets a stored state the
 * system no longer declares be shown as unknown: the renderer can see that
 * `current` is not among `options`.
 *
 * `current` null means none of them, which is a real answer — an uninjured
 * character is at no position on a damage track.
 */
export interface ValueState {
  current: string | null;
  /** In the system's own order, worst last. */
  options: string[];
}

export interface SheetValue {
  id: string;
  label: string;
  abbreviation?: string | null;
  /** Already rendered, for reading. */
  value: string;
  /**
   * Present only for a pool, and the only thing a bar is drawn from.
   *
   * The server sends both halves as numbers precisely so nothing here has to
   * parse `value` back apart — doing that was branching on what a value means,
   * and a system writing "4 of 7" instead of "4 / 7" silently lost its bar
   * (spec 032 T019a). The same rule governs `track` and `state`: read the
   * structured field, never the string.
   */
  fraction?: ValueFraction | null;
  /** Present only for a track. Marks to tick, not a bar to fill. */
  track?: ValueTrack | null;
  /** Present only for a state ladder. */
  state?: ValueState | null;
  /**
   * The group this belongs to, when it is part of one (FR-033).
   *
   * A Fate consequence is a severity *and* the aspect written into it; a
   * Cypher stat is a current value, a pool and an edge. Values sharing a
   * group are one thing on the sheet, and a renderer that drew them as
   * unrelated rows would have lost that.
   */
  group?: string | null;
  origin: ValueOrigin;
}

/**
 * What the system declares, per set, in the system's own declaration order.
 *
 * The pack never reorders these. A system lists its abilities the way its
 * book does, and a pack sorting them would be making a claim about the
 * ruleset.
 *
 * # Why `all` is here as well as the named sets
 *
 * Because `other` (FR-034) is *everything the named sets did not claim*, and
 * a complement cannot be computed from the things it excludes alone — the
 * renderer has to be told what the whole was. Without `all`, a system
 * declaring something this build has never heard of would have it silently
 * dropped, and a value missing from a sheet is indistinguishable from the
 * character not having it (FR-035). So the caller passes the system's full
 * published set, and `other` falls out of it.
 *
 * A caller that omits `all` gets an empty `other` rather than an error: that
 * is the old behaviour, and it is honest — nothing was published, so nothing
 * went unclaimed.
 */
export interface SheetDeclarations extends Record<
  NamedDeclarationSet,
  readonly SheetValue[]
> {
  /** Everything the system publishes, in its own declaration order. */
  all: readonly SheetValue[];
}

/**
 * Declarations with `other` worked out, which is what the renderer resolves
 * against. A caller never builds one of these; `declarationsFrom` does.
 */
export type ResolvedDeclarations = Record<
  DeclarationSet,
  readonly SheetValue[]
>;
