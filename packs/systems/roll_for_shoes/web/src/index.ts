/**
 * Roll for Shoes, web side.
 *
 * The host mounts the sheet by finding `src/ActorSheet.tsx`'s path at build
 * time, not by importing it from here. It is re-exported so a direct consumer
 * can reach it the same way as anything else in this pack.
 */

export { ActorSheet, default as default } from "./ActorSheet.tsx";

export { SkillLineage } from "./components/SkillLineage.tsx";
export { RollResult } from "./components/RollResult.tsx";
export { AdvancementPrompt } from "./components/AdvancementPrompt.tsx";

export {
  SYSTEM_ID,
  STARTING_SKILL,
  grantSkill,
  isAdvancement,
  lineageOrder,
  newSkillId,
  remainingNonSixes,
  sixesShown,
  skillsOf,
  spendXp,
  verdict,
  xpAward,
  xpOf,
} from "./game.ts";
export type {
  GrantMade,
  GrantRefusal,
  LineageRow,
  Skill,
  SpendAllowed,
  SpendRefusal,
  Verdict,
} from "./game.ts";

/**
 * `@thunderforge/sheet-roll_for_shoes` is `packs/systems/roll_for_shoes/sheet`
 * compiled for the browser: the reader for the one-page ThunderForge sheet,
 * the same one the server re-reads an upload with (spec 048 US4). It is
 * fetched only when somebody imports a sheet, and
 * `apps/web/src/pages/world/actor/systemSheetReaders.ts` finds it here.
 */
export const sheetReader = () => import("@thunderforge/sheet-roll_for_shoes");
