import { postGraphQL } from "./graphqlClient";

/**
 * Attribute scores for the tokens in a scene, as the active system names them.
 *
 * Separate from `getTokens` because attributes live on the *actor* rather than
 * the token — the server resolves them from the world's system manifest and
 * the actor's stored sheet. Merged onto the token payload before it reaches
 * the engine, so a token and its scores arrive together and there is no
 * ordering to get wrong.
 */

export interface TokenAttribute {
  /** The system's own identifier — `might`, `strength`, `prowess`. */
  id: string;
  label: string;
  /** Short form where the system offers one; not every system does. */
  abbreviation: string | null;
  value: number;
}

interface TokenAttributesRow {
  tokenId: string;
  attributes: TokenAttribute[];
}

const QUERY = `
  query ($sceneId: UUID!) {
    tokenAttributes(sceneId: $sceneId) {
      tokenId
      attributes {
        id
        label
        abbreviation
        value
      }
    }
  }
`;

/**
 * Attribute scores keyed by token id, for every token that has any.
 *
 * A token with no actor, or whose sheet is unfilled, is simply absent from
 * the map. That is deliberately not the same as being present with an empty
 * list, which would claim the character has no attributes — a statement about
 * the ruleset rather than about the sheet.
 *
 * Answers `{}` rather than throwing when the query fails: attributes are
 * supplementary, and a world whose tokens cannot be drawn because a character
 * sheet could not be read is a worse outcome than one drawn without scores.
 */
export async function getTokenAttributes(
  sceneId: string,
): Promise<Record<string, Record<string, number>>> {
  try {
    const rows = await postGraphQL<{ tokenAttributes?: TokenAttributesRow[] }>(
      QUERY,
      { sceneId },
    );

    const out: Record<string, Record<string, number>> = {};
    for (const row of rows?.tokenAttributes ?? []) {
      const scores: Record<string, number> = {};
      for (const attribute of row.attributes) {
        scores[attribute.id] = attribute.value;
      }
      out[row.tokenId] = scores;
    }
    return out;
  } catch {
    return {};
  }
}
