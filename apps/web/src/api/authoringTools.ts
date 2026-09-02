import { postGraphQL } from "./graphqlClient";
import {
  GM_TOOL_IDS,
  type GmToolId,
} from "@/components/world/GmToolRail/GmToolRail";

/**
 * Whether a string the server sent names a tool this build has.
 *
 * Written against `GM_TOOL_IDS` rather than restating the names — a second
 * copy of that list is what drifted last time. It lives here rather than
 * beside the list because the rail is a component module, and a non-component
 * export there costs fast refresh.
 */
function isGmToolId(value: string): value is GmToolId {
  return (GM_TOOL_IDS as readonly string[]).includes(value);
}

/**
 * Which authoring tools the signed-in person may use in one world.
 *
 * Spec 031 FR-044/FR-047. The rail used to decide this from the caller's role,
 * in React, which made chrome the authority on a permission. It is not: the
 * server resolves it (`auth/authoring_tools.rs`) and the engine refuses
 * anything outside the answer, so what this function returns only decides what
 * is *offered*. A client that lied to itself here would still be refused on
 * the canvas.
 */
export async function getAuthoringTools(worldId: string): Promise<GmToolId[]> {
  const payload = await postGraphQL<{ authoringTools: string[] }>(
    `query ($worldId: UUID!) {
      authoringTools(worldId: $worldId)
    }`,
    { worldId },
  );

  // Filtered through the rail's own list rather than cast. A tool this build
  // does not have cannot be rendered or armed, and passing an unknown id on to
  // the engine would spend a `set_authoring_mode` call to be told the same
  // thing. Dropping it here keeps the failure at the boundary that noticed.
  return (payload?.authoringTools ?? []).filter(isGmToolId);
}
