import { useEffect, useState } from "react";
import { getAuthoringTools } from "@/api/authoringTools";
import {
  clearAllowedAuthoringTools,
  setAllowedAuthoringTools,
} from "@/engine/bevy";
import { GM_TOOL_IDS } from "@/components/world/GmToolRail/GmToolRail";
import type { ResolvedTools } from "@/lib/authoringTools";

/**
 * Which authoring tools this person may use in this world, and telling the
 * engine about it.
 *
 * Spec 031 FR-044/FR-047. Both halves live here because they must not diverge:
 * a rail filtered from one answer while the engine holds another is how a tool
 * comes to be visible and inert, which is the failure the spec's edge case
 * calls out by name.
 *
 * Chrome is not the gate. The server resolves the permission and the engine
 * refuses anything outside it; what this hook decides is only what is
 * *offered*.
 *
 * The engine is left unrestricted for the moment before the answer arrives,
 * which is safe only because it is not the sole gate either: every authoring
 * input system still checks `IsGameMaster`, so the window cannot hand a player
 * a tool. It exists so a Game Master's rail does not flicker on every load.
 */
export function useAuthoringTools(worldId: string): ResolvedTools {
  const [allowed, setAllowed] = useState<ResolvedTools>(null);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        const tools = await getAuthoringTools(worldId);
        if (cancelled) return;
        setAllowed(tools);

        // A full grant clears the restriction rather than declaring all six.
        // The engine distinguishes "no declaration" from "granted everything",
        // and a Game Master is the former: nothing about their canvas should
        // change because this feature exists (FR-045).
        if (tools.length === GM_TOOL_IDS.length) {
          await clearAllowedAuthoringTools();
        } else {
          await setAllowedAuthoringTools(tools);
        }
      } catch {
        // Left unresolved deliberately. A failed query must not invent a
        // permission in either direction: granting on failure would be a
        // client deciding an authorization question, and revoking on failure
        // would take a Game Master's tools away because a request timed out.
        // The engine keeps whatever it was last told, and the server refuses
        // the write regardless.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [worldId]);

  return allowed;
}
