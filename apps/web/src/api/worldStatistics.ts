import { postGraphQL } from "@/api/graphqlClient";

/** The encounter in progress, as the dashboard reads it. */
export interface ActiveEncounterSummary {
  id: string;
  round: number;
  combatants: number;
}

/**
 * A world's figures, counted on the server for the person asking.
 *
 * `npcs` is `null` for anyone who does not run the world. That is not a
 * loading state and not a zero: the server refuses to count NPCs for a
 * player at all, so hidden ones can never move a number a player can read.
 * A page that meets `null` should draw one figure fewer, not a placeholder.
 */
export interface WorldStatistics {
  scenes: number;
  members: number;
  membersWithCharacter: number;
  characters: number;
  npcs: number | null;
  tokens: number;
  activeEncounter: ActiveEncounterSummary | null;
}

export function getWorldStatistics(worldId: string): Promise<WorldStatistics> {
  return postGraphQL<{ worldStatistics: WorldStatistics }>(
    `
      query WorldStatistics($worldId: UUID!) {
        worldStatistics(worldId: $worldId) {
          scenes
          members
          membersWithCharacter
          characters
          npcs
          tokens
          activeEncounter {
            id
            round
            combatants
          }
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldStatistics);
}

/** Just enough of a scene to list it by name and recency. */
export interface SceneHeadline {
  sceneId: string;
  name: string;
  updatedAt: string;
  hidden: boolean;
}

/**
 * The world's scenes, by name and when each last changed — nothing else.
 *
 * The same `scenes(worldId)` field the scene switcher reads, with the same
 * hidden-scene rule, asked for four fields instead of twenty. The switcher
 * needs each scene's summary HTML, background URL and preview URL; a list
 * of the five most recently touched scenes needs none of them, and a
 * five-year campaign's forty summaries are exactly the weight the owner
 * asked this page to stop carrying.
 */
export function getSceneHeadlines(worldId: string): Promise<SceneHeadline[]> {
  return postGraphQL<{ scenes: SceneHeadline[] }>(
    `
      query WorldSceneHeadlines($worldId: UUID!) {
        scenes(worldId: $worldId) {
          sceneId
          name
          updatedAt
          hidden
        }
      }
    `,
    { worldId },
  ).then((data) => data.scenes);
}
