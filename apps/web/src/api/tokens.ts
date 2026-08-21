import { withCsrf } from "@/api/auth";
import type { CreateTokenInput, TokenRecord, UpdateTokenInput } from "@/types/token";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const TOKEN_FIELDS = `
  tokenId
  sceneId
  actorId
  x
  y
  rotation
  scale
  metadata
  createdAt
  updatedAt
  ownerUserId
  isPrimary
  photoUrl
  health
  maxHealth
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

type TokensQuery = {
  tokens: TokenRecord[];
};

type CreateTokenMutation = {
  createToken: TokenRecord;
};

type UpdateTokenMutation = {
  updateToken: TokenRecord;
};

type DeleteTokenMutation = {
  deleteToken: boolean;
};

/**
 * Fetch every token on a scene. Used both for the initial load and as the
 * "refetch on notify" step of real-time token sync (see
 * engine/world/sync/tokens.ts): the world_events NOTIFY payload only
 * carries the changed token's id and scene, so on receipt we re-fetch this
 * list rather than trying to reconstruct a token from the notify payload.
 */
export function getTokens(sceneId: string): Promise<TokenRecord[]> {
  return postGraphQL<TokensQuery>(
    `
      query SceneTokens($sceneId: UUID!) {
        tokens(sceneId: $sceneId) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.tokens);
}

export function createToken(input: CreateTokenInput): Promise<TokenRecord> {
  return postGraphQL<CreateTokenMutation>(
    `
      mutation CreateToken($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createToken);
}

export function updateToken(
  tokenId: string,
  input: UpdateTokenInput,
): Promise<TokenRecord> {
  return postGraphQL<UpdateTokenMutation>(
    `
      mutation UpdateToken($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { tokenId, input },
  ).then((data) => data.updateToken);
}

export function deleteToken(tokenId: string): Promise<boolean> {
  return postGraphQL<DeleteTokenMutation>(
    `
      mutation DeleteToken($tokenId: UUID!) {
        deleteToken(tokenId: $tokenId)
      }
    `,
    { tokenId },
  ).then((data) => data.deleteToken);
}

type MoveOwnTokenMutation = {
  moveOwnToken: TokenRecord;
};

type SetOwnPrimaryTokenPhotoMutation = {
  setOwnPrimaryTokenPhoto: TokenRecord;
};

/**
 * Player-facing move: succeeds only when the caller is this token's
 * `ownerUserId` (their primary token, or one the GM granted them). Spec
 * 004 FR-009 — position only, no scene-ownership required.
 */
export function moveOwnToken(
  tokenId: string,
  x: number,
  y: number,
): Promise<TokenRecord> {
  return postGraphQL<MoveOwnTokenMutation>(
    `
      mutation MoveOwnToken($tokenId: UUID!, $x: Float!, $y: Float!) {
        moveOwnToken(tokenId: $tokenId, x: $x, y: $y) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { tokenId, x, y },
  ).then((data) => data.moveOwnToken);
}

/**
 * Player-facing photo edit: succeeds only for the caller's own primary
 * token. Spec 004 FR-009a.
 */
export function setOwnPrimaryTokenPhoto(
  tokenId: string,
  photoUrl: string,
): Promise<TokenRecord> {
  return postGraphQL<SetOwnPrimaryTokenPhotoMutation>(
    `
      mutation SetOwnPrimaryTokenPhoto($tokenId: UUID!, $photoUrl: String!) {
        setOwnPrimaryTokenPhoto(tokenId: $tokenId, photoUrl: $photoUrl) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { tokenId, photoUrl },
  ).then((data) => data.setOwnPrimaryTokenPhoto);
}
