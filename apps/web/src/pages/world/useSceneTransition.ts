import { useEffect, useState, type RefObject } from "react";
import {
  beginSceneTransition,
  completeSceneTransition,
  onSceneLoadRequested,
} from "@/engine/bevy";
import type { SceneLoadState } from "./useSceneLoadState";

interface UseSceneTransitionParams {
  sceneId: string | null;
  sceneLoadGeneration: number;
  sceneLoadGenerationRef: RefObject<number>;
  sceneLoadState: SceneLoadState;
  bridgeReady: boolean;
  engineReady: boolean;
  sceneLevelSettled: boolean;
}

/**
 * The engine's scene transition, from chrome's side (spec 031 US4, FR-018).
 *
 * Moved out of `WorldPage` unchanged. Returns whether the engine has granted
 * the current scene's load, which every content loader is gated on.
 */
export function useSceneTransition({
  sceneId,
  sceneLoadGeneration,
  sceneLoadGenerationRef,
  sceneLoadState,
  bridgeReady,
  engineReady,
  sceneLevelSettled,
}: UseSceneTransitionParams) {
  /**
   * Which scene the engine has cleared the canvas for, and for which attempt.
   *
   * Spec 031 (US4, FR-018). Unloading the previous scene is the engine's job
   * now — it owns the entities, and it despawns tokens, walls, lights, shapes,
   * interactives and the selection together on one frame. What chrome owns is
   * the network, so the two have to be ordered, and this is the token that
   * orders them: the loaders below run only once the engine says it has
   * finished clearing and is waiting.
   *
   * Firing the loaders in parallel with `beginSceneTransition` was the
   * rejected alternative and the reason this exists. The despawn lands on the
   * engine's next frame while a fetch is network-bound, so it *usually* wins —
   * and the times it does not, it deletes the scene that just arrived. A bug
   * that empties a map one switch in fifty is worse than one that empties it
   * every time, because nobody can reproduce it.
   *
   * Carrying the generation as well as the scene id, because a retry of the
   * same scene is a second attempt that must not be satisfied by the first
   * attempt's grant.
   */
  const [sceneLoadGrant, setSceneLoadGrant] = useState<{
    sceneId: string;
    generation: number;
  } | null>(null);

  const sceneLoadGranted =
    sceneLoadGrant !== null &&
    sceneLoadGrant.sceneId === sceneId &&
    sceneLoadGrant.generation === sceneLoadGeneration;

  // The engine reporting that the canvas is empty and it is waiting for the
  // new scene. Subscribed once for the life of the page rather than per
  // scene: the machine can be re-pointed mid-transition, so the event's own
  // `sceneId` is the trustworthy answer to "which scene is this for", not
  // whatever the closure that started it captured.
  useEffect(
    () =>
      onSceneLoadRequested((event) => {
        setSceneLoadGrant({
          sceneId: event.sceneId,
          generation: sceneLoadGenerationRef.current,
        });
      }),
    // A ref from `useSceneLoadState`: its identity never changes, so this
    // still subscribes exactly once.
    [sceneLoadGenerationRef],
  );

  /**
   * Ask the engine to change scenes, and grant the load ourselves if it cannot.
   *
   * `beginSceneTransition` returning false means no despawn will happen and no
   * `scene_load_requested` will ever be emitted — an engine bundle predating
   * the machine, or a wasm layer that is unreachable. Waiting for an event
   * that cannot arrive would turn a degraded engine into a scene that never
   * loads at all, so chrome grants itself the load and proceeds exactly as it
   * did before the machine existed. `engineReady` is folded into the same
   * answer for the same reason: the machine's systems only run inside a
   * running app, so an engine that has not started cannot reply either.
   *
   * This runs for the *first* scene too, not only for switches. A first load
   * has nothing to unload, but giving it its own ungated path would leave the
   * only-once code path as the untested one.
   */
  useEffect(() => {
    // Scene levels: nothing is loaded until it is known which level to load.
    // Every loader behind the grant asks for "the level on screen", and
    // before that is settled the answer would be the server's default — the
    // entry level — which for a player standing upstairs is a floor they are
    // answered nothing for, loaded and then thrown away.
    if (!sceneId || !bridgeReady || !sceneLevelSettled) {
      return;
    }
    // One transition per scene per attempt. Without this, the engine finishing
    // its boot after chrome had already granted itself the load (`engineReady`
    // flipping true a moment after `bridgeReady`) would begin a transition for
    // the scene that was *already loading*: the despawn would take content the
    // loaders had finished delivering, and nothing would re-fetch it, because
    // the grant those effects watch would not have changed. A scene wiped
    // seconds after it appeared, once per session, at boot.
    if (sceneLoadGranted) {
      return;
    }

    const generation = sceneLoadGeneration;
    let cancelled = false;

    void (
      engineReady ? beginSceneTransition(sceneId) : Promise.resolve(false)
    ).then((began) => {
      if (cancelled || began) {
        return;
      }
      setSceneLoadGrant({ sceneId, generation });
    });

    return () => {
      cancelled = true;
    };
  }, [
    sceneId,
    sceneLoadGeneration,
    bridgeReady,
    engineReady,
    sceneLoadGranted,
    sceneLevelSettled,
  ]);

  /**
   * Tell the engine chrome has stopped sending, however the load ended.
   *
   * `Loading` has no timeout by design — the engine owns no network and is in
   * no position to decide chrome has failed — so every exit from the load
   * lifecycle has to end here or the machine waits forever. There are four,
   * and all four arrive as a `sceneLoadState` that is no longer `loading`:
   * every resource loaded; a resource failed (`markSceneResourceFailed`); a
   * scene with no imported art, which satisfies "background" without fetching;
   * and no scene record at all, which satisfies it the same way.
   *
   * The error branch completes deliberately. The alternative — hold the
   * machine in `Loading` because the scene is incomplete — states something
   * false: chrome is not still loading, it has given up, and the retry button
   * the error surface offers begins a *new* transition anyway. A partly
   * loaded scene the Game Master can see and retry beats a machine stuck
   * describing a load that ended minutes ago.
   *
   * Two paths deliberately do not complete, because neither is a transition:
   * no `sceneId` at all (the engine refuses an empty id, so nothing was ever
   * begun), and unmounting the page mid-load. The wasm module outlives the
   * React tree, so an unmount can leave the machine in `Loading` — harmless,
   * because nothing reads the canvas while the page is gone and the next
   * `begin_scene_transition` supersedes it whatever state it was in.
   */
  useEffect(() => {
    if (!sceneLoadGranted || sceneLoadState.status === "loading") {
      return;
    }
    void completeSceneTransition();
  }, [sceneLoadGranted, sceneLoadState]);

  return sceneLoadGranted;
}
