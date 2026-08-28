/**
 * tokens.ts
 * Scene-scoped token sync: mirrors walls.ts's shape (inbound NOTIFY-refetch,
 * outbound mutation bridge) but for the modern `tokens` table
 * (src/server/src/graphql/mutations_tokens.rs), which is what actually
 * persists tokens across a page reload. The legacy `world_tokens`/RxDB
 * path (formerly this directory's index.ts#startWorldSync) has been
 * removed as dead code: it wrote to an RxDB-only collection nothing read
 * back, and posted to `syncWorldMutations`/`publishTokenDeltas` GraphQL
 * mutations that never existed server-side. This module (the real,
 * working sync path) is unaffected by that removal.
 *
 * Two independent responsibilities, matching walls.ts:
 *
 * 1. Inbound: the server emits a generic `world_events` NOTIFY
 *    (subscription field `worldEventsCreated(worldId)`) with
 *    `eventCode = 14` for any token create/update/delete
 *    (src/server/src/world_events.rs::EVENT_CODE_TOKEN_CHANGED). The
 *    notify payload only carries `{ action, tokenId, sceneId }` — not the
 *    full token — so `applyTokenWorldEvent` re-fetches the scene's tokens
 *    via GraphQL (api/tokens.ts#getTokens) and dispatches `upsert_token`/
 *    `remove_token` into the world store.
 *
 * 2. Outbound: unlike walls/lights/shapes, the Bevy engine already owns
 *    click-drag-to-move for tokens end-to-end (engine's
 *    `handle_token_drag` in src/engine/src/systems/selection.rs) and
 *    emits *generic* `upsert_token`/`remove_token` commands directly —
 *    there is no separate "create_token intent" vs "confirmed upsert"
 *    distinction the way there is for walls/lights/shapes, because the
 *    engine has owned tokens' live-drag UX since before scene-scoped
 *    persistence existed. Note also that the engine's two demo tokens
 *    ("player"/"npc", spawned unconditionally at Bevy startup —
 *    src/engine/src/lib.rs's setup — not from any command) use fixed
 *    non-UUID engine ids, which can't be used directly as the `tokens`
 *    table's UUID primary key. `startTokenMutationBridge` therefore keeps
 *    an in-memory `engineIdToTokenId` map: the first `upsert_token` seen
 *    for a given engine id calls `createToken` (server mints the UUID)
 *    and remembers the mapping; every later drag of that same engine id
 *    calls `updateToken` against the remembered server id. The map is
 *    seeded from the scene's current tokens on start (identity-mapped,
 *    since `loadTokensIntoStore` dispatches reloaded tokens using their
 *    server `tokenId` as the engine id), so reloading a scene with
 *    already-persisted tokens routes straight to `updateToken` for them.
 */

import { createToken, deleteToken, getTokens, moveOwnToken, updateToken } from "@/api/tokens";
import type { TokenRecord, UpdateTokenInput } from "@/types/token";
import type { WorldStore } from "../store";
import type { WorldCommand, WorldToken } from "../types";
import { queueEdit, shouldQueue } from "./offlineQueue";

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

const TOKEN_EVENT_CODE = 14;

function tokenRecordToWorldToken(record: TokenRecord): WorldToken {
  const label =
    record.metadata && typeof record.metadata.label === "string"
      ? (record.metadata.label as string)
      : undefined;

  return {
    id: record.tokenId,
    x: record.x,
    y: record.y,
    z: 0,
    label,
    rotation: record.rotation,
    scale: record.scale,
    ownerUserId: record.ownerUserId,
    isPrimary: record.isPrimary,
    photoUrl: record.photoUrl,
    health: record.health,
    maxHealth: record.maxHealth,
  };
}

/**
 * Apply a single `world_events` NOTIFY payload to the world store,
 * re-fetching the affected scene's tokens when it's a token event
 * (eventCode 14). No-ops for any other event code.
 */
export async function applyTokenWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== TOKEN_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | { action?: string; token_id?: string; tokenId?: string; scene_id?: string; sceneId?: string }
    | undefined;

  const eventSceneId = payload?.scene_id ?? payload?.sceneId;
  if (eventSceneId && eventSceneId !== sceneId) {
    return;
  }

  const tokenId = payload?.token_id ?? payload?.tokenId;
  const action = payload?.action;

  if (action === "deleted") {
    if (tokenId) {
      worldStore.dispatch({ type: "remove_token", tokenId }, "sync");
    }
    return;
  }

  // created/updated: the notify payload doesn't carry the full token, so
  // re-fetch the scene's tokens rather than reconstructing one from it.
  const tokens = await getTokens(sceneId);
  for (const token of tokens) {
    worldStore.dispatch(
      { type: "upsert_token", token: tokenRecordToWorldToken(token) },
      "sync",
    );
  }
}

/**
 * Drive applyTokenWorldEvent from a `worldEventsCreated` GraphQL
 * subscription async iterable. Returns a cleanup function that stops
 * consuming the subscription.
 */
export function startTokenEventSync(
  worldStore: WorldStore,
  sceneId: string,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const abortController = new AbortController();

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;
        await applyTokenWorldEvent(worldStore, sceneId, event);
      }
    } catch (error) {
      console.error("World scene tokens event sync error:", error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Load a scene's current tokens and seed the world store with them,
 * replacing the previous hardcoded WorldPage.tsx `initialTokens` fixture.
 * Call once when a scene is opened, before/alongside the live event sync
 * above.
 */
export async function loadTokensIntoStore(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const tokens = await getTokens(sceneId);
  for (const token of tokens) {
    worldStore.dispatch(
      { type: "upsert_token", token: tokenRecordToWorldToken(token) },
      "sync",
    );
  }
}

/**
 * Bridge the engine's generic `upsert_token`/`remove_token` commands
 * (emitted directly by click-drag-to-move, see module docstring above)
 * into GraphQL mutations against the scene-scoped `tokens` table.
 *
 * `engineIdToTokenId` maps the engine's token id (which for the demo
 * "player"/"npc" tokens is a fixed non-UUID string, and for
 * previously-persisted tokens is the server's own UUID `tokenId` — see
 * module docstring) to the server-assigned `tokenId` used for
 * `updateToken`/`deleteToken`. It's seeded from the scene's current
 * tokens (identity-mapped) so a reload routes straight to `updateToken`
 * for tokens that already exist server-side; an engine id never seen
 * before for this scene routes to `createToken` once, and the returned
 * `tokenId` is remembered for every later drag/removal of that same
 * engine id. Returns an unsubscribe function.
 *
 * `isSceneOwner` (spec 004, FR-009/FR-009b): a non-GM caller never creates
 * tokens and never has full-field update rights — every drag they make
 * routes through `moveOwnToken` (position only; the server enforces
 * `owner_user_id = requester`, so a drag on a token they don't control is
 * rejected server-side with no effect, satisfying SC-003) rather than
 * `createToken`/`updateToken`. Resize/rotate are GM-only client-side (no
 * handles rendered for non-GMs, per FR-010), so a non-GM's `upsert_token`
 * commands are position-only by construction.
 */
/**
 * Spec 028 US7: while disconnected, a token edit goes to the outbox instead
 * of the wire.
 *
 * This is the right seam for it because every token edit already funnels
 * through here — the alternative, hooking each call site, is how one path
 * ends up queueing and another silently dropping. The command is stored
 * exactly as the store emitted it, which is what lets the server replay it
 * through the ordinary mutation on reconnect.
 *
 * Creation is deliberately **not** queued (FR-035a): the bridge's create
 * branch is skipped entirely while offline, because precedence cannot settle
 * a create racing a delete without destroying work nobody can see was
 * destroyed. The user is told by `WorldPage`'s indicator, which is already
 * saying that changes are being held.
 */
async function queueTokenEditWhileOffline(
  worldId: string,
  command: WorldCommand,
  isGameMaster: boolean,
): Promise<boolean> {
  const attempt = await queueEdit({
    worldId,
    localId: crypto.randomUUID(),
    kind: "move",
    command,
    isGameMaster,
  });
  if (!attempt.queued && attempt.explanation) {
    console.warn("[offline] change not queued:", attempt.explanation);
  }
  return attempt.queued;
}

export function startTokenMutationBridge(
  worldStore: WorldStore,
  sceneId: string,
  isSceneOwner: boolean,
  worldId?: string,
): () => void {
  const engineIdToTokenId = new Map<string, string>();
  const creating = new Set<string>();

  const ready = getTokens(sceneId)
    .then((tokens) => {
      for (const token of tokens) {
        engineIdToTokenId.set(token.tokenId, token.tokenId);
      }
    })
    .catch((error) => {
      console.error("Failed to seed known scene tokens for mutation bridge:", error);
    });

  const unsubscribe = worldStore.subscribe((event) => {
    // Avoid reacting to our own confirmed dispatches.
    if (event.source === "sync") {
      return;
    }

    const { command } = event;

    if (command.type === "upsert_token") {
      const { token } = command;

      void ready.then(async () => {
        const knownTokenId = engineIdToTokenId.get(token.id);

        // US7. Asked before anything is sent, rather than after a mutation
        // fails: firing into a dead socket costs a timeout per edit before
        // the user sees their token move, which is what makes offline play
        // feel broken rather than merely disconnected.
        if (worldId && shouldQueue()) {
          if (!knownTokenId) {
            // A token this scene has never seen is a creation, and FR-035a
            // refuses those offline.
            return;
          }
          await queueTokenEditWhileOffline(
            worldId,
            { type: "upsert_token", token: { ...token, id: knownTokenId } } as WorldCommand,
            isSceneOwner,
          );
          return;
        }

        if (knownTokenId) {
          if (isSceneOwner) {
            // Resize/rotate (US2, FR-006/FR-007) are GM-only and travel
            // through this same generic `upsert_token` engine event —
            // forwarded only when present so a plain move doesn't churn
            // scale/rotation on every drag.
            const input: UpdateTokenInput = {
              x: token.x,
              y: token.y,
            };
            if (token.rotation !== undefined) {
              input.rotation = token.rotation;
            }
            if (token.scale !== undefined) {
              input.scale = token.scale;
            }
            // Token art (TokenTool's art picker). Forwarded only when the
            // command actually carries it, for the same reason as
            // rotation/scale above — a drag must not rewrite it. `null` is
            // meaningful here and must survive: it is how the GM removes
            // art, as distinct from `undefined`, which leaves it alone.
            if (token.photoUrl !== undefined) {
              input.photoUrl = token.photoUrl;
            }
            void updateToken(knownTokenId, input).catch((error) => {
              console.error("Failed to update token:", error);
            });
          } else {
            // Spec 004 FR-009: non-GM callers only ever move a token they
            // control; the server enforces owner_user_id = requester and
            // rejects anything else with no effect.
            void moveOwnToken(knownTokenId, token.x, token.y).catch((error) => {
              console.error("Failed to move own token:", error);
            });
          }
          return;
        }

        // Non-GM callers never create tokens (FR-009b) — only ever move
        // one already known to this scene.
        if (!isSceneOwner) {
          return;
        }

        // First time this engine id has been seen for this scene: create
        // it. Guard against a second drag firing before this request
        // resolves (createToken always mints a fresh server tokenId, so a
        // duplicate call here would create a duplicate row).
        if (creating.has(token.id)) {
          return;
        }
        creating.add(token.id);

        void createToken({
          sceneId,
          x: token.x,
          y: token.y,
          metadata: token.label ? { label: token.label } : undefined,
        })
          .then((created) => {
            engineIdToTokenId.set(token.id, created.tokenId);
          })
          .catch((error) => {
            console.error("Failed to create token:", error);
          })
          .finally(() => {
            creating.delete(token.id);
          });
      });
      return;
    }

    if (command.type === "remove_token") {
      const { tokenId: engineId } = command;

      void ready.then(() => {
        const knownTokenId = engineIdToTokenId.get(engineId);
        if (!knownTokenId) {
          return;
        }

        void deleteToken(knownTokenId)
          .then((ok) => {
            if (ok) {
              engineIdToTokenId.delete(engineId);
            }
          })
          .catch((error) => {
            console.error("Failed to delete token:", error);
          });
      });
    }
  });

  return unsubscribe;
}
