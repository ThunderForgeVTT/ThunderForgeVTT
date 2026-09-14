import { postGraphQL } from "@/api/graphqlClient";
import type {
  CreateTokenInput,
  TokenRecord,
  UpdateTokenInput,
} from "@/types/token";

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
  tokenType
  linked
  name
  nameVisibleToPlayers
`;

/**
 * Show or hide a token's name from players (playtest 2026-09-10 P7). Game
 * Masters only; the server then withholds a hidden name from everyone else,
 * on the token and in the combat tracker.
 */
export function setTokenNameVisibility(
  tokenId: string,
  visible: boolean,
): Promise<TokenRecord> {
  return postGraphQL<{ setTokenNameVisibility: TokenRecord }>(
    `
      mutation SetTokenNameVisibility($tokenId: UUID!, $visible: Boolean!) {
        setTokenNameVisibility(tokenId: $tokenId, visible: $visible) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { tokenId, visible },
  ).then((data) => data.setTokenNameVisibility);
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

/** One point of a route a token walked through, in world coordinates. */
export type PathPoint = { x: number; y: number };

/**
 * Player-facing move: succeeds only when the caller is this token's
 * `ownerUserId` (their primary token, or one the GM granted them). Spec
 * 004 FR-009 — position only, no scene-ownership required.
 *
 * Spec 045 US2: this is judged against the scene's walls, and can be refused
 * with "A wall is in the way". `path` is the route the token took, which the
 * server needs to tell walking around a wall from teleporting through it —
 * the endpoints alone cannot. Omit it for a drag: a drag *is* the straight
 * line the server assumes when no path is given.
 */
export function moveOwnToken(
  tokenId: string,
  x: number,
  y: number,
  path?: PathPoint[],
): Promise<TokenRecord> {
  return postGraphQL<MoveOwnTokenMutation>(
    `
      mutation MoveOwnToken(
        $tokenId: UUID!
        $x: Float!
        $y: Float!
        $path: [GraphQLPathPoint!]
      ) {
        moveOwnToken(tokenId: $tokenId, x: $x, y: $y, path: $path) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    // Undefined rather than an empty list when there is no route: an empty
    // list is a claim ("I walked nowhere"), and absence is the honest thing
    // to send when the client has nothing to say about how it got there.
    { tokenId, x, y, path: path && path.length > 0 ? path : undefined },
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

/**
 * Spec 046 FR-016 (ADR-102): make a token its actor (`linked`) or an
 * unlinked copy holding its own hit points. Game Master only. Linking a copy
 * discards its own hit points; unlinking starts them from the actor's.
 */
export function setTokenLink(
  tokenId: string,
  linked: boolean,
): Promise<TokenRecord> {
  return postGraphQL<{ setTokenLink: TokenRecord }>(
    `
      mutation SetTokenLink($tokenId: UUID!, $linked: Boolean!) {
        setTokenLink(tokenId: $tokenId, linked: $linked) {
          ${TOKEN_FIELDS}
        }
      }
    `,
    { tokenId, linked },
  ).then((data) => data.setTokenLink);
}
