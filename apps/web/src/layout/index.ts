/**
 * The layout renderer for interface packs (spec 032).
 *
 * `SheetLayout` is the only entry point a page needs; the rest is exported
 * because a caller assembling declarations from a GraphQL response needs the
 * shapes to assemble them into.
 */

export { SheetLayout, type SheetLayoutProps } from "./SheetLayout";
export {
  declarationsFrom,
  emptyDeclarations,
  indexById,
  valuesIn,
} from "./declarations";
export {
  rendersAnything,
  resolutionFrom,
  slotLevels,
  type Resolution,
  type SlotLevel,
} from "./resolve";
export {
  DECLARATION_SETS,
  type DeclarationSet,
  type LayoutDeclaration,
  type LayoutNode,
  type SheetDeclarations,
  type SheetValue,
  type ValueOrigin,
} from "./types";
