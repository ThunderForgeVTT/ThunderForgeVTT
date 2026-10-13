import type { EngineMountOptions, EngineState } from "./types";
import { emitEngineLoad } from "./loadSignals";
import {
  getWasmModule,
  type EngineLoadListener,
  type EngineLoadStage,
} from "./wasmModule";

const state: EngineState = {
  started: false,
  canvasSelector: null,
  worldId: null,
};

/**
 * The engine is built for WebGL2 and nothing else. A browser that will not
 * hand one out (WebGL switched off, a blocklisted driver, a hardened profile)
 * used to find that out at the end of a 50 MB download, as a panic in
 * `create_surface` with a dead canvas behind it. Ask a throwaway canvas first;
 * the answer is the same one the engine would get, and it costs nothing.
 */
export function webgl2Unavailable(doc: Document = document): string | null {
  if (probeWebgl2(doc)) return null;
  // Nothing a page can ask for: there is no permission prompt for WebGL.
  // Privacy builds of Firefox (LibreWolf, Tor Browser, a hardened user.js)
  // turn it off as an anti-fingerprinting default, so say where the switch is.
  return (
    "This browser is not offering WebGL2, which the board needs to draw. " +
    "Privacy-focused browsers such as LibreWolf turn it off by default: " +
    'look for "Enable WebGL" in the browser\'s own settings, or set ' +
    "webgl.disabled to false in about:config, then reload. A blocklisted " +
    "graphics driver can also be the cause; another browser will tell."
  );
}

function probeWebgl2(doc: Document): WebGL2RenderingContext | null {
  try {
    return doc.createElement("canvas").getContext("webgl2");
  } catch {
    return null;
  }
}

/**
 * The widest the board may be, in CSS pixels, on this device.
 *
 * A GPU has a largest texture it will make, and the canvas the engine draws
 * into is one. WebGL2 guarantees only 2048px; phones, integrated chips and
 * hardened browsers stop there, and a 2560px window on one of them panicked
 * the engine in `Surface::configure`. The engine cannot shrink its own
 * canvas — on the web the backing store is the element's CSS size times the
 * device pixel ratio, set by the windowing layer — so the page keeps the
 * element within the limit instead. `null` when there is no WebGL2 to ask,
 * which `webgl2Unavailable` reports in words.
 */
export function boardSizeCeiling(
  doc: Document = document,
  pixelRatio: number = window.devicePixelRatio || 1,
): number | null {
  const gl = probeWebgl2(doc);
  if (!gl) return null;
  const limit = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
  if (!Number.isFinite(limit) || limit <= 0) return null;
  // Two pixels under the exact quotient, so a fractional ratio rounding the
  // backing store up cannot land one pixel over the limit.
  return Math.max(1, Math.floor(limit / pixelRatio) - 2);
}

export async function mountEngine(
  options: EngineMountOptions,
  onStageChange?: (stage: EngineLoadStage) => void,
  onProgress?: EngineLoadListener,
): Promise<void> {
  const unavailable = webgl2Unavailable();
  if (unavailable) {
    emitEngineLoad({ stage: "unavailable", reason: "no_webgl2" });
    throw new Error(unavailable);
  }
  onStageChange?.("downloading");
  emitEngineLoad({ stage: "downloading" });
  const module = await getWasmModule(onProgress);
  // FR-031: "starting" is a distinct phase, not the tail of the download.
  // Instantiation of a bundle this size is perceptible, and reporting it as
  // a download stalled at 100% is exactly the "is it broken?" moment this
  // story exists to remove.
  onStageChange?.("starting");
  emitEngineLoad({ stage: "starting" });
  onProgress?.({ stage: "starting", loaded: 0, total: null });

  state.canvasSelector = options.canvasSelector;
  state.worldId = options.worldId;

  if (!state.started) {
    module.start(options.canvasSelector);
    state.started = true;
    // The engine draws its first frame in the next animation frame; this
    // callback, queued after it, runs once that frame is done (spec 086 US7).
    requestAnimationFrame(() => emitEngineLoad({ stage: "started" }));
  }
}

export function setActiveWorld(worldId: string): void {
  state.worldId = worldId;
}

export function getEngineState(): Readonly<EngineState> {
  return state;
}
