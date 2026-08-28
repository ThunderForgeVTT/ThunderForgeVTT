/**
 * Facets: the typed surfaces a session is driven through.
 *
 * Assemble once per world with `createWorldFacets`, then hand individual
 * facets to whatever needs them. See `./types.ts` for why these exist and
 * how they line up with `thunderforge-crucible`.
 */

import type { WorldStore } from "../store";
import { createLocalAdjudicator } from "./adjudicator";
import { createPlaybackFacet, type PlaybackFacet } from "./playback";
import { createSelectionFacet, type SelectionFacet } from "./selection";
import {
  createTokenControlFacet,
  type TokenControlFacet,
} from "./tokenControl";
import type { Adjudicator, FacetContext, FacetPrincipal } from "./types";

export * from "./types";
export * from "./adjudicator";
export * from "./playback";
export * from "./selection";
export * from "./tokenControl";

export interface WorldFacets {
  tokens: TokenControlFacet;
  selection: SelectionFacet;
  playback: PlaybackFacet;
  /** Teardown for anything holding a subscription. */
  stop: () => void;
}

export function createWorldFacets(
  store: WorldStore,
  options: {
    worldId: string;
    sceneId: string | null;
    principal: FacetPrincipal;
    /** Defaults to the local pass-through — today's behaviour. Pass a
     *  remote one to put Crucible in charge. */
    adjudicator?: Adjudicator;
  },
): WorldFacets {
  const context: FacetContext = {
    worldId: options.worldId,
    sceneId: options.sceneId,
    principal: options.principal,
    adjudicator: options.adjudicator ?? createLocalAdjudicator(),
  };

  const tokens = createTokenControlFacet(store, context);
  const selection = createSelectionFacet(store, context, tokens);
  const playback = createPlaybackFacet(options.worldId);

  return {
    tokens,
    selection,
    playback,
    stop: () => playback.stop(),
  };
}
