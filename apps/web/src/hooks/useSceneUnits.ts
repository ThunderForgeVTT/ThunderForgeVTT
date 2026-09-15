import { useEffect, useState } from "react";
import { getSceneUnits } from "@/api/lights";
import type { SceneUnits } from "@/types/light";

/**
 * What a scene's distances are measured in, or `undefined` until the server
 * says (and when it cannot: a panel then shows the default five-foot square
 * rather than nothing).
 *
 * Spec 045 FR-061: a light's reach is set in the system's units. The units
 * are the system's and the square's size is the scene's, and only the server
 * holds both.
 */
export function useSceneUnits(
  sceneId: string | null | undefined,
  enabled = true,
): SceneUnits | undefined {
  const [answer, setAnswer] = useState<{
    sceneId: string;
    units: SceneUnits;
  } | null>(null);

  useEffect(() => {
    if (!sceneId || !enabled) return;
    let active = true;
    getSceneUnits(sceneId)
      .then((units) => {
        if (active) setAnswer({ sceneId, units });
      })
      .catch(() => {
        // Left unknown; the caller's default stands.
      });
    return () => {
      active = false;
    };
  }, [sceneId, enabled]);

  return answer && answer.sceneId === sceneId ? answer.units : undefined;
}
