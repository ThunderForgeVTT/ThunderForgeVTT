import { useCallback, useRef, useSyncExternalStore } from "react";

import type { SceneToken } from "@/types/token";

import type { WorldStore } from "./store";
import type { WorldToken } from "./types";

const NONE: SceneToken[] = [];

/**
 * The scene's tokens as the world store holds them, following every token
 * event: a token placed, changed or removed, by anyone.
 *
 * The store holds only the scene on screen (WorldPage empties it on a scene
 * change), so `sceneId` is a guard for the moment between scenes, and the
 * scene of a token the store has not yet read back from the server (one the
 * engine has only just made).
 *
 * The list is the same array until something a panel reads changes. A drag
 * writes the store on every step; without this, every panel reading tokens
 * would re-render with it.
 */
export function useSceneTokens(
  store: WorldStore,
  sceneId: string | null,
): SceneToken[] {
  const held = useRef<{
    tokens: Readonly<Record<string, WorldToken>> | null;
    sceneId: string | null;
    list: SceneToken[];
  }>({ tokens: null, sceneId: null, list: NONE });

  const getSnapshot = useCallback(() => {
    const tokens = store.getState().tokens;
    const last = held.current;
    if (last.tokens === tokens && last.sceneId === sceneId) return last.list;
    const next = sceneId ? sceneTokensOf(tokens, sceneId) : NONE;
    const list = sameSceneTokens(last.list, next) ? last.list : next;
    held.current = { tokens, sceneId, list };
    return list;
  }, [store, sceneId]);

  return useSyncExternalStore(store.subscribe, getSnapshot);
}

export function sceneTokensOf(
  tokens: Readonly<Record<string, WorldToken>>,
  sceneId: string,
): SceneToken[] {
  const result: SceneToken[] = [];
  for (const token of Object.values(tokens)) {
    if (token.sceneId !== undefined && token.sceneId !== sceneId) continue;
    result.push(toSceneToken(token, sceneId));
  }
  return result;
}

function toSceneToken(token: WorldToken, sceneId: string): SceneToken {
  return {
    tokenId: token.id,
    sceneId: token.sceneId ?? sceneId,
    levelId: token.levelId ?? "",
    actorId: token.actorId ?? null,
    ownerUserId: token.ownerUserId ?? null,
    isPrimary: token.isPrimary ?? false,
    photoUrl: token.photoUrl ?? null,
    tokenType: (token.tokenType ?? "character") as SceneToken["tokenType"],
    linked: token.linked ?? false,
    name: token.label ?? null,
    nameVisibleToPlayers: token.nameHidden !== true,
    ...(token.conditions ? { conditions: token.conditions } : {}),
  };
}

export function sameSceneTokens(a: SceneToken[], b: SceneToken[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((token, i) => sameSceneToken(token, b[i]));
}

function sameSceneToken(a: SceneToken, b: SceneToken): boolean {
  const keys = Object.keys(a) as (keyof SceneToken)[];
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every((key) =>
    key === "conditions"
      ? JSON.stringify(a.conditions) === JSON.stringify(b.conditions)
      : a[key] === b[key],
  );
}
