import { withCsrf } from "@/api/auth";

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

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

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
async function postGraphQLMultipart<TData>(
  query: string,
  variables: Record<string, unknown>,
  file: Blob,
  filePathInVariables: string,
): Promise<TData> {
  const operations = JSON.stringify({
    query,
    variables: { ...variables, [filePathInVariables]: null },
  });
  const map = JSON.stringify({ "0": [`variables.${filePathInVariables}`] });

  const formData = new FormData();
  formData.append("operations", operations);
  formData.append("map", map);
  formData.append("0", file);

  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    // Deliberately no Content-Type header: the browser sets the
    // multipart boundary itself when the body is a FormData instance.
    headers: withCsrf(),
    body: formData,
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok || payload.errors?.length) {
    throw new Error(payload.errors?.[0]?.message || `Upload failed with status ${response.status}`);
  }
  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }
  return payload.data;
}

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
  const data = await postGraphQLMultipart<{ uploadCanvasImage: CanvasImageAsset }>(
    mutation,
    { worldId, sceneId, kind },
    file,
    "file",
  );
  return data.uploadCanvasImage;
}

/** FR-019: assets for a scene, readable by any owner/accepted member of the owning world. */
export async function fetchCanvasImageAssetsForScene(sceneId: string): Promise<CanvasImageAsset[]> {
  const query = `
    query CanvasImageAssetsForScene($sceneId: UUID!) {
      canvasImageAssetsForScene(sceneId: $sceneId) {
        ${ASSET_FIELDS}
      }
    }
  `;
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({ query, variables: { sceneId } }),
  });
  const payload = (await response.json()) as GraphQLResponse<{
    canvasImageAssetsForScene: CanvasImageAsset[];
  }>;
  if (!response.ok || payload.errors?.length) {
    throw new Error(payload.errors?.[0]?.message || "Failed to load scene assets");
  }
  return payload.data?.canvasImageAssetsForScene ?? [];
}
