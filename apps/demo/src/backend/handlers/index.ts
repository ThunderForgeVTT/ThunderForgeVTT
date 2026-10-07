/**
 * The handlers that live in their own files, one file per area, gathered for
 * `../handlers.ts` to add to the declared list (spec 074 FR-009).
 */
import type { Handler } from "./common";
import { diceMutations, diceQueries } from "./dice";
import { levelMutations, levelQueries } from "./levels";
import { lightingMutations, lightingQueries } from "./lighting";

export const areaQueries: Record<string, Handler> = {
  ...diceQueries,
  ...lightingQueries,
  ...levelQueries,
};

export const areaMutations: Record<string, Handler> = {
  ...diceMutations,
  ...lightingMutations,
  ...levelMutations,
};
