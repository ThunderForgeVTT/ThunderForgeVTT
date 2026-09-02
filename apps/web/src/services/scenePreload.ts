import type { SceneRecord } from "@/types/scene";

/**
 * Getting a scene ready without telling anyone.
 *
 * # What Preload is, and what it deliberately is not
 *
 * It warms *this* Game Master's browser. It does not touch the world's active
 * scene, and no connected player observes anything at all (spec 031 FR-020,
 * SC-004).
 *
 * That restraint is forced rather than chosen. ADR-046 makes the active scene
 * server-authoritative and broadcast, so anything that sets it is immediately
 * visible at the table — which is the opposite of preparing. Launch remains
 * the only action that changes what players see.
 *
 * # Why an image fetch and not the world cache
 *
 * The obvious implementation is `syncWorldCache`, and it is wrong here twice
 * over: it is world-scoped rather than scene-scoped, and it lives in the
 * engine — so calling it from a scene list would mount a ~25MB wasm bundle on
 * a page that draws no canvas, to warm assets for a scene the Game Master may
 * not even open.
 *
 * Fetching the background is the honest 90%: it is far and away the largest
 * thing a scene load waits on, the browser keeps it, and the engine's own
 * cache still does its work normally when the scene is actually opened.
 *
 * # Failure is not worth reporting
 *
 * A preload that fails cost nothing and promised nothing — the scene simply
 * loads at its usual speed later. Surfacing an error for an optimisation the
 * user cannot act on would be noise, so this resolves either way and says
 * whether it warmed anything.
 */

export interface PreloadOutcome {
  /** Whether anything was actually fetched. */
  warmed: boolean;
  /** Why not, when it was not — for a caller that wants to say so. */
  reason?: "no-background" | "failed";
}

export async function preloadScene(
  scene: Pick<SceneRecord, "backgroundUrl">,
): Promise<PreloadOutcome> {
  if (!scene.backgroundUrl) {
    // A scene with no background has nothing heavy to warm. Not a failure:
    // it will open quickly regardless, which is the outcome Preload wants.
    return { warmed: false, reason: "no-background" };
  }

  try {
    // `cache: "force-cache"` because the point is to *populate* the browser
    // cache, and a revalidating fetch would defeat it. The response body is
    // read to completion — a fetch whose body is never consumed may not be
    // stored, which would make this look like it worked and warm nothing.
    const response = await fetch(scene.backgroundUrl, {
      cache: "force-cache",
      credentials: "same-origin",
    });
    if (!response.ok) {
      return { warmed: false, reason: "failed" };
    }
    await response.blob();
    return { warmed: true };
  } catch {
    return { warmed: false, reason: "failed" };
  }
}
