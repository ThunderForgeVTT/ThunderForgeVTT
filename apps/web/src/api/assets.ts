import { postGraphQL, postGraphQLMultipart } from "@/api/graphqlClient";

export interface CanvasImageAsset {
  id: string;
  worldId: string;
  sceneId: string | null;
  ownerUserId: string;
  storagePath: string;
  kind: "BACKGROUND" | "PASTED";
  widthPx: number;
  heightPx: number;
  byteSize: number;
  createdAt: string;
}

const ASSET_FIELDS = `
  id
  worldId
  sceneId
  ownerUserId
  storagePath
  kind
  widthPx
  heightPx
  byteSize
  createdAt
`;

/**
 * uploadCanvasImage sends `file` as an `Upload!` scalar, which requires
 * the GraphQL multipart request spec (https://github.com/jaydenseric/graphql-multipart-request-spec)
 * rather than the plain JSON body every other mutation in `api/*.ts`
 * uses (see `api/walls.ts`'s `postGraphQL`) — a JSON body cannot carry
 * binary file data.
 */

/**
 * FR-011, FR-012, FR-013: uploads `file` (any image the browser can
 * produce via clipboard paste) to be transcoded server-side to WebP and
 * stored under RustFS. Throws with a message suitable for direct display
 * (server-side errors — FORBIDDEN, oversized upload — surface as plain
 * GraphQL error messages here).
 */
export async function uploadCanvasImage(
  worldId: string,
  sceneId: string,
  kind: "BACKGROUND" | "PASTED",
  file: Blob,
): Promise<CanvasImageAsset> {
  const mutation = `
    mutation UploadCanvasImage($worldId: UUID!, $sceneId: UUID!, $kind: GraphQLCanvasImageAssetKind!, $file: Upload!) {
      uploadCanvasImage(worldId: $worldId, sceneId: $sceneId, kind: $kind, file: $file) {
        ${ASSET_FIELDS}
      }
    }
  `;
  const data = await postGraphQLMultipart<{
    uploadCanvasImage: CanvasImageAsset;
  }>(mutation, { worldId, sceneId, kind }, file, "file");
  return data.uploadCanvasImage;
}

/** FR-019: assets for a scene, readable by any owner/accepted member of the owning world. */
export async function fetchCanvasImageAssetsForScene(
  sceneId: string,
): Promise<CanvasImageAsset[]> {
  const query = `
    query CanvasImageAssetsForScene($sceneId: UUID!) {
      canvasImageAssetsForScene(sceneId: $sceneId) {
        ${ASSET_FIELDS}
      }
    }
  `;
  // Previously a hand-rolled fetch that ended in `?? []`, which turned a
  // failed or malformed response into a silent "this scene has no assets" —
  // indistinguishable from the real empty case. It now throws like every other
  // call, so a background-image load failure is visible rather than looking
  // like an empty scene.
  const data = await postGraphQL<{
    canvasImageAssetsForScene: CanvasImageAsset[];
  }>(query, {
    sceneId,
  });
  return data.canvasImageAssetsForScene;
}
