/**
 * The token-control facet: what this principal may do to which tokens.
 *
 * "A player controls their own token" is nearly right and wrong in the two
 * places that matter. A player may control *several* tokens — a familiar, a
 * summon, a vehicle, a second character — so ownership is a filter over the
 * scene's tokens, never a single id. And control is not one permission:
 * today's server lets a player move a token they own, lets only the scene
 * owner change its size or facing (spec 004 FR-010), and lets a player set
 * art on their own *primary* token through a different mutation entirely
 * (`setOwnPrimaryTokenPhoto`). Those are three different answers and a
 * single `canControl` boolean cannot give them.
 *
 * So permissions are resolved once, by `resolveTokenPermissions`, and both
 * the UI and this facet read that same answer. A control that is enabled
 * while the intent behind it would be refused is a bug the types should
 * make awkward to write.
 *
 * This facet proposes; it does not decide. Every mutating call goes through
 * the `Adjudicator`, so the same code path serves today's optimistic local
 * dispatch and a future Crucible round trip that can answer `Adjusted`.
 */

import type { WorldStore } from "../store";
import type { WorldToken } from "../types";
import type {
  ControllableToken,
  FacetContext,
  FacetPrincipal,
  IntentResult,
} from "./types";

/** A requested move, in world units. The payload Crucible receives for
 *  `ActionKind::Move`. */
export interface MoveIntent {
  tokenId: string;
  x: number;
  y: number;
}

/** A requested change to a token's presentation. Crucible sees this as
 *  `ActionKind::Manipulate`. */
export interface ManipulateIntent {
  tokenId: string;
  rotation?: number;
  scale?: number;
  /** `null` removes the art; omitted leaves it alone. The distinction is
   *  carried end to end — see `UpdateTokenInput.photoUrl`. */
  photoUrl?: string | null;
}

export interface TokenControlFacet {
  /** Every token in the scene, each with its resolved permissions. */
  tokens(): ControllableToken[];
  /** Just the ones this principal may move — the player's own, or all of
   *  them for a GM. Plural by construction. */
  controllable(): ControllableToken[];
  /** Permissions for one token, or `null` if the store does not know it. */
  permissions(tokenId: string): ControllableToken | null;
  move(intent: MoveIntent): Promise<IntentResult<MoveIntent>>;
  manipulate(intent: ManipulateIntent): Promise<IntentResult<ManipulateIntent>>;
}

/**
 * Resolves what `principal` may do to `token`.
 *
 * Pure, and deliberately mirrors the server's own rules rather than
 * inventing friendlier ones — a client that permits what the server refuses
 * produces an action that silently fails, which is worse than a disabled
 * button.
 */
export function resolveTokenPermissions(
  token: WorldToken,
  principal: FacetPrincipal,
): ControllableToken {
  const isGm = principal.authority === "gm";
  const owns =
    principal.authority === "player" &&
    principal.userId !== null &&
    token.ownerUserId === principal.userId;

  return {
    token,
    // `moveOwnToken` exists for exactly this: the server re-checks that
    // `owner_user_id` is the requester, so a player moving someone else's
    // token is refused server-side regardless of what the client allows.
    canMove: isGm || owns,
    // Spec 004 FR-010: size and facing are the GM's, and non-GMs are not
    // even rendered handles for them.
    canRotate: isGm,
    canResize: isGm,
    // A player may set art on their own primary token — a different
    // mutation with its own authorisation — but not on any other.
    canSetArt: isGm || (owns && token.isPrimary === true),
    canDelete: isGm,
  };
}

export function createTokenControlFacet(
  store: WorldStore,
  context: FacetContext,
): TokenControlFacet {
  const all = (): ControllableToken[] =>
    Object.values(store.getState().tokens).map((token) =>
      resolveTokenPermissions(token, context.principal),
    );

  const find = (tokenId: string): ControllableToken | null =>
    all().find((entry) => entry.token.id === tokenId) ?? null;

  return {
    tokens: all,
    controllable: () => all().filter((entry) => entry.canMove),
    permissions: find,

    async move(intent) {
      const entry = find(intent.tokenId);
      if (!entry) {
        return { status: "refused", reason: "unknown-subject" };
      }
      if (!entry.canMove) {
        return {
          status: "refused",
          reason: context.principal.authority === "observer" ? "observer" : "not-yours",
        };
      }

      const result = await context.adjudicator.resolve<MoveIntent>({
        worldId: context.worldId,
        actorId: intent.tokenId,
        kind: "move",
        payload: intent,
      });

      // The *authoritative* position is applied, not the requested one.
      // When the ruleset adjusts a move — a wall, a speed limit — this is
      // the line that makes the client show what actually happened rather
      // than what was asked for.
      if (result.status === "accepted" || result.status === "adjusted") {
        store.dispatch(
          {
            type: "upsert_token",
            token: { ...entry.token, x: result.value.x, y: result.value.y },
          },
          "ui",
        );
      }
      return result;
    },

    async manipulate(intent) {
      const entry = find(intent.tokenId);
      if (!entry) {
        return { status: "refused", reason: "unknown-subject" };
      }

      const wantsFacing = intent.rotation !== undefined || intent.scale !== undefined;
      const wantsArt = intent.photoUrl !== undefined;
      if (wantsFacing && !(entry.canRotate && entry.canResize)) {
        return { status: "refused", reason: "gm-only" };
      }
      if (wantsArt && !entry.canSetArt) {
        return { status: "refused", reason: entry.canMove ? "gm-only" : "not-yours" };
      }

      const result = await context.adjudicator.resolve<ManipulateIntent>({
        worldId: context.worldId,
        actorId: intent.tokenId,
        kind: "manipulate",
        payload: intent,
      });

      if (result.status === "accepted" || result.status === "adjusted") {
        const { rotation, scale, photoUrl } = result.value;
        store.dispatch(
          {
            type: "upsert_token",
            token: {
              ...entry.token,
              ...(rotation !== undefined ? { rotation } : {}),
              ...(scale !== undefined ? { scale } : {}),
              ...(photoUrl !== undefined ? { photoUrl } : {}),
            },
          },
          "ui",
        );
      }
      return result;
    },
  };
}
