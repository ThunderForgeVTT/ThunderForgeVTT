import { postGraphQL } from "@/api/graphqlClient";
import type { MapCredit } from "@/types/scene";

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

/** Just enough of a scene to show it as a card: name, recency and picture. */
export interface SceneHeadline {
  sceneId: string;
  name: string;
  updatedAt: string;
  hidden: boolean;
  /** Spec 100 US3: the card's picture, `previewUrl` first. */
  previewUrl: string | null;
  backgroundUrl: string | null;
  /** Spec 088 FR-030: a base map is credited wherever it is shown. */
  backgroundCredit?: MapCredit | null;
}

/**
 * The world's scenes, by name, when each last changed and its picture —
 * nothing else.
 *
 * The same `scenes(worldId)` field the scene switcher reads, with the same
 * hidden-scene rule, asked for a handful of fields instead of twenty. The
 * switcher needs each scene's summary HTML; the overview's scene cards
 * (spec 100 US3) need only a name and a picture, and a five-year
 * campaign's forty summaries are exactly the weight the owner asked this
 * page to stop carrying. The picture URLs are strings; the images
 * themselves load only for the few cards drawn.
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
          previewUrl
          backgroundUrl
          backgroundCredit { author licence licenceUrl source catalog shareAlike }
        }
      }
    `,
    { worldId },
  ).then((data) => data.scenes);
}

/**
 * The name of the scene Play opens on, or `null` when no scene is active or
 * the caller cannot see it. One request, for the Play button on every world
 * page (spec 100 US3.4).
 */
export function getResumeSceneName(worldId: string): Promise<string | null> {
  return postGraphQL<{
    world: { activeSceneId: string | null } | null;
    scenes: { sceneId: string; name: string }[];
  }>(
    `
      query WorldResumeScene($worldId: UUID!) {
        world(id: $worldId) {
          activeSceneId
        }
        scenes(worldId: $worldId) {
          sceneId
          name
        }
      }
    `,
    { worldId },
  ).then(({ world, scenes }) => {
    const active = world?.activeSceneId;
    return scenes.find((scene) => scene.sceneId === active)?.name ?? null;
  });
}
