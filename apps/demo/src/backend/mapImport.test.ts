/**
 * A `.dd2vtt` map lands on a scene as `map_import/` lands one: the file's own
 * grid, the art centred with y up, doors from portals, its light, and one
 * code-13 event for the batch.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ambientLevel, importUvtt, storedCellSize } from "./mapImport";
import type { DemoState, Row } from "./state";
import { freshWorld, heard, releaseEvents } from "./testing/world";
import { imaging, type Fit } from "./uploads";

const PNG = btoa(
  String.fromCharCode(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0),
);

const browser = imaging.transcode;
let state: DemoState;
let sceneId: string;

function pictureOf(width: number, height: number) {
  imaging.transcode = async (_bytes: Blob, fit: Fit) => {
    const [w, h] = fit(width, height);
    return { bytes: new Blob(["webp"]), width: w, height: h };
  };
}

/** A 10×6-cell map at 128px, as `geometry.rs`'s own tests draw it. */
function uvtt(overrides: Row = {}): Row {
  return {
    format: 0.3,
    resolution: {
      map_origin: { x: 0, y: 0 },
      map_size: { x: 10, y: 6 },
      pixels_per_grid: 128,
    },
    line_of_sight: [
      [
        { x: 0, y: 0 },
        { x: 10, y: 0 },
        { x: 10, y: 6 },
      ],
      [{ x: 1, y: 1 }],
    ],
    objects_line_of_sight: [],
    portals: [
      {
        bounds: [
          { x: 5, y: 3 },
          { x: 6, y: 3 },
        ],
        closed: true,
        freestanding: true,
      },
    ],
    environment: { ambient_light: "ff3a3a3a" },
    lights: [
      {
        position: { x: 5, y: 3 },
        range: 2.5,
        intensity: 0.8,
        color: "ffffaa00",
        shadows: true,
      },
    ],
    image: PNG,
    ...overrides,
  };
}

function form(body: Row | string): FormData {
  const f = new FormData();
  const text = typeof body === "string" ? body : JSON.stringify(body);
  f.append("file", new Blob([text]), "map.dd2vtt");
  return f;
}

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
  pictureOf(1280, 768);
  releaseEvents();
});

afterEach(() => {
  imaging.transcode = browser;
});

describe("importUvtt", () => {
  it("lays the map's walls, door and light on its art, centred and y up", async () => {
    const events = heard();
    const before = { walls: state.walls.length, lights: state.lights.length };
    const answer = await importUvtt(sceneId, form(uvtt()));
    expect(answer.status).toBe(200);
    expect(answer.body).toMatchObject({
      wallsCreated: 2,
      doorsCreated: 1,
      lightsCreated: 1,
      backgroundImageSet: true,
      skippedDegeneratePolygons: 1,
    });
    expect(answer.body.warnings).toEqual([
      "1 freestanding portal present in the source file; freestanding portals are not attached to wall geometry and may not appear as expected",
    ]);
    const walls = state.walls.slice(before.walls);
    expect(walls.map((w) => [w.x1, w.y1, w.x2, w.y2, w.doorState])).toEqual([
      [-640, 384, 640, 384, "NONE"],
      [640, 384, 640, -384, "NONE"],
      [0, 0, 128, 0, "CLOSED"],
    ]);
    expect(state.lights.slice(before.lights)[0]).toMatchObject({
      x: 0,
      y: 0,
      radius: 320,
      brightRadius: 160,
      intensity: 0.8,
      castsShadows: true,
    });
    const scene = state.scenes.find((s) => s.sceneId === sceneId) as Row;
    expect(scene).toMatchObject({
      gridSize: 128,
      width: 1280,
      height: 768,
      ambientLight: "dark",
      metadata: {
        mapImport: {
          sourceMapCellsX: 10,
          sourceMapCellsY: 6,
          sourcePixelsPerGrid: 128,
        },
      },
    });
    const entry = state.levels.find((l) => l.sceneId === sceneId && l.isEntry);
    expect(entry?.backgroundAssetId).toBe(scene.backgroundAssetId);
    releaseEvents();
    expect(events.map((e) => e.eventCode)).toEqual([13]);
  });

  it("keeps whole cells when the art is shrunk to the texture cap", async () => {
    pictureOf(6144, 3456);
    const answer = await importUvtt(
      sceneId,
      form(
        uvtt({
          resolution: { map_size: { x: 48, y: 27 }, pixels_per_grid: 128 },
        }),
      ),
    );
    expect(answer.status).toBe(200);
    const scene = state.scenes.find((s) => s.sceneId === sceneId) as Row;
    expect([scene.gridSize, scene.width, scene.height]).toEqual([
      85, 4080, 2295,
    ]);
    expect(storedCellSize(1280, 768, 128)).toBeNull();
  });

  it("refuses as the server does", async () => {
    expect(await importUvtt(sceneId, new FormData())).toEqual({
      status: 400,
      body: { error: "no file field found in multipart upload" },
    });
    expect(
      (await importUvtt(sceneId, form(uvtt({ format: 0.2 })))).body,
    ).toEqual({
      error: "unsupported UVTT format version 0.2; only 0.3 is supported",
    });
    expect((await importUvtt(sceneId, form("{"))).status).toBe(400);
    expect(
      (await importUvtt(sceneId, form(uvtt({ image: btoa("GIF89a") })))).body,
    ).toEqual({ error: "decoded image does not look like a PNG file" });
    state.viewer = "player";
    expect(await importUvtt(sceneId, form(uvtt()))).toEqual({
      status: 403,
      body: { error: "scene not found or not owned by caller" },
    });
  });
});

describe("ambientLevel", () => {
  it("reads the map's light as ambient.rs does", () => {
    expect(ambientLevel(undefined)).toBe("bright");
    expect(ambientLevel("fffff7e4")).toBe("bright");
    expect(ambientLevel("ff8a7a60")).toBe("dim");
    expect(ambientLevel("#1a2233")).toBe("dark");
    expect(ambientLevel("night")).toBe("bright");
  });
});
