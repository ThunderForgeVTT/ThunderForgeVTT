import { useEffect } from "react";
import { getWorldPlayState, type WorldPlayState } from "@/api/playPause";
import { useWorldChromeStore } from "./world-chrome-context";
import { isRefusal } from "./world-chrome-store";

/**
 * A world's play state for the pages around play (spec 051 US5): the world
 * hub, its card in the world list, and its settings.
 *
 * Read on arrival and again when the window regains focus, so a table that
 * comes back to a tab it left open sees a pause (or its lift) without
 * reloading. No interval: only the notice polls, because only the notice is
 * waiting on the answer.
 *
 * Spec 101 US4: the answer is kept in the world chrome store, so the next
 * page shows the banner on its first frame and only refreshes it.
 *
 * `null` until known, and `null` if the read is refused. A person who is not
 * a member (an operator browsing every world) is refused by
 * `require_world_member`. Any other failure is not news about a pause, so
 * what was known stays.
 */
export function useWorldPlayState(
  worldId: string | null | undefined,
): WorldPlayState | null {
  const { chrome, refresh } = useWorldChromeStore();

  useEffect(() => {
    if (!worldId) return;
    const ask = () => {
      refresh(worldId, "playState", readPlayState).catch(() => undefined);
    };
    ask();
    window.addEventListener("focus", ask);
    return () => {
      window.removeEventListener("focus", ask);
    };
  }, [worldId, refresh]);

  return (worldId && chrome[worldId]?.playState) || null;
}

function readPlayState(worldId: string): Promise<WorldPlayState | null> {
  return getWorldPlayState(worldId).catch((err: unknown) => {
    if (isRefusal(err)) return null;
    throw err;
  });
}
