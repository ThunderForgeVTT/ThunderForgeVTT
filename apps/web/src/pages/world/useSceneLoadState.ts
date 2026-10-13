import { useCallback, useEffect, useRef, useState } from "react";

/** A piece of a scene the board waits on before the loading overlay lifts. */
export type SceneLoadResource =
  | "background"
  | "walls"
  | "tokens"
  | "lights"
  | "shapes";
export type SceneLoadState =
  | { status: "loading" }
  | { status: "ready" }
  | { status: "error"; failedResource: SceneLoadResource };

/**
 * Whether the scene on the board has finished loading, and the means to say
 * so (spec 004, US4).
 *
 * Moved out of `WorldPage` unchanged. The page's loaders report into
 * `markSceneResourceLoaded` / `markSceneResourceFailed`, each with the
 * generation it started under, and `retrySceneLoad` starts a new generation.
 */
export function useSceneLoadState({ sceneId }: { sceneId: string | null }) {
  // Spec 004 (US4, T034-T036): scene-switch loading/error feedback. Before
  // this, the four per-scene loaders below (walls/tokens/lights/shapes)
  // plus the background-image dispatch had zero UI signal on failure —
  // just a `console.error`, per contracts/scene-load-state.md's
  // documented gap. `sceneLoadGeneration` is the "retry"/"latest wins"
  // mechanism (T036): bumping it both re-triggers every loader effect
  // below (added to each one's dependency array) and invalidates any
  // in-flight response from a stale scene/attempt via
  // `sceneLoadGenerationRef` — a loader that resolves after the user has
  // already switched scenes or hit retry again must not resurrect a
  // finished "loading" state or overwrite a newer error.
  const [sceneLoadGeneration, setSceneLoadGeneration] = useState(0);
  const sceneLoadGenerationRef = useRef(0);
  useEffect(() => {
    sceneLoadGenerationRef.current = sceneLoadGeneration;
  }, [sceneLoadGeneration]);

  const [sceneLoadState, setSceneLoadState] = useState<SceneLoadState>({
    status: "loading",
  });
  const pendingSceneResourcesRef = useRef<Set<SceneLoadResource>>(new Set());
  /**
   * The same pending set, in a form render can read (owner, 2026-09-15).
   *
   * The ref decides *when* the overlay lifts and must stay a ref — it is
   * written from loader callbacks that must not wait for a render to see each
   * other's writes. This mirror decides *what the overlay says* while it is
   * up: which pieces of the scene are still coming, the map image above all,
   * which is the one the owner watched arrive with no signal at all. Two
   * copies of one fact is a cost paid deliberately; the ref remains the
   * authority and this is only ever read for words on screen.
   */
  const [pendingSceneResources, setPendingSceneResources] = useState<
    readonly SceneLoadResource[]
  >([]);

  // Reset to "loading" whenever the active scene or the retry generation
  // changes — mirrors the walls/shapes "clear the previous scene's stale
  // data" effects below, but for load-status rather than store contents.
  useEffect(() => {
    if (!sceneId) {
      return;
    }
    pendingSceneResourcesRef.current = new Set<SceneLoadResource>([
      "background",
      "walls",
      "tokens",
      "lights",
      "shapes",
    ]);
    // Deliberately left in the effect: this reset is one atomic step with
    // the `pendingSceneResourcesRef` write above it, and a ref cannot be
    // written during render (react-hooks/refs). Hoisting only the state half
    // would leave a window where the status says "loading" for the new scene
    // while the pending set is still the old scene's — and the pending set is
    // what decides when the overlay lifts, so an in-flight loader resolving
    // into that window would empty it and report a scene ready that had not
    // started loading. The two belong together, in the commit phase, where
    // the loader effects below see the same state they report into.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSceneLoadState({ status: "loading" });
    // Same reset, same reason — the words on screen and the status they
    // describe have to change together.
    setPendingSceneResources([
      "background",
      "walls",
      "tokens",
      "lights",
      "shapes",
    ]);
  }, [sceneId, sceneLoadGeneration]);

  const markSceneResourceLoaded = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      pendingSceneResourcesRef.current.delete(resource);
      setPendingSceneResources((current) =>
        current.filter((pending) => pending !== resource),
      );
      if (pendingSceneResourcesRef.current.size === 0) {
        setSceneLoadState((current) =>
          current.status === "error" ? current : { status: "ready" },
        );
      }
    },
    [],
  );

  const markSceneResourceFailed = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      // Background-image failure takes priority in the reported
      // `failedResource` if multiple fail simultaneously, per
      // contracts/scene-load-state.md — it's the most visually
      // disruptive, so don't let a later walls/lights/shapes/tokens
      // failure downgrade an already-reported background failure.
      setSceneLoadState((current) => {
        if (
          current.status === "error" &&
          current.failedResource === "background"
        ) {
          return current;
        }
        return { status: "error", failedResource: resource };
      });
    },
    [],
  );

  const retrySceneLoad = useCallback(() => {
    setSceneLoadGeneration((generation) => generation + 1);
  }, []);

  return {
    sceneLoadGeneration,
    sceneLoadGenerationRef,
    sceneLoadState,
    pendingSceneResources,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    retrySceneLoad,
  };
}
