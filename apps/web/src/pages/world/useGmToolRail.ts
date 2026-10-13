import { useEffect, useState } from "react";
import { onAuthoringToolRevoked, setAuthoringMode } from "@/engine/bevy";
import { useAuthoringTools } from "@/hooks/useAuthoringTools";
import { railTools, reconcileOpenTool } from "@/lib/authoringTools";
import {
  GM_TOOL_IDS,
  type GmToolId,
} from "@/components/world/GmToolRail/GmToolRail";

interface UseGmToolRailParams {
  id: string;
  isSceneOwner: boolean;
}

/**
 * The authoring tool rail's state: which tool is open, which tools this
 * viewer may use, and the one the engine took away mid-use.
 *
 * Moved out of `WorldPage` unchanged, including the effect that tells the
 * engine which tool is armed.
 */
export function useGmToolRail({ id, isSceneOwner }: UseGmToolRailParams) {
  /**
   * Which of the GM's authoring tools is open in the left-hand rail.
   *
   * Held here rather than in `GmToolRail` because that component is rendered
   * only once the scene and the viewer's role have resolved, and remounts as
   * they settle — which was silently closing whatever the Game Master had just
   * opened. "Which tool am I working with" outlives the rail, so it lives with
   * the page.
   */
  // Select, not `null`: plain selection is what a GM is doing most of the
  // time, and opening on an empty rail made "no tool armed" an unlabelled
  // state you could only reach by closing something.
  const [openGmToolId, setOpenGmToolId] = useState<GmToolId | null>("select");

  /**
   * Which tools this person may use here (spec 031 FR-044).
   *
   * The rail used to be a role check: Game Master, therefore all six tools.
   * The role is still what decides whether the rail renders at all, but *which*
   * tools it holds is now a permission the server resolves — and the same
   * answer is pushed into the engine by this hook, so the two cannot disagree.
   */
  const allowedTools = useAuthoringTools(id);

  /**
   * A tool the engine took away while it was in use.
   *
   * The engine has already left the mode and discarded the gesture; without
   * this the person would be left clicking at a map that had quietly stopped
   * listening, with the rail still showing their tool as armed — the exact
   * failure spec 031's edge case names.
   */
  const [revokedTool, setRevokedTool] = useState<GmToolId | null>(null);

  useEffect(
    () =>
      onAuthoringToolRevoked((event) => {
        // Narrowed through the rail's own list rather than cast: an id this
        // build does not have would otherwise be rendered into the notice.
        const tool =
          GM_TOOL_IDS.find((toolId) => toolId === event.tool) ?? null;
        setRevokedTool(tool);
      }),
    [],
  );

  /**
   * Which tool the rail actually has open, once permission is accounted for.
   *
   * Derived rather than synchronised into `openGmToolId` by an effect: the
   * chosen tool and the permitted set are two independent inputs, and copying
   * one into the other leaves a render in which the flyout is still offering
   * controls for a tool the engine has already refused.
   */
  const effectiveGmToolId = reconcileOpenTool(openGmToolId, allowedTools);

  /**
   * Whether this viewer has a rail at all. A Game Master always does; a
   * player does once the server says which tools they hold, and by default
   * that is Select and Shapes (spec 082).
   */
  const hasToolRail =
    railTools(
      GM_TOOL_IDS.map((toolId) => ({ id: toolId })),
      allowedTools,
      isSceneOwner,
    ).length > 0;

  /**
   * Tell the engine which tool is armed, whenever that changes.
   *
   * This state decides which flyout renders — that is chrome's business. It is
   * **not** the authority for what a click on the canvas does; every canvas
   * input system lives in the engine, and until this effect existed none of
   * them knew which tool had been chosen. `handle_wall_input` is gated by
   * `IsGameMaster` alone, as is token dragging, so for a Game Master a single
   * click was offered to every authoring system at once.
   *
   * Closing the rail entirely (`null`) returns the engine to Select. It used
   * to leave the engine armed with whatever it had, so a Game Master who
   * picked Lights and closed the rail placed a light with every click on the
   * map, with nothing on screen saying a tool was armed — the "yellow dot"
   * of two playtests (2026-09-01 #16, 2026-09-10 P2). Select is not an
   * invented "no tool" mode: it is the rail's own default, and in it a click
   * on empty map only deselects.
   */
  useEffect(() => {
    // Only someone with a rail arms a tool: a Game Master, or a player once
    // the server has said which tools they hold (spec 082). The engine
    // refuses a mode outside that answer regardless, and the server refuses
    // the write — this is the third layer, not the only one.
    if (!hasToolRail) return;
    void setAuthoringMode(effectiveGmToolId ?? "select");
  }, [hasToolRail, effectiveGmToolId]);

  return {
    setOpenGmToolId,
    allowedTools,
    revokedTool,
    setRevokedTool,
    effectiveGmToolId,
    hasToolRail,
  };
}
