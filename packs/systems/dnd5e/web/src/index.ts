/**
 * D&D 5e, web side.
 *
 * The host mounts the sheet by finding `src/ActorSheet.tsx`'s path at build
 * time, not by importing it from here, so this module stays small: it is what
 * `apps/web/src/pages/world/actor/systemSheetReaders.ts` loads to find the
 * pack's character-sheet reader (spec 048).
 *
 * `@thunderforge/sheet-dnd5e` is `packs/systems/dnd5e/sheet` compiled for the
 * browser: the same reader the server re-reads an upload with. It is fetched
 * only when somebody imports a sheet.
 */
export const sheetReader = () => import("@thunderforge/sheet-dnd5e");
