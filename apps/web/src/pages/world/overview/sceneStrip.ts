import { useEffect, useState } from "react";
import { getSceneHeadlines, type SceneHeadline } from "@/api/worldStatistics";

/** How many scenes the overview draws as cards; the rest are a link away. */
export const SCENE_CARD_COUNT = 5;

/**
 * The scenes the overview shows, in order: the table's scene first, then the
 * most recently changed.
 *
 * Capped, not the whole list. The owner, on the dashboard this replaced:
 * "imagine 40 scenes we've been running for five years. Showing all the
 * scenes would be chaotic." Sorted here rather than trusting the order the
 * scenes query returns, which is not a contract.
 */
export function scenesForStrip(
  scenes: readonly SceneHeadline[],
  activeSceneId: string | null,
): SceneHeadline[] {
  const recent = [...scenes].sort(
    (a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt),
  );
  const active = recent.findIndex((scene) => scene.sceneId === activeSceneId);
  if (active > 0) recent.unshift(...recent.splice(active, 1));
  return recent.slice(0, SCENE_CARD_COUNT);
}

/** The card's picture: the reduced preview when there is one. */
export const pictureOf = (scene: SceneHeadline): string | null =>
  scene.previewUrl ?? scene.backgroundUrl;

/**
 * The world's scene headlines: `null` while loading, `"failed"` when they
 * could not be read. Read once by the overview, for the strip and for the
 * scene Play resumes in.
 */
export function useSceneHeadlines(
  worldId: string,
): SceneHeadline[] | "failed" | null {
  const [loaded, setLoaded] = useState<{
    worldId: string;
    scenes: SceneHeadline[] | "failed";
  } | null>(null);
  useEffect(() => {
    let active = true;
    getSceneHeadlines(worldId)
      .then((scenes) => {
        if (active) setLoaded({ worldId, scenes });
      })
      .catch(() => {
        if (active) setLoaded({ worldId, scenes: "failed" });
      });
    return () => {
      active = false;
    };
  }, [worldId]);
  return loaded?.worldId === worldId ? loaded.scenes : null;
}

/** Play names the scene it opens on, once the scenes say which that is. */
export function resumeLabel(
  scenes: readonly SceneHeadline[] | "failed" | null,
  activeSceneId: string | null,
): string {
  if (!activeSceneId || !Array.isArray(scenes)) return "Play";
  const active = scenes.find((scene) => scene.sceneId === activeSceneId);
  return active ? `Resume in ${active.name}` : "Play";
}
