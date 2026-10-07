/**
 * A visitor's picture is taken as `mutations_assets.rs` takes one: members
 * only, within the size cap, transcoded, kept, and listed for its scene.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { DemoState, Row } from "../state";
import { data, freshWorld, refusal } from "../testing/world";
import {
  bytesOf,
  fitToCap,
  imaging,
  MAX_UPLOAD_BYTES,
  type Fit,
} from "../uploads";

const UPLOAD = `mutation ($w: UUID!, $s: UUID!, $k: GraphQLCanvasImageAssetKind!, $f: Upload!) {
  uploadCanvasImage(worldId: $w, sceneId: $s, kind: $k, file: $f) {
    id worldId sceneId ownerUserId storagePath kind widthPx heightPx byteSize
  }
}`;
const LIST = `query ($s: UUID!) { canvasImageAssetsForScene(sceneId: $s) { id kind widthPx } }`;
const SET_BACKGROUND = `mutation ($l: UUID!, $i: GraphQLUpdateSceneLevelInput!) {
  updateSceneLevel(levelId: $l, input: $i) { backgroundAssetId backgroundUrl }
}`;

const browser = imaging.transcode;
let state: DemoState;
let sceneId: string;

/** A picture of `width` by `height`, "transcoded" to the size `fit` gives. */
function pictureOf(width: number, height: number) {
  imaging.transcode = async (_bytes: Blob, fit: Fit) => {
    const [w, h] = fit(width, height);
    return { bytes: new Blob(["webp bytes"]), width: w, height: h };
  };
}

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
  pictureOf(1200, 800);
});

afterEach(() => {
  imaging.transcode = browser;
});

const upload = (file: Blob, worldId = state.world.id) =>
  data<{ uploadCanvasImage: Row }>(UPLOAD, {
    w: worldId,
    s: sceneId,
    k: "BACKGROUND",
    f: file,
  });

describe("uploadCanvasImage", () => {
  it("keeps the picture, and its row says where", async () => {
    const { uploadCanvasImage: asset } = await upload(new Blob(["png"]));
    expect(asset).toMatchObject({
      worldId: state.world.id,
      sceneId,
      kind: "BACKGROUND",
      widthPx: 1200,
      heightPx: 800,
      byteSize: 10,
      storagePath: `${asset.ownerUserId}/${state.world.id}/${sceneId}/${asset.id}.webp`,
    });
    expect(await (await bytesOf(asset.id as string))?.text()).toBe(
      "webp bytes",
    );
  });

  it("shrinks a picture past the texture cap, in proportion", async () => {
    pictureOf(8192, 2048);
    const { uploadCanvasImage: asset } = await upload(new Blob(["png"]));
    expect([asset.widthPx, asset.heightPx]).toEqual([4096, 1024]);
    expect(fitToCap(100, 50)).toEqual([100, 50]);
  });

  it("refuses another world, too large a file, and what is not a picture", async () => {
    expect(
      await refusal(UPLOAD, {
        w: crypto.randomUUID(),
        s: sceneId,
        k: "BACKGROUND",
        f: new Blob(["png"]),
      }),
    ).toBe("user is not a member of this world");
    const huge = { size: MAX_UPLOAD_BYTES + 1 } as Blob;
    Object.setPrototypeOf(huge, Blob.prototype);
    expect(
      await refusal(UPLOAD, {
        w: state.world.id,
        s: sceneId,
        k: "BACKGROUND",
        f: huge,
      }),
    ).toBe(
      `upload exceeds maximum size of ${MAX_UPLOAD_BYTES} bytes (got ${MAX_UPLOAD_BYTES + 1})`,
    );
    imaging.transcode = async () => {
      throw new Error("The source image cannot be decoded.");
    };
    expect(
      await refusal(UPLOAD, {
        w: state.world.id,
        s: sceneId,
        k: "BACKGROUND",
        f: new Blob(["text"]),
      }),
    ).toBe(
      "failed to decode/transcode image: The source image cannot be decoded.",
    );
  });

  it("is listed for its scene beside the seeded map, and becomes a level's background", async () => {
    const { uploadCanvasImage: asset } = await upload(new Blob(["png"]));
    const { canvasImageAssetsForScene } = await data<{
      canvasImageAssetsForScene: Row[];
    }>(LIST, { s: sceneId });
    const seeded = (state.scenes.find((s) => s.sceneId === sceneId) as Row)
      .backgroundAssetId;
    expect(canvasImageAssetsForScene.map((a) => a.id)).toEqual([
      seeded,
      asset.id,
    ]);
    const entry = state.levels.find((l) => l.sceneId === sceneId && l.isEntry)
      ?.levelId as string;
    const { updateSceneLevel } = await data<{ updateSceneLevel: Row }>(
      SET_BACKGROUND,
      {
        l: entry,
        i: { backgroundAssetId: asset.id, width: 1200, height: 800 },
      },
    );
    expect(updateSceneLevel.backgroundUrl).toBe(
      `/api/canvas-assets/${asset.id}.webp`,
    );
  });

  it("goes when the visitor forgets the world", async () => {
    const { uploadCanvasImage: asset } = await upload(new Blob(["png"]));
    await freshWorld();
    expect(await bytesOf(asset.id as string)).toBeNull();
  });
});
