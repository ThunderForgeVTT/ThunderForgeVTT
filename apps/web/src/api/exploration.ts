/**
 * A scene's memory: whether it has one, and resetting it.
 *
 * Spec 045 US7. Both are Game-Master-only, enforced server-side — these are
 * the calls the control panel makes, not the authority for who may make them.
 */
import { postGraphQL } from "@/api/graphqlClient";

export async function setSceneExploration(
  sceneId: string,
  enabled: boolean,
): Promise<boolean> {
  const answer = await postGraphQL<{ setSceneExploration: boolean }>(
    `
      mutation SetSceneExploration($sceneId: UUID!, $enabled: Boolean!) {
        setSceneExploration(sceneId: $sceneId, enabled: $enabled)
      }
    `,
    { sceneId, enabled },
  );
  return answer.setSceneExploration;
}

/**
 * Reset what has been explored. `forUser` of `null` reaches everyone.
 *
 * Returns the epoch every client must now be at or above — the number that
 * makes the reset stick for a player who is not here to see it.
 */
export async function resetSceneExploration(
  sceneId: string,
  forUser: string | null,
): Promise<number> {
  const answer = await postGraphQL<{ resetSceneExploration: number }>(
    `
      mutation ResetSceneExploration($sceneId: UUID!, $forUser: UUID) {
        resetSceneExploration(sceneId: $sceneId, forUser: $forUser)
      }
    `,
    { sceneId, forUser },
  );
  return answer.resetSceneExploration;
}
