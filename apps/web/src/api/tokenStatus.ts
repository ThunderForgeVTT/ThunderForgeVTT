import { postGraphQL } from "./graphqlClient";
import type { Disclosed } from "@/engine/sdk/Disclosed";
import type { ResourceDefinition } from "@/engine/sdk/ResourceDefinition";

/**
 * Reading each token's resources, as the server resolved them for *this*
 * viewer.
 *
 * Spec 029. The server has already coarsened anything this person is not
 * entitled to see, so nothing here filters and nothing here can widen. A
 * chunked resource arrives as a quarter index with no proportion; a percentage
 * arrives with no maximum; a greyed one carries no figure at all. A client that
 * never receives a value cannot leak it, however the UI is written or modified.
 *
 * That is why this module is thin: the interesting decisions happened before
 * the response was sent.
 */

/** One resource as the wire carries it, before conversion to the engine shape. */
interface WireResource {
  definitionId: string;
  label: string;
  kind: string;
  disclosure: string;
  entries:
    | { current: number; max: number | null; label: string | null }[]
    | null;
  proportion: number | null;
  quarter: number | null;
}

interface WireTokenStatus {
  tokenId: string;
  resources: WireResource[];
}

/** A token's resources in the shape the engine command expects. */
export interface TokenStatus {
  tokenId: string;
  resources: { definition: ResourceDefinition; disclosed: Disclosed }[];
}

const TOKEN_STATUS_QUERY = `
  query TokenStatus($sceneId: UUID!) {
    tokenStatus(sceneId: $sceneId) {
      tokenId
      resources {
        definitionId
        label
        kind
        disclosure
        entries { current max label }
        proportion
        quarter
      }
    }
  }
`;

/**
 * Rebuild the tagged union the engine expects from GraphQL's flat optionals.
 *
 * GraphQL cannot express a discriminated union over scalars, so the wire
 * carries three mutually exclusive nullable fields and the discriminant beside
 * them. Reassembling here means the rest of the client — and the engine —
 * work with a shape where an over-disclosing value is unrepresentable.
 *
 * A row whose discriminant does not match its payload is dropped rather than
 * guessed at. It would mean the server and this build disagree about the
 * contract, and inventing a reading from a disagreement is how a coarse value
 * gets shown as an exact one.
 */
function toDisclosed(resource: WireResource): Disclosed | null {
  switch (resource.disclosure) {
    case "visible":
      return resource.entries
        ? { disclosure: "visible", entries: resource.entries }
        : null;
    case "greyed":
      return { disclosure: "greyed" };
    case "percentage":
      return resource.proportion !== null
        ? { disclosure: "percentage", proportion: resource.proportion }
        : null;
    case "chunked":
      return resource.quarter !== null
        ? { disclosure: "chunked", quarter: resource.quarter }
        : null;
    default:
      return null;
  }
}

export async function getTokenStatus(sceneId: string): Promise<TokenStatus[]> {
  const response = await postGraphQL<{
    data?: { tokenStatus?: WireTokenStatus[] };
    errors?: { message: string }[];
  }>(TOKEN_STATUS_QUERY, { sceneId });

  if (response.errors?.length) {
    throw new Error(response.errors.map((e) => e.message).join("; "));
  }

  return (response.data?.tokenStatus ?? []).map((token) => ({
    tokenId: token.tokenId,
    resources: token.resources.flatMap((resource, index) => {
      const disclosed = toDisclosed(resource);
      if (!disclosed) return [];
      return [
        {
          definition: {
            id: resource.definitionId,
            label: resource.label,
            kind: resource.kind as ResourceDefinition["kind"],
            // The server has already sorted by the system's declared order,
            // so the array position *is* the order. Preserving it here matters:
            // the engine sorts by this field, and a constant would collapse
            // Genie's health and wish points into whichever order the map
            // iteration happened to produce.
            order: index,
            allowStacking: false,
          },
          disclosed,
        },
      ];
    }),
  }));
}
