/**
 * The selection facet, including stacked tokens.
 *
 * Tokens stack: two characters in a doorway, a familiar on its owner, a
 * swarm in one square — and every token in a freshly created scene spawns
 * on the same spot. The engine used to take the first hit and stop, so
 * everything underneath was unreachable without dragging the pile apart one
 * token at a time.
 *
 * Two gestures, both driven by the engine:
 *
 * - **Click** selects the whole stack and picks it up. Dragging moves every
 *   member, each keeping its own offset, which is what "move these out of
 *   the doorway" means.
 * - **Double-click** asks *which one*. The click that preceded it already
 *   selected the stack, so this needs no new engine round trip: `disambiguate`
 *   just hands back what is selected for a picker to render, and the picker
 *   calls `selectOne`. Nothing is mutated by asking, so dismissing leaves the
 *   board exactly as it was.
 *
 * Double-click is detected in the DOM, not the engine. Two clicks that fast
 * routinely land in the same frame, where Bevy's `just_pressed` sees one
 * press and the second is simply lost — measured, not theorised. The
 * browser's own `dblclick` has no such problem.
 *
 * **The hit test lives in one place, and it is not here.** An earlier draft
 * of this file resolved stacks from token positions in the store, which
 * would have been a second implementation racing the engine's: the engine
 * owns the camera and the true transforms, and store positions lag during a
 * drag. It now lives in `thunderforge_canvas_core::token_stack`, tested
 * there, and both gestures report through the engine — so the picker can
 * never list a token the click would have missed, by construction rather
 * than by keeping two copies in step.
 */

import type { WorldStore } from "../store";
import type { WorldToken } from "../types";
import type { ControllableToken, FacetContext, IntentResult } from "./types";
import {
  resolveTokenPermissions,
  type MoveIntent,
  type TokenControlFacet,
} from "./tokenControl";

/** A world-space point. */
export interface WorldPoint {
  x: number;
  y: number;
}

/** Tokens the engine resolved at a point, topmost first. */
export interface TokenStack {
  at: WorldPoint;
  members: ControllableToken[];
}

export interface SelectionFacet {
  /** Selected token ids, topmost first. Empty when nothing is selected. */
  selectedIds(): string[];
  /** The selection, decorated with what this principal may do to each. */
  selection(): ControllableToken[];
  /**
   * The stack to offer a picker over, or `null` when there is nothing to
   * choose between — one token, or none. Reads the current selection, which
   * the click preceding the double-click already established.
   */
  disambiguate(): TokenStack | null;
  /** Narrow the selection to one member, e.g. from the picker. */
  selectOne(tokenId: string): void;
  clear(): void;
  /**
   * Move everything selected by the same delta.
   *
   * Each token is adjudicated separately, because they are separately
   * owned: a player dragging a stack containing someone else's token moves
   * their own and is refused the rest, rather than the whole gesture
   * failing. Results come back in selection order, so a caller can say
   * precisely what did not move.
   */
  moveBy(delta: WorldPoint): Promise<IntentResult<MoveIntent>[]>;
}

export function createSelectionFacet(
  store: WorldStore,
  context: FacetContext,
  control: TokenControlFacet,
): SelectionFacet {
  const decorate = (token: WorldToken): ControllableToken =>
    resolveTokenPermissions(token, context.principal);

  const selection = (): ControllableToken[] => {
    const { tokens, selectedTokenIds } = store.getState();
    return (
      selectedTokenIds
        .map((id) => tokens[id])
        // An id the store has never seen is dropped rather than rendered as
        // a blank row: the engine spawns demo tokens that were never synced,
        // and offering one in a picker would let someone pick a token
        // nothing can act on.
        .filter((token): token is WorldToken => token !== undefined)
        .map(decorate)
    );
  };

  return {
    selectedIds: () => [...store.getState().selectedTokenIds],
    selection,

    disambiguate() {
      const members = selection();
      // One token is not a choice, and a one-row picker over it is noise.
      if (members.length < 2) return null;
      return { at: { x: members[0].token.x, y: members[0].token.y }, members };
    },

    selectOne(tokenId) {
      store.dispatch({ type: "select_token", tokenId }, "ui");
    },

    clear() {
      store.dispatch({ type: "select_token", tokenId: null }, "ui");
    },

    async moveBy(delta) {
      const state = store.getState();
      const results: IntentResult<MoveIntent>[] = [];
      for (const tokenId of state.selectedTokenIds) {
        const token = state.tokens[tokenId];
        if (!token) {
          results.push({ status: "refused", reason: "unknown-subject" });
          continue;
        }
        results.push(
          await control.move({
            tokenId,
            x: token.x + delta.x,
            y: token.y + delta.y,
          }),
        );
      }
      return results;
    },
  };
}
