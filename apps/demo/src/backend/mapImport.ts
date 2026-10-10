/**
 * `POST /api/scenes/{scene_id}/import/uvtt`: a Universal VTT map (`.dd2vtt`)
 * onto a scene, as the server imports one.
 *
 * Mirrors `crates/thunderforge-server/src/map_import/`:
 * - `parse.rs`: format `0.3` only; a line-of-sight polygon of fewer than two
 *   points is skipped and counted.
 * - `mod.rs` (`import_uvtt_impl`): parse first, then the Game Master check,
 *   then the background, then the walls, doors and lights, the scene's grid,
 *   size, light and `metadata.mapImport`, and one `EVENT_CODE_MAP_IMPORTED`
 *   (13) for the batch; the refusals and their statuses (`error_response`).
 * - `image.rs` and `storage/transcode.rs` (`transcode_map_background`): the
 *   background and its cell size are decided together, so a map wider than
 *   the texture cap keeps whole cells.
 * - `geometry.rs`: grid units from the image's top-left, y down, become the
 *   scene's coordinates, centred, y up.
 * - `ambient.rs` and `warnings.rs`, word for word.
 * - `perimeter.rs` (spec 088 FR-090, FR-091): walls along the map's edges
 *   where the file leaves them open, marked `metadata.perimeter`, replacing
 *   the marked walls an earlier import left on the old edges.
 *
 * Everything here but the image work is arithmetic on the parsed file, and
 * could be the server's own code compiled for the page, as the dice are,
 * once `parse`, `geometry`, `ambient` and `warnings` move out of the server
 * crate into one that builds for wasm. The image work would stay the
 * browser's: the server's `image` crate is the larger half of the server.
 */
import { DEMO_USER, type Viewer } from "../seed/world";
import { now, record } from "./events";
import { storeImage } from "./handlers/assets";
import { levelsOf } from "./handlers/common";
import { backgroundUrlOf } from "./handlers/levels";
import { mirrorToEntry } from "./handlers/scenes";
import { demoState, markChanged, type Row } from "./state";
import { MAX_CANVAS_TEXTURE_DIMENSION, MAX_UPLOAD_BYTES } from "./uploads";

/** `world_events.rs`: `EVENT_CODE_MAP_IMPORTED`. */
const EVENT_MAP_IMPORTED = 13;
const SUPPORTED_FORMAT = 0.3;

export interface ImportAnswer {
  status: number;
  body: Row;
}

class ImportError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

interface Point {
  x: number;
  y: number;
}

interface UvttFile {
  format: number;
  resolution: { map_size: Point; pixels_per_grid: number };
  line_of_sight: Point[][];
  objects_line_of_sight: Point[][];
  portals: { bounds: Point[]; closed: boolean; freestanding: boolean }[];
  environment: { ambient_light?: string | null };
  lights: {
    position: Point;
    range: number;
    intensity: number;
    color: string;
    shadows: boolean;
  }[];
  image: string;
}

function invalid(message: string): never {
  throw new ImportError(400, `invalid UVTT JSON: ${message}`);
}

function required<T>(value: T | undefined | null, field: string): T {
  if (value === undefined || value === null)
    invalid(`missing field \`${field}\``);
  return value;
}

/** `parse_uvtt`. */
function parseUvtt(raw: string): { file: UvttFile; skipped: number } {
  let json: Row;
  try {
    json = JSON.parse(raw) as Row;
  } catch (error) {
    invalid(error instanceof Error ? error.message : String(error));
  }
  if (!json || typeof json !== "object") invalid("expected an object");
  const format = required(json.format as number, "format");
  const resolution = required(json.resolution as Row, "resolution");
  required(resolution.map_size, "map_size");
  required(resolution.pixels_per_grid, "pixels_per_grid");
  const image = required(json.image as string, "image");
  if (Math.abs(format - SUPPORTED_FORMAT) > Number.EPSILON) {
    throw new ImportError(
      400,
      `unsupported UVTT format version ${format}; only ${SUPPORTED_FORMAT} is supported`,
    );
  }
  let skipped = 0;
  const polygons = (value: unknown): Point[][] =>
    ((value as Point[][] | undefined) ?? []).filter((polygon) => {
      const keep = polygon.length >= 2;
      if (!keep) skipped += 1;
      return keep;
    });
  const file: UvttFile = {
    format,
    resolution: resolution as unknown as UvttFile["resolution"],
    line_of_sight: polygons(json.line_of_sight),
    objects_line_of_sight: polygons(json.objects_line_of_sight),
    portals: ((json.portals as Row[] | undefined) ?? []).map((p) => ({
      bounds: (p.bounds as Point[]) ?? [],
      closed: p.closed === true,
      freestanding: p.freestanding === true,
    })),
    environment: (json.environment as UvttFile["environment"]) ?? {},
    lights: ((json.lights as Row[] | undefined) ?? []).map((l) => ({
      position: required(l.position as Point, "position"),
      range: required(l.range as number, "range"),
      intensity: (l.intensity as number | undefined) ?? 1,
      color: required(l.color as string, "color"),
      shadows: l.shadows === true,
    })),
    image,
  };
  return { file, skipped };
}

/** `ambient::brightness`: perceived brightness of `AARRGGBB` or `RRGGBB`. */
function brightness(value: string): number | null {
  const hex = value.trim().replace(/^#+/, "");
  const rgb = hex.length === 8 ? hex.slice(2) : hex.length === 6 ? hex : null;
  if (!rgb || !/^[0-9a-fA-F]{6}$/.test(rgb)) return null;
  const [r, g, b] = [0, 2, 4].map(
    (i) => parseInt(rgb.slice(i, i + 2), 16) / 255,
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** `ambient::ambient_level`. */
export function ambientLevel(ambient?: string | null): string {
  const b = typeof ambient === "string" ? brightness(ambient) : null;
  if (b !== null && b < 0.35) return "dark";
  if (b !== null && b < 0.75) return "dim";
  return "bright";
}

/** `warnings.rs`. */
function warningsFor(file: UvttFile): string[] {
  const out: string[] = [];
  const free = file.portals.filter((p) => p.freestanding).length;
  if (free > 0) {
    out.push(
      `${free} freestanding portal${free === 1 ? "" : "s"} present in the source file; freestanding portals are not attached to wall geometry and may not appear as expected`,
    );
  }
  const objects = file.objects_line_of_sight.length;
  if (objects > 0) {
    out.push(
      `${objects} objects_line_of_sight occluder polygon${objects === 1 ? "" : "s"} present in the source file; object-attached vision-blocking geometry is imported as ordinary static walls, not as object-linked occluders`,
    );
  }
  return out;
}

/** `transcode::stored_cell_size`: a whole cell that keeps under the cap. */
export function storedCellSize(
  width: number,
  height: number,
  pixelsPerGrid: number,
): number | null {
  const cap = MAX_CANVAS_TEXTURE_DIMENSION;
  if (width <= cap && height <= cap) return null;
  if (!Number.isFinite(pixelsPerGrid) || pixelsPerGrid < 1) return null;
  const cell = Math.floor((pixelsPerGrid * cap) / Math.max(width, height));
  return cell < 1 ? null : cell;
}

/** `ScenePlacement::point`. */
export function placed(
  p: Point,
  grid: number,
  width: number,
  height: number,
): [number, number] {
  return [p.x * grid - width / 2, height / 2 - p.y * grid];
}

/** `perimeter::TOLERANCE`: how far off an edge a wall may sit and lie on it. */
const TOLERANCE = 0.5;

interface Segment {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** `perimeter::Edge`: `horizontal` edges sit at `y = at`, from x `from` to `to`. */
interface Edge {
  horizontal: boolean;
  at: number;
  from: number;
  to: number;
}

/** `perimeter::edges`: top, right, bottom, left; centred, y up. */
function edgesOf(width: number, height: number): Edge[] {
  const [w, h] = [width / 2, height / 2];
  return [
    { horizontal: true, at: h, from: -w, to: w },
    { horizontal: false, at: w, from: -h, to: h },
    { horizontal: true, at: -h, from: -w, to: w },
    { horizontal: false, at: -w, from: -h, to: h },
  ];
}

/** `Edge::local`: (distance off the edge's line, position along it). */
function local(edge: Edge, x: number, y: number): [number, number] {
  return edge.horizontal
    ? [Math.abs(y - edge.at), x]
    : [Math.abs(x - edge.at), y];
}

/** `Edge::covered_by`. */
function coveredBy(edge: Edge, wall: Segment): [number, number] | null {
  const [offA, alongA] = local(edge, wall.x1, wall.y1);
  const [offB, alongB] = local(edge, wall.x2, wall.y2);
  if (offA > TOLERANCE || offB > TOLERANCE) return null;
  const start = Math.max(Math.min(alongA, alongB), edge.from);
  const end = Math.min(Math.max(alongA, alongB), edge.to);
  return end > start ? [start, end] : null;
}

function edgeWall(edge: Edge, start: number, end: number): Segment {
  return edge.horizontal
    ? { x1: start, y1: edge.at, x2: end, y2: edge.at }
    : { x1: edge.at, y1: start, x2: edge.at, y2: end };
}

/** `perimeter::perimeter_walls`: the edges, less what `existing` covers. */
export function perimeterWalls(
  width: number,
  height: number,
  existing: Segment[],
): Segment[] {
  const walls: Segment[] = [];
  for (const edge of edgesOf(width, height)) {
    const covered = existing
      .map((wall) => coveredBy(edge, wall))
      .filter((c): c is [number, number] => c !== null)
      .sort((a, b) => a[0] - b[0]);
    let cursor = edge.from;
    for (const [start, end] of covered) {
      if (start - cursor > TOLERANCE) walls.push(edgeWall(edge, cursor, start));
      cursor = Math.max(cursor, end);
    }
    if (edge.to - cursor > TOLERANCE) {
      walls.push(edgeWall(edge, cursor, edge.to));
    }
  }
  return walls;
}

/** `perimeter::lies_on_bounds`. */
export function liesOnBounds(
  wall: Segment,
  width: number,
  height: number,
): boolean {
  return edgesOf(width, height).some((edge) => {
    const [offA, alongA] = local(edge, wall.x1, wall.y1);
    const [offB, alongB] = local(edge, wall.x2, wall.y2);
    const inside = (along: number) =>
      along >= edge.from - TOLERANCE && along <= edge.to + TOLERANCE;
    return (
      offA <= TOLERANCE && offB <= TOLERANCE && inside(alongA) && inside(alongB)
    );
  });
}

/** `perimeter::is_marked`. */
function isMarked(metadata: unknown): boolean {
  return (
    !!metadata &&
    typeof metadata === "object" &&
    (metadata as Row).perimeter === true
  );
}

/** The bytes of the file's `image`, checked as `save_background_image` does. */
function imageBytes(base64: string): Uint8Array {
  let binary: string;
  try {
    binary = atob(base64);
  } catch (error) {
    throw new ImportError(
      400,
      `image field is not valid base64: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  const isPng = png.every((b, i) => bytes[i] === b);
  const ascii = (from: number) =>
    String.fromCharCode(...bytes.subarray(from, from + 4));
  const isWebp =
    bytes.length >= 12 && ascii(0) === "RIFF" && ascii(8) === "WEBP";
  if (!isPng && !isWebp) {
    throw new ImportError(400, "decoded image does not look like a PNG file");
  }
  return bytes;
}

async function importUvttFile(
  sceneId: string,
  raw: string,
  viewer: Viewer,
  wallEdges: boolean,
): Promise<Row> {
  const { file, skipped } = parseUvtt(raw);
  const warnings = warningsFor(file);
  const state = demoState();
  const scene = state.scenes.find((s) => s.sceneId === sceneId);
  if (!scene || viewer === "player") {
    throw new ImportError(403, "scene not found or not owned by caller");
  }

  const ppg = file.resolution.pixels_per_grid;
  let grid = Math.max(1, Math.round(ppg));
  const bytes = imageBytes(file.image);
  let background: Row;
  try {
    background = await storeImage(
      state,
      sceneId,
      "BACKGROUND",
      new Blob([bytes as BlobPart]),
      (width, height) => {
        const cell = storedCellSize(width, height, ppg);
        if (cell === null) {
          const scale = Math.min(
            1,
            MAX_CANVAS_TEXTURE_DIMENSION / Math.max(width, height),
          );
          return [
            Math.max(1, Math.round(width * scale)),
            Math.max(1, Math.round(height * scale)),
          ];
        }
        grid = cell;
        const scale = cell / ppg;
        return [
          Math.max(1, Math.round(width * scale)),
          Math.max(1, Math.round(height * scale)),
        ];
      },
    );
  } catch (error) {
    throw new ImportError(
      500,
      `storage error: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const width = background.widthPx as number;
  const height = background.heightPx as number;
  // The scene's size before this import: where an earlier one's edges are.
  const oldWidth = scene.width as number;
  const oldHeight = scene.height as number;
  const at = now();
  const levelId = levelsOf(state, sceneId).find((l) => l.isEntry)?.levelId;
  const by = { createdBy: DEMO_USER.id, updatedBy: DEMO_USER.id };
  const row = (
    { x1, y1, x2, y2 }: Segment,
    doorState: string,
    perimeter = false,
  ): Row => ({
    wallId: crypto.randomUUID(),
    sceneId,
    levelId,
    x1,
    y1,
    x2,
    y2,
    blocksVision: true,
    blocksMovement: true,
    doorState,
    locked: false,
    secret: false,
    metadata: perimeter ? { perimeter: true } : null,
    ...by,
    createdAt: at,
    updatedAt: at,
  });
  const wall = (a: Point, b: Point, doorState: string): Row => {
    const [x1, y1] = placed(a, grid, width, height);
    const [x2, y2] = placed(b, grid, width, height);
    return row({ x1, y1, x2, y2 }, doorState);
  };
  const pairs = (polygons: Point[][]) =>
    polygons.flatMap((polygon) =>
      polygon.slice(1).map((point, i) => wall(polygon[i], point, "NONE")),
    );
  const walls = [
    ...pairs(file.line_of_sight),
    ...pairs(file.objects_line_of_sight),
  ];
  const doors = file.portals.flatMap((portal) =>
    portal.bounds.length >= 2
      ? [
          wall(
            portal.bounds[0],
            portal.bounds[1],
            portal.closed ? "CLOSED" : "OPEN",
          ),
        ]
      : [],
  );
  const lights = file.lights.map((light) => {
    const [x, y] = placed(light.position, grid, width, height);
    const radius = light.range * grid;
    return {
      lightId: crypto.randomUUID(),
      sceneId,
      levelId,
      x,
      y,
      radius,
      brightRadius: radius * 0.5,
      intensity: light.intensity,
      color: light.color,
      attachedTokenId: null,
      castsShadows: light.shadows,
      metadata: null,
      ...by,
      createdAt: at,
      updatedAt: at,
    };
  });
  const edges = wallEdges
    ? perimeterWalls(width, height, [
        ...walls,
        ...doors,
      ] as unknown as Segment[]).map((segment) => row(segment, "NONE", true))
    : [];
  walls.push(...edges);
  if (wallEdges) {
    // `scene_write::remove_old_perimeter`: an earlier import's edge walls
    // still on the old edges go; one the Game Master moved stays.
    const stale = (w: Row) =>
      w.sceneId === sceneId &&
      w.levelId === levelId &&
      isMarked(w.metadata) &&
      liesOnBounds(w as unknown as Segment, oldWidth, oldHeight);
    for (let i = state.walls.length - 1; i >= 0; i -= 1) {
      if (stale(state.walls[i])) state.walls.splice(i, 1);
    }
  }
  state.walls.push(...walls, ...doors);
  state.lights.push(...lights);

  const metadata =
    scene.metadata &&
    typeof scene.metadata === "object" &&
    !Array.isArray(scene.metadata)
      ? (scene.metadata as Row)
      : {};
  Object.assign(scene, {
    backgroundAssetId: background.id,
    backgroundUrl: backgroundUrlOf(background.id as string),
    gridSize: grid,
    gridType: "square",
    width,
    height,
    ambientLight: ambientLevel(file.environment.ambient_light),
    metadata: {
      ...metadata,
      mapImport: {
        sourceMapCellsX: file.resolution.map_size.x,
        sourceMapCellsY: file.resolution.map_size.y,
        sourcePixelsPerGrid: ppg,
      },
    },
    updatedAt: at,
  });
  mirrorToEntry(state, scene);
  record(EVENT_MAP_IMPORTED, {
    scene_id: sceneId,
    walls_created: walls.length,
    doors_created: doors.length,
    lights_created: lights.length,
    perimeter_walls_created: edges.length,
    background_image_set: true,
  });
  markChanged();
  return {
    wallsCreated: walls.length,
    doorsCreated: doors.length,
    lightsCreated: lights.length,
    backgroundImageSet: true,
    skippedDegeneratePolygons: skipped,
    perimeterWallsCreated: edges.length,
    warnings,
  };
}

/** `map_import::wall_edges_from`. */
function wallEdgesFrom(value: FormDataEntryValue | null | undefined): boolean {
  if (typeof value !== "string") return true;
  return !["false", "0", "off"].includes(value.trim().toLowerCase());
}

/**
 * The endpoint: the multipart `file` field, as `import_uvtt` reads it, and
 * `wallEdges`, on unless it says `false`, `0` or `off`.
 */
export async function importUvtt(
  sceneId: string,
  form: FormData | null,
  /** The asking tab's (spec 081 R7): the world's is whoever asked last. */
  viewer: Viewer,
): Promise<ImportAnswer> {
  try {
    const field = form?.get("file");
    if (!(field instanceof Blob)) {
      throw new ImportError(400, "no file field found in multipart upload");
    }
    if (field.size > MAX_UPLOAD_BYTES) {
      throw new ImportError(413, "upload exceeds the maximum allowed size");
    }
    return {
      status: 200,
      body: await importUvttFile(
        sceneId,
        await field.text(),
        viewer,
        wallEdgesFrom(form?.get("wallEdges")),
      ),
    };
  } catch (error) {
    if (error instanceof ImportError) {
      return { status: error.status, body: { error: error.message } };
    }
    throw error;
  }
}
