// Spec 090 FR-009/FR-010/FR-013: the engine the demo ships, loaded only when
// the visitor asks for it.
//
// The demo's build names its hashed files in `/demo/engine.json`; nothing in
// the landing's build knows them. The wasm is fetched here, not by the
// engine's own init, so the bytes can be counted as they arrive. The count is
// of decoded bytes, because fetch decodes the body, so the total is
// `bytes.raw`; the readout shows the wire size beside it.

import { cachedWasm, keepWasm } from "./wasmCache.ts";

export interface Manifest {
  js: string;
  wasm: string;
  bytes: { raw: number; br?: number; gz?: number };
  version: string;
}

/** The engine's JavaScript surface that the landing uses. */
export interface Engine {
  start(canvasSelector: string): void;
  apply_world_command(json: string): void;
  engine_stats(): string;
  dice_landed(): string;
  /** Spec 091: the one listener for the engine's events, JSON strings. */
  set_event_callback(callback: (json: string) => void): void;
  /** Whose sight the board shows; `""` for none. */
  set_viewer_token(tokenId: string): boolean;
  // Read-backs the room's tests and its DOM path ask instead of the canvas.
  movement_state(): string;
  /** `{ x, y, scale }`: the world point at the board's centre, and world units per board pixel. */
  camera_state(): string;
  door_icons(): string;
  sight_probe(x: number, y: number): string;
  list_interactives(): string;
  /** Spec 097: a way round the walls, `{"path":[{x,y},…]|null}`. Absent from older builds. */
  plan_path?(request: string): string;
  /** Spec 096: the ids of the tokens that are down, as a JSON array. */
  token_defeated?(): string;
}

/** Which step failed, as telemetry reports it (T038). */
export type Stage = "manifest" | "wasm" | "js" | "init" | "start";

export class EngineLoadError extends Error {
  constructor(
    readonly stage: Stage,
    cause: unknown,
  ) {
    super(`engine ${stage}: ${cause instanceof Error ? cause.message : String(cause)}`);
  }
}

export interface Progress {
  /** Decoded bytes so far. */
  loaded: number;
  /** `bytes.raw` from the manifest. */
  total: number;
  /** What the wire carries for the whole engine: brotli, else gzip, else raw. */
  wire: number;
}

const BASE = "/demo/";

async function step<T>(stage: Stage, run: () => Promise<T> | T): Promise<T> {
  try {
    return await run();
  } catch (e) {
    throw new EngineLoadError(stage, e);
  }
}

async function readManifest(signal?: AbortSignal): Promise<Manifest> {
  const res = await fetch(`${BASE}engine.json`, { cache: "no-cache", signal });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const m = (await res.json()) as Manifest;
  if (typeof m.js !== "string" || typeof m.wasm !== "string" || !(m.bytes?.raw > 0)) {
    throw new Error("malformed");
  }
  return m;
}

async function readWasm(
  m: Manifest,
  onProgress: (p: Progress) => void,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const total = m.bytes.raw;
  const wire = m.bytes.br ?? m.bytes.gz ?? total;
  const kept = await cachedWasm(m.wasm, total);
  if (kept) {
    onProgress({ loaded: total, total, wire });
    return kept;
  }
  const res = await fetch(BASE + m.wasm, { signal });
  if (!res.ok || !res.body) throw new Error(`HTTP ${res.status}`);
  const out = new Uint8Array(total);
  const reader = res.body.getReader();
  let loaded = 0;
  onProgress({ loaded, total, wire });
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (loaded + value.length > total) throw new Error("longer than the manifest says");
    out.set(value, loaded);
    loaded += value.length;
    onProgress({ loaded, total, wire });
  }
  if (loaded !== total) throw new Error(`${loaded} of ${total} bytes`);
  void keepWasm(m.wasm, out);
  return out;
}

/**
 * Fetch the manifest, stream the wasm, import the JS, initialise it with the
 * bytes and start drawing into `canvasSelector`.
 */
export async function loadEngine(
  canvasSelector: string,
  onProgress: (p: Progress) => void,
  signal?: AbortSignal,
): Promise<Engine> {
  const manifest = await step("manifest", () => readManifest(signal));
  const [bytes, mod] = await Promise.all([
    step("wasm", () => readWasm(manifest, onProgress, signal)),
    step(
      "js",
      () =>
        import(/* @vite-ignore */ BASE + manifest.js) as Promise<
          Engine & { default(opts: { module_or_path: BufferSource }): Promise<unknown> }
        >,
    ),
  ]);
  await step("init", () => mod.default({ module_or_path: bytes as Uint8Array<ArrayBuffer> }));
  await step("start", () => mod.start(canvasSelector));
  return mod;
}
