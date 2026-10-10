/**
 * Spec 048 FR-036a: what a player is told when something their character
 * brought in with a sheet is used before the Game Master adopts it. The
 * server refuses with `CONTENT_NOT_ADOPTED` and this sentence
 * (`crates/thunderforge-server/src/staged_content/guard.rs`); the client
 * keeps its own copy so the words survive however the error is joined.
 */
import { GraphQLRequestError } from "@/api/graphqlClient";

export const NOT_ADOPTED_CODE = "CONTENT_NOT_ADOPTED";

export const NOT_ADOPTED =
  "This came in with the character, and the Game Master has not adopted it yet.";

/** What a refused play action says: the FR-036a sentence where it applies. */
export function playRefusalText(err: unknown, fallback: string): string {
  if (err instanceof GraphQLRequestError && err.hasCode(NOT_ADOPTED_CODE)) {
    return NOT_ADOPTED;
  }
  return err instanceof Error ? err.message : fallback;
}
