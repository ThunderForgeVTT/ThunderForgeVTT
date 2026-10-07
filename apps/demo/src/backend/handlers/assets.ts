/**
 * A visitor's own picture on the board, as a server takes one.
 *
 * Mirrors `graphql/mutations_assets.rs`: `upload_canvas_image_impl` checks
 * membership before anything else, transcodes to WebP within
 * `MAX_UPLOAD_BYTES` and `MAX_CANVAS_TEXTURE_DIMENSION`
 * (`storage/transcode.rs`), keeps the bytes and only then the row;
 * `canvas_image_assets_for_scene` answers a member with the scene's rows.
 * The bytes live in the browser (`../uploads.ts`) and are served at the same
 * `/api/canvas-assets/{id}.webp` the engine reads every background from.
 */
import { DEMO_USER } from "../../seed/world";
import { viewerUser } from "../actors";
import { now } from "../events";
import { demoState, markChanged, type DemoState, type Row } from "../state";
import {
  fitToCap,
  imaging,
  keepBytes,
  MAX_UPLOAD_BYTES,
  type Fit,
} from "../uploads";
import { refuse, sceneOf, type Handler } from "./common";

/** `object_key`: `{owner}/{world}/{scene}/{asset}.webp`. */
function objectKey(
  owner: string,
  world: string,
  scene: string,
  asset: string,
): string {
  return `${owner}/${world}/${scene}/${asset}.webp`;
}

/**
 * Transcodes and keeps a picture, and records its row. Shared with the map
 * import, which stores its background the same way.
 */
export async function storeImage(
  state: DemoState,
  sceneId: string,
  kind: string,
  file: Blob,
  fit: Fit = fitToCap,
): Promise<Row> {
  if (file.size > MAX_UPLOAD_BYTES) {
    refuse(
      `upload exceeds maximum size of ${MAX_UPLOAD_BYTES} bytes (got ${file.size})`,
    );
  }
  let transcoded;
  try {
    transcoded = await imaging.transcode(file, fit);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    refuse(
      message.startsWith("failed to decode/transcode image")
        ? message
        : `failed to decode/transcode image: ${message}`,
    );
  }
  const id = crypto.randomUUID();
  const owner = viewerUser(state).id as string;
  await keepBytes(id, transcoded.bytes);
  const row: Row = {
    id,
    worldId: state.world.id,
    sceneId,
    ownerUserId: owner,
    storagePath: objectKey(owner, state.world.id, sceneId, id),
    kind,
    widthPx: transcoded.width,
    heightPx: transcoded.height,
    byteSize: transcoded.bytes.size,
    createdAt: now(),
  };
  state.assets[id] = { byteSize: transcoded.bytes.size, row };
  markChanged();
  return row;
}

/** A seeded map's row, which the seed keeps as a file name only. */
function seededRow(state: DemoState, scene: Row, id: string): Row {
  const level = state.levels.find((l) => l.backgroundAssetId === id);
  const width = (level?.width ?? scene.width) as number;
  const height = (level?.height ?? scene.height) as number;
  return {
    id,
    worldId: state.world.id,
    sceneId: scene.sceneId,
    ownerUserId: DEMO_USER.id,
    storagePath: objectKey(
      DEMO_USER.id,
      state.world.id,
      scene.sceneId as string,
      id,
    ),
    kind: "BACKGROUND",
    widthPx: Math.round(width),
    heightPx: Math.round(height),
    byteSize: state.assets[id]?.byteSize ?? 0,
    createdAt: state.world.createdAt,
  };
}

export const assetQueries: Record<string, Handler> = {
  canvasImageAssetsForScene: ({ sceneId }) => {
    const state = demoState();
    const scene = state.scenes.find((s) => s.sceneId === sceneId);
    if (!scene) refuse("database error: scene not found");
    const seeded = new Set(
      [scene, ...state.levels.filter((l) => l.sceneId === sceneId)]
        .map((holder) => holder.backgroundAssetId as string | null)
        .filter((id): id is string => !!id && !!state.assets[id]?.file),
    );
    const uploaded = Object.values(state.assets)
      .map((held) => held.row)
      .filter((row): row is Row => !!row && row.sceneId === sceneId);
    return [
      ...[...seeded].map((id) => seededRow(state, scene, id)),
      ...uploaded,
    ];
  },
};

export const assetMutations: Record<string, Handler> = {
  uploadCanvasImage: async ({ worldId, sceneId, kind, file }) => {
    const state = demoState();
    // Every visitor is a member of the one world; any other is not theirs.
    if (worldId !== state.world.id) {
      refuse("user is not a member of this world");
    }
    sceneOf(state, sceneId);
    if (!(file instanceof Blob)) refuse("failed to read upload: no file");
    return storeImage(state, sceneId, kind, file);
  },
};
