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

/**
 * What one member of a world has been *granted*, for the Game Master looking
 * at the toggles (spec 031 FR-046).
 *
 * Not the same question as `getAuthoringTools`, which answers "what may I
 * use" and folds in a Game Master's implicit everything. A member absent from
 * this list holds nothing, which is the server's default (FR-045) rather than
 * a gap in the response.
 */
export interface MemberAuthoringTools {
  worldMemberId: string;
  userId: string;
  tools: GmToolId[];
}

/** Every grant handed out in one world. GM-only, refused server-side. */
export async function getAuthoringToolGrants(
  worldId: string,
): Promise<MemberAuthoringTools[]> {
  const payload = await postGraphQL<{
    authoringToolGrants: {
      worldMemberId: string;
      userId: string;
      tools: string[];
    }[];
  }>(
    `query ($worldId: UUID!) {
      authoringToolGrants(worldId: $worldId) {
        worldMemberId
        userId
        tools
      }
    }`,
    { worldId },
  );

  return (payload?.authoringToolGrants ?? []).map((entry) => ({
    worldMemberId: entry.worldMemberId,
    userId: entry.userId,
    // Same filter as `getAuthoringTools`, for the same reason: a tool this
    // build does not have cannot be rendered as a toggle, and a toggle that
    // controls a name nothing recognises is worse than its absence.
    tools: entry.tools.filter(isGmToolId),
  }));
}

/**
 * Grant or revoke one tool for one member. Returns that member's grants after
 * the write — the table's answer, not the click's.
 *
 * The refusal lives on the server (`is_dm_of_world`), per Constitution
 * Principle III; hiding this card from a player is chrome, and a player
 * calling this directly is refused there.
 */
export async function setAuthoringToolGrant(input: {
  worldId: string;
  worldMemberId: string;
  tool: GmToolId;
  granted: boolean;
}): Promise<GmToolId[]> {
  const payload = await postGraphQL<{ setAuthoringToolGrant: string[] }>(
    `mutation ($worldId: UUID!, $worldMemberId: UUID!, $tool: String!, $granted: Boolean!) {
      setAuthoringToolGrant(
        worldId: $worldId
        worldMemberId: $worldMemberId
        tool: $tool
        granted: $granted
      )
    }`,
    input,
  );

  return (payload?.setAuthoringToolGrant ?? []).filter(isGmToolId);
}
