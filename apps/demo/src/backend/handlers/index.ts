/**
 * The handlers that live in their own files, one file per area, gathered for
 * `../handlers.ts` to add to the declared list (spec 074 FR-009).
 */
import { diceMutations, diceQueries } from "./dice";

type Handler = (args: Record<string, unknown>) => unknown;

export const areaQueries: Record<string, Handler> = {
  ...diceQueries,
};

export const areaMutations: Record<string, Handler> = {
  ...diceMutations,
};
