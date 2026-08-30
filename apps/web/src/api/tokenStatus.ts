import { postGraphQL } from "./graphqlClient";
import type { Disclosed } from "@/engine/sdk/Disclosed";
import type { DisclosureState } from "@/engine/sdk/DisclosureState";
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
  /** What the table sees. Present only for someone who runs the world. */
  configured: string | null;
}

interface WireTokenStatus {
  tokenId: string;
  resources: WireResource[];
}

/** A token's resources in the shape the engine command expects. */
export interface TokenStatus {
  tokenId: string;
  resources: {
    definition: ResourceDefinition;
    disclosed: Disclosed;
    /** What the table sees, when this viewer is entitled to know. */
    configured: DisclosureState | null;
  }[];
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
        configured
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
  // `postGraphQL` unwraps `data` and throws on `errors`, so what comes back is
  // the payload itself. Typing it as the envelope silently yields an empty
  // list on every call — the query succeeds, `response.data` is `undefined`,
  // and the caller sees "this scene has no status" rather than a mistake.
  const payload = await postGraphQL<{ tokenStatus: WireTokenStatus[] }>(
    TOKEN_STATUS_QUERY,
    { sceneId },
  );

  return (payload?.tokenStatus ?? []).map((token) => ({
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
            // The server has already sorted by the system's declared order, so
            // the array position *is* the order. A constant here would
            // collapse Genie's health and wish points into whichever order the
            // map iteration produced.
            order: index,
            allowStacking: false,
          },
          disclosed,
          configured: (resource.configured as DisclosureState | null) ?? null,
        },
      ];
    }),
  }));
}

/**
 * Set what one token discloses about one resource.
 *
 * Refused by the server for anyone who does not run the world, which is where
 * that rule belongs — this function does not check, so there is no second
 * opinion to drift from the first.
 */
export async function setTokenDisclosure(
  tokenId: string,
  resourceId: string,
  state: DisclosureState,
): Promise<void> {
  await postGraphQL(
    `mutation ($input: SetTokenDisclosureInput!) {
      setTokenDisclosure(input: $input) { tokenId }
    }`,
    { input: { tokenId, resourceId, state: state.toUpperCase() } },
  );
}
