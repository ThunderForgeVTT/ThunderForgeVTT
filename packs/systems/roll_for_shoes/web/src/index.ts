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
