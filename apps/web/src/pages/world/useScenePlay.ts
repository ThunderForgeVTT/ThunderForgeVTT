import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import {
  refreshInteractives,
  setScenePlaying,
  startTriggerBridge,
} from "@/engine/world/sync";
import type { WorldStore } from "@/engine/world/store";
import { onOpenLore } from "@/engine/bevy";
import { getMyActorClaim } from "@/api/actorClaims";
import { appHref } from "@/lib/appHref";

interface UseScenePlayParams {
  id: string;
  sceneId: string | null;
  worldStore: WorldStore;
  bridgeReady: boolean;
  sceneLoadGranted: boolean;
  sceneLoadGeneration: number;
  setApprovalRevision: Dispatch<SetStateAction<number>>;
}

/**
 * The scene as played: its interactive elements, the character this viewer
 * plays, and the lore pages interactives open.
 *
 * Moved out of `WorldPage` unchanged. Returns the claimed actor's id.
 */
export function useScenePlay({
  id,
  sceneId,
  worldStore,
  bridgeReady,
  sceneLoadGranted,
  sceneLoadGeneration,
  setApprovalRevision,
}: UseScenePlayParams) {
  /**
   * Spec 030: interactive elements.
   *
   * Three things, and they belong together because each is useless without the
   * others: load what this viewer may interact with, tell the engine the scene
   * is being *played* (region entry fires on nothing otherwise — FR-032), and
   * bridge engine-detected triggers back to the server, which decides.
   */
  useEffect(() => {
    // Scene levels: gated on the engine's grant like the other loaders, and
    // re-run by the load generation. Interactives are per level, and a level
    // switch changes neither `sceneId` nor `bridgeReady` — without the grant
    // in this list the new floor's stairs would never be read.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    void refreshInteractives(worldStore, sceneId).catch((error: unknown) => {
      console.error("Failed to load interactive elements:", error);
    });

    // The play view is, by definition, play. Preparation happens on the
    // staging route, which does not mount this.
    setScenePlaying(worldStore, true);

    const stopTriggers = startTriggerBridge(worldStore, (result) => {
      if (result.outcome === "requested") {
        setApprovalRevision((n) => n + 1);
      }
    });

    return () => {
      stopTriggers();
      // A scene nobody is looking at is not being played, and leaving this on
      // would let a background tab's stale positions fire a region.
      setScenePlaying(worldStore, false);
    };
    // `setApprovalRevision` is the page's state setter: stable.
  }, [
    sceneId,
    bridgeReady,
    worldStore,
    sceneLoadGranted,
    sceneLoadGeneration,
    setApprovalRevision,
  ]);

  /**
   * Which character the person at this keyboard is playing.
   *
   * Needed only to know where a picked-up item goes (spec 031 FR-015). `null`
   * is an ordinary answer, not a failure: a Game Master has no claim, and the
   * pickup prompt says so rather than offering an action that cannot work.
   */
  const [claimedActorId, setClaimedActorId] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getMyActorClaim(id)
      .then((claim) => {
        if (active) {
          setClaimedActorId(claim?.actorId ?? null);
        }
      })
      .catch(() => {
        // A claim that cannot be read leaves Pick up unavailable and explained,
        // which is the same state as having no claim. Nothing on the map
        // depends on this, so there is nothing to report.
        if (active) {
          setClaimedActorId(null);
        }
      });
    return () => {
      active = false;
    };
  }, [id]);

  /**
   * Spec 030: an interactive asked for a lore page to be opened.
   *
   * The engine resolves *which* entry; opening a tab needs this application's
   * URL structure, which is chrome and belongs on this side (Principle I).
   */
  useEffect(
    () =>
      onOpenLore((event) => {
        window.open(
          appHref(`/world/${id}/lore/${event.entryId}`),
          "_blank",
          "noopener,noreferrer",
        );
      }),
    [id],
  );

  return claimedActorId;
}
