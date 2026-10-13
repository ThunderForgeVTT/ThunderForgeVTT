import { useEffect, useMemo, useRef, useState } from "react";
import { reportPlayPausedInSyncReason } from "@/api/playPauseSignal";
import type { WorldStore } from "@/engine/world/store";
import { reportWorldCacheSync } from "@/services/worldCacheDiagnostics";
import type { AuthUser } from "@/types/auth";

interface UseEngineBridgeParams {
  id: string;
  user: AuthUser | null;
  isSceneOwner: boolean;
  worldStore: WorldStore;
  engineReady: boolean;
}

/**
 * The world store's bond with React and with the engine.
 *
 * Moved out of `WorldPage` unchanged: React's copy of the store's state, the
 * engine-side binding (`bridgeReady`), the world cache sync on open, and who
 * the engine is told is looking — Game Master, viewer token, party eyes.
 */
export function useEngineBridge({
  id,
  user,
  isSceneOwner,
  worldStore,
  engineReady,
}: UseEngineBridgeParams) {
  const loaded = useRef(false);
  const [worldState, setWorldState] = useState(() => worldStore.getState());
  // True once `bindWorldStore` has actually finished registering its
  // store subscription (the thing that forwards "sync"-sourced dispatches
  // — e.g. loadWallsIntoStore's confirmed wall rows — into the engine via
  // apply_world_command). NOT the same as `engineReady`: bindWorldStore is
  // reached through its own `import("@/engine/bevy")` dynamic-chunk hop,
  // which `useCanvasEngine`'s static-imported `mountEngine` doesn't pay,
  // so `engineReady` can and does flip true before this subscription
  // exists. Without gating on this specifically (found via
  // specs/002-canvas-authoring-asset-storage T014's live debugging),
  // walls/tokens/lights/shapes fetched on mount/reload can lose the race:
  // the GraphQL fetch resolves and dispatches "upsert_wall" (etc.) before
  // anyone is listening to relay it into the engine's WallSet/TokenSet/
  // etc. — the dispatch still updates React state correctly, so nothing
  // looks broken in the UI's data, but the engine-side resource silently
  // stays empty (e.g. no wall is selectable-by-click after a reload, even
  // though it's genuinely persisted and the property panel would show it
  // if selection worked). Every effect that dispatches something meant to
  // reach the engine gates on this now.
  const [bridgeReady, setBridgeReady] = useState(false);

  useEffect(
    () => worldStore.subscribe((event) => setWorldState(event.state)),
    [worldStore],
  );

  useEffect(() => {
    if (loaded.current) {
      return;
    }

    loaded.current = true;

    // Engine is now mounted via useCanvasEngine hook
    // Still need to bind world store for mutations
    void import("@/engine/bevy").then(({ bindWorldStore }) =>
      bindWorldStore(worldStore).then(() => setBridgeReady(true)),
    );
  }, [worldStore]);

  // Spec 028 (US1): bring the local world cache into agreement with the
  // server for the world being opened, before its assets are asked for.
  //
  // Runs on identity, not on `engineReady`: the sync is a network round trip
  // plus OPFS/IndexedDB work that has nothing to do with the render loop, and
  // the sooner it finishes the more of this visit's asset loads can be served
  // from disk. Anything it publishes late simply misses those loads, which
  // fall back to the network — the same outcome as no cache at all.
  //
  // Not awaited by anything and not gated on. `syncWorldCache` never throws
  // and never rejects: a browser without OPFS, a missing session key, an
  // unreachable server or a malformed plan all resolve to a "degraded"
  // summary and leave the page on exactly today's behaviour. A cache problem
  // must not be able to stop a world from opening.
  useEffect(() => {
    const userId = user?.id;
    if (!id || !userId) {
      return;
    }

    void import("@/engine/bevy").then(({ syncWorldCache }) =>
      syncWorldCache(id, userId).then((summary) => {
        // The only observation this side makes of the cache, and it is
        // diagnostic: what was held, fetched and evicted, never *which*
        // items. See `WorldCacheSyncSummary`.
        console.debug("[world-cache] sync", summary);
        // And kept, so the diagnostics panel can report the repairs and
        // evictions this open performed (FR-051). The sync happens once, on
        // open; a panel opened later would otherwise have nothing to show
        // and no way to get it without re-running the sync.
        reportWorldCacheSync(summary);
        // Spec 051 US2 (T039): the sync plan is the first thing opening a
        // world asks for, and a paused world refuses it. The engine's fetch
        // does not pass through `graphqlClient`, so the refusal is read out
        // of the summary here and sent down the same road as every other
        // pause, to the notice. Without it, a table opening a paused world
        // would sit on a playfield whose other requests fail one by one.
        if (summary?.status === "degraded") {
          reportPlayPausedInSyncReason(id, summary.reason);
        }
      }),
    );
  }, [id, user?.id]);

  // FR-010: tell the engine whether this session may author walls/shapes.
  // Without this, WallPlugin/ShapePlugin's IsGameMaster resource stays at
  // its `false` default forever and no click/keyboard authoring input is
  // ever accepted, regardless of `isSceneOwner`. Re-sent whenever
  // ownership resolves/changes or the engine (re)becomes ready, since a
  // fresh `start()` always resets the wasm side back to the default.
  useEffect(() => {
    if (!engineReady) {
      return;
    }

    // Spec 082: and who is looking, so a player's board offers a handle
    // only on the drawings they made.
    void import("@/engine/bevy").then(({ setIsGameMaster, setViewerUser }) => {
      setIsGameMaster(isSceneOwner);
      setViewerUser(user?.id ?? null);
    });
  }, [engineReady, isSceneOwner, user?.id]);

  // Playtest 2026-09-10 P9: a player sees the board through their own token —
  // their primary one, else any they own — so walls hide what it cannot see.
  // A Game Master sees through none. The engine used to use its own demo
  // token as everyone's eyes, the Game Master's included.
  const myTokens =
    isSceneOwner || !user?.id
      ? []
      : Object.values(worldState.tokens).filter(
          (token) => token.ownerUserId === user.id,
        );
  const viewerTokenId =
    (myTokens.find((token) => token.isPrimary) ?? myTokens[0])?.id ?? null;

  // Spec 045 FR-033: the party's eyes, for a Game Master's board alone.
  //
  // Each owner resolved to one token by the same rule FR-001 gives a player
  // for their own — their primary, else any they own — so what the Game
  // Master's marks are computed against is exactly what each player is
  // looking through, rather than "every character token", which would count
  // an unclaimed pregen as a pair of eyes.
  //
  // Sorted and joined into a string so the effect below re-runs when the
  // *set* changes and not on every re-render that rebuilds the array.
  const allTokens = worldState.tokens;
  const partyEyesKey = useMemo(() => {
    if (!isSceneOwner) {
      return "";
    }
    const byOwner = new Map<string, (typeof allTokens)[string][]>();
    for (const token of Object.values(allTokens)) {
      if (!token.ownerUserId) {
        continue;
      }
      const owned = byOwner.get(token.ownerUserId) ?? [];
      owned.push(token);
      byOwner.set(token.ownerUserId, owned);
    }
    return [...byOwner.values()]
      .map((owned) => (owned.find((t) => t.isPrimary) ?? owned[0]).id)
      .sort()
      .join(",");
  }, [isSceneOwner, allTokens]);

  useEffect(() => {
    if (!engineReady) {
      return;
    }
    void import("@/engine/bevy").then(
      ({ setViewerToken, setControlledToken }) => {
        void setViewerToken(viewerTokenId);
        // Spec 045: the same token is theirs to move. Sight and control are
        // asked separately because a Game Master has no viewer token and may
        // still move any token; moving a *selected* token is its own
        // affordance, not this one.
        void setControlledToken(viewerTokenId);
      },
    );
  }, [engineReady, viewerTokenId]);

  useEffect(() => {
    if (!engineReady) {
      return;
    }
    void import("@/engine/bevy").then(({ setPartyEyes }) => {
      void setPartyEyes(partyEyesKey ? partyEyesKey.split(",") : []);
    });
    // `partyEyesKey` rather than `partyEyes`: the array is rebuilt on every
    // token move, and re-sending an unchanged list each time would ask the
    // engine to re-resolve the party several times a second.
  }, [engineReady, partyEyesKey]);

  return { worldState, bridgeReady, allTokens };
}
