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
 */

/** A set of declarations, addressed by kind rather than by name. */
export type DeclarationSet =
  | "attributes"
  | "resources"
  | "skills"
  | "movement"
  | "derived";

/** Every set the format offers, in the order the Rust enum lists them. */
export const DECLARATION_SETS: readonly DeclarationSet[] = [
  "attributes",
  "resources",
  "skills",
  "movement",
  "derived",
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
  | { kind: "tracker"; id: string; boxes: number; rows?: number }
  | { kind: "slotGrid"; id: string; levels: number };

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
 * string server-side, on purpose, so that no surface has to branch on a
 * value's type — and branching on a value's type is the first step towards
 * knowing what it means.
 */
export interface SheetValue {
  id: string;
  label: string;
  abbreviation?: string | null;
  value: string;
  origin: ValueOrigin;
}

/**
 * What the system declares, per set, in the system's own declaration order.
 *
 * The pack never reorders these. A system lists its abilities the way its
 * book does, and a pack sorting them would be making a claim about the
 * ruleset.
 */
export type SheetDeclarations = Record<DeclarationSet, readonly SheetValue[]>;
