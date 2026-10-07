/**
 * The handlers that live in their own files, one file per area, gathered for
 * `../handlers.ts` to add to the declared list (spec 074 FR-009).
 */
import type { Handler } from "./common";
import { assetMutations, assetQueries } from "./assets";
import { diceMutations, diceQueries } from "./dice";
import { levelMutations, levelQueries } from "./levels";
import { lightingMutations, lightingQueries } from "./lighting";
import { sceneMutations, sceneQueries } from "./scenes";
import { shapeMutations, shapeQueries } from "./shapes";
import { tokenSheetQueries } from "./tokenSheets";

export const areaQueries: Record<string, Handler> = {
  ...diceQueries,
  ...lightingQueries,
  ...levelQueries,
  ...tokenSheetQueries,
  ...sceneQueries,
  ...assetQueries,
  ...shapeQueries,
};

export const areaMutations: Record<string, Handler> = {
  ...diceMutations,
  ...lightingMutations,
  ...levelMutations,
  ...sceneMutations,
  ...assetMutations,
  ...shapeMutations,
};
