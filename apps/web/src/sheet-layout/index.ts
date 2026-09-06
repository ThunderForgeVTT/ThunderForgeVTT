/**
 * The layout renderer for interface packs (spec 032).
 *
 * `SheetLayout` is the only entry point a page needs; the rest is exported
 * because a caller assembling declarations from a GraphQL response needs the
 * shapes to assemble them into — including `all`, the system's full published
 * set, without which `other` has nothing to be the complement of.
 */

export { SheetLayout, type SheetLayoutProps } from "./SheetLayout";
export {
  declarationsFrom,
  emptyDeclarations,
  declarationsDrift,
  indexById,
  resetDeclarationDriftWarnings,
  valuesIn,
} from "./declarations";
export {
  rendersAnything,
  resolutionFrom,
  shapeOf,
  stateReading,
  unitReading,
  unitsOf,
  type Resolution,
  type StateReading,
  type UnitReading,
  type ValueShape,
  type ValueUnit,
} from "./resolve";
export {
  DECLARATION_SETS,
  NAMED_DECLARATION_SETS,
  type DeclarationSet,
  type LayoutDeclaration,
  type LayoutNode,
  type NamedDeclarationSet,
  type ResolvedDeclarations,
  type SheetDeclarations,
  type SheetValue,
  type ValueFraction,
  type ValueOrigin,
  type ValueState,
  type ValueTrack,
} from "./types";
