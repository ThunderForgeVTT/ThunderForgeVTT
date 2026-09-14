import { useEffect, useState } from "react";
import { getWorldPlayState, type WorldPlayState } from "@/api/playPause";

/**
 * A world's play state for the pages around play (spec 051 US5): the world
 * hub, its card in the world list, and its settings.
 *
 * Read once on arrival and again when the window regains focus, so a table
 * that comes back to a tab it left open sees a pause (or its lift) without
 * reloading. No interval: only the notice polls, because only the notice is
 * waiting on the answer.
 *
 * `null` until known, and stays `null` if the read is refused. A person who
 * is not a member (an operator browsing every world) is refused by
 * `require_world_member`, and a failed read is not news about a pause, so
 * nothing is shown rather than something wrong.
 */
export function useWorldPlayState(
  worldId: string | null | undefined,
): WorldPlayState | null {
  const [state, setState] = useState<{
    worldId: string;
    value: WorldPlayState;
  } | null>(null);

  useEffect(() => {
    if (!worldId) return;
    let active = true;
    const ask = () => {
      getWorldPlayState(worldId)
        .then((value) => {
          if (active) setState({ worldId, value });
        })
        .catch(() => undefined);
    };
    ask();
    window.addEventListener("focus", ask);
    return () => {
      active = false;
      window.removeEventListener("focus", ask);
    };
  }, [worldId]);

  return state && state.worldId === worldId ? state.value : null;
}
