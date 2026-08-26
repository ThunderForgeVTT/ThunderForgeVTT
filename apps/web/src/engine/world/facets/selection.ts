/**
 * The selection facet, including stacked tokens.
 *
 * Tokens stack. Two characters share a doorway, a familiar sits on its
 * owner, a swarm occupies one square — and every token in a freshly created
 * scene spawns on the same spot, which is how this came up: a click selects
 * whichever the hit-test happens to find on top, and the others are
 * unreachable without dragging the pile apart one at a time.
 *
 * Treating that as a hazard produces fiddly UI. Treating it as a fact
 * produces two gestures:
 *
 * - **Click** selects the whole stack. Moving the selection moves every
 *   token in it, which is what someone dragging a pile of tokens off a
 *   doorway actually means.
 * - **Double-click** disambiguates: the caller renders a small picker over
 *   `members` and calls `selectOne` with what the user chose.
 *
 * Both read the same `resolveStack`, so the picker can never list a token
 * the click would not have caught.
 *
 * The world store holds a single `selectedTokenId` today. This facet keeps
 * the full selection itself and mirrors the primary member into the store,
 * so existing single-selection UI (`TokenTool`, the engine's own highlight)
 * keeps working untouched while callers that understand stacks get the
 * whole set.
 */

import type { WorldStore } from "../store";
import type { WorldToken } from "../types";
import type { ControllableToken, FacetContext, IntentResult } from "./types";
import { resolveTokenPermissions, type MoveIntent, type TokenControlFacet } from "./tokenControl";

/** A world-space point. */
export interface WorldPoint {
  x: number;
  y: number;
}

/** Tokens found at a point, topmost first. */
export interface TokenStack {
  at: WorldPoint;
  members: ControllableToken[];
}

/**
 * How close a token's centre must be to count as "at" the point, in world
 * units, when the caller does not say.
 *
 * A hit radius rather than the token's own bounds: art is fitted inside the
 * footprint and may be much narrower than its cell (a starship is), so
 * bounds-testing the sprite would make a token harder to click the less
 * square its art happens to be — a rule no player could predict.
 */
export const DEFAULT_HIT_RADIUS = 32;

/**
 * Tokens whose centre lies within `radius` of `point`, topmost first.
 *
 * Pure, so the ordering the picker shows and the ordering a click resolves
 * are provably the same list. Ties on `z` fall back to id so the order is
 * stable between calls — a picker whose entries reshuffle as you reach for
 * one is worse than no picker.
 */
export function resolveStack(
  tokens: WorldToken[],
  point: WorldPoint,
  radius: number = DEFAULT_HIT_RADIUS,
): WorldToken[] {
  return tokens
    .filter((token) => Math.hypot(token.x - point.x, token.y - point.y) <= radius)
    .sort((a, b) => (b.z ?? 0) - (a.z ?? 0) || a.id.localeCompare(b.id));
}

export interface SelectionFacet {
  /** Currently selected token ids, topmost first. */
  selectedIds(): string[];
  /** What a click at `point` would catch, without selecting it. */
  stackAt(point: WorldPoint, radius?: number): TokenStack;
  /** Single click: select every token at the point. */
  selectStack(point: WorldPoint, radius?: number): TokenStack;
  /**
   * Double click: the same stack, for the caller to render a picker over.
   * Selection is left untouched until `selectOne` is called, so dismissing
   * the picker leaves the board exactly as it was.
   */
  disambiguate(point: WorldPoint, radius?: number): TokenStack;
  /** Narrow the selection to one member, e.g. from the picker. */
  selectOne(tokenId: string): void;
  clear(): void;
  /**
   * Move everything selected by the same delta.
   *
   * Each token is adjudicated separately, because they are separately
   * owned: a player dragging a stack containing someone else's token moves
   * their own and is refused the rest, rather than the whole gesture
   * failing. Results come back in selection order so a caller can report
   * precisely what did not move.
   */
  moveBy(delta: WorldPoint): Promise<IntentResult<MoveIntent>[]>;
}

export function createSelectionFacet(
  store: WorldStore,
  context: FacetContext,
  control: TokenControlFacet,
): SelectionFacet {
  let selected: string[] = [];

  const decorate = (token: WorldToken): ControllableToken =>
    resolveTokenPermissions(token, context.principal);

  const stackAt = (point: WorldPoint, radius?: number): TokenStack => ({
    at: point,
    members: resolveStack(Object.values(store.getState().tokens), point, radius).map(decorate),
  });

  /** Mirrors the primary member into the store's single-selection slot. */
  const publishPrimary = () => {
    store.dispatch({ type: "select_token", tokenId: selected[0] ?? null }, "ui");
  };

  return {
    selectedIds: () => [...selected],
    stackAt,
    disambiguate: stackAt,

    selectStack(point, radius) {
      const stack = stackAt(point, radius);
      selected = stack.members.map((entry) => entry.token.id);
      publishPrimary();
      return stack;
    },

    selectOne(tokenId) {
      selected = [tokenId];
      publishPrimary();
    },

    clear() {
      selected = [];
      publishPrimary();
    },

    async moveBy(delta) {
      const state = store.getState();
      const results: IntentResult<MoveIntent>[] = [];
      for (const tokenId of selected) {
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
