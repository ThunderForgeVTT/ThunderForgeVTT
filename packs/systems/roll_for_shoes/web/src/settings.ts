/**
 * Reading and writing this world's optional rules.
 *
 * The pack's whole network surface is these two fields. It goes through
 * `postGraphQL` from `@thunderforge/host` — the same transport every other
 * caller in the app uses — so no shared web file learns that this system
 * exists (`scripts/check-system-registry.mjs`).
 *
 * `world: WorldRecord` does not carry these settings and will not: they live
 * in a table this pack owns (ADR-063, and ADR-108 for why the generic
 * per-world settings surface is still a spec of its own).
 */

import { postGraphQL } from "@thunderforge/host";

import { DEFAULT_SETTINGS, type WorldSettings } from "./game";

const SETTINGS_FIELDS = `
  worldId
  difficultyMode
  tieSucceeds
  statusesEnabled
  skillSlotsEnabled
  startingSkills {
    name
    level
  }
`;

const WORLD_SETTINGS_QUERY = `
  query RollForShoesWorldSettings($worldId: UUID!) {
    rollForShoesWorldSettings(worldId: $worldId) {
      ${SETTINGS_FIELDS}
    }
  }
`;

const UPDATE_WORLD_SETTINGS_MUTATION = `
  mutation UpdateRollForShoesWorldSettings(
    $input: UpdateRollForShoesWorldSettingsInput!
  ) {
    updateRollForShoesWorldSettings(input: $input) {
      ${SETTINGS_FIELDS}
    }
  }
`;

/** The settings as the server states them, which is the shape both fields return. */
interface WorldSettingsPayload {
  worldId: string;
  difficultyMode: string;
  tieSucceeds: boolean;
  statusesEnabled: boolean;
  skillSlotsEnabled: boolean;
  startingSkills: { name: string; level: number }[];
}

/**
 * The wire shape as the rules see it.
 *
 * `difficultyMode` crosses as a string, so an unknown one — a server newer
 * than this build — reads as `free` rather than as a mode nothing can render.
 * The server refuses to *store* an unknown mode; this is the other end of the
 * same caution.
 */
function toSettings(payload: WorldSettingsPayload): WorldSettings {
  const mode = payload.difficultyMode;
  return {
    difficultyMode:
      mode === "rolled" || mode === "target" || mode === "free" ? mode : "free",
    tieSucceeds: payload.tieSucceeds,
    statusesEnabled: payload.statusesEnabled,
    skillSlotsEnabled: payload.skillSlotsEnabled,
    startingSkills: payload.startingSkills.map((skill) => ({
      name: skill.name,
      level: skill.level,
    })),
  };
}

/**
 * This world's settings.
 *
 * Any member may read: the settings describe how the table plays, and every
 * player needs them to roll. A world with no stored row answers with the
 * defaults rather than an error, so this never has to distinguish "not
 * configured" from "configured as the core game" — there is no difference.
 */
export async function fetchWorldSettings(
  worldId: string,
): Promise<WorldSettings> {
  const data = await postGraphQL<{
    rollForShoesWorldSettings: WorldSettingsPayload;
  }>(WORLD_SETTINGS_QUERY, { worldId });
  return toSettings(data.rollForShoesWorldSettings);
}

/**
 * Set this world's settings. Game Master only, and refused while the world is
 * paused.
 *
 * A whole-row write, not a patch: the caller states all five every time. An
 * input of optionals would make "absent" ambiguous between "leave it" and
 * "clear it", and the guarantee that enabling one setting never silently
 * changes another is easiest to hold when every call says all five.
 */
export async function saveWorldSettings(
  worldId: string,
  settings: WorldSettings,
): Promise<WorldSettings> {
  const data = await postGraphQL<{
    updateRollForShoesWorldSettings: WorldSettingsPayload;
  }>(UPDATE_WORLD_SETTINGS_MUTATION, {
    input: {
      worldId,
      difficultyMode: settings.difficultyMode,
      tieSucceeds: settings.tieSucceeds,
      statusesEnabled: settings.statusesEnabled,
      skillSlotsEnabled: settings.skillSlotsEnabled,
      startingSkills: settings.startingSkills.map((skill) => ({
        name: skill.name,
        level: skill.level,
      })),
    },
  });
  return toSettings(data.updateRollForShoesWorldSettings);
}

/**
 * The settings to draw with when the read has not landed or has failed.
 *
 * A sheet that cannot reach the settings shows the core game rather than
 * nothing: every Extra is off by default, so falling back to the defaults is
 * the same as falling back to "this world has configured nothing".
 */
export { DEFAULT_SETTINGS };
