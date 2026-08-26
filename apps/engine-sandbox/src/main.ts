/**
 * Standalone harness for the Bevy engine.
 *
 * The engine's entire integration surface is three wasm exports — `start`,
 * `apply_world_command`, `set_event_callback` — so nothing about driving it
 * requires the app: no server, no session cookie, no GraphQL, no React. This
 * page calls those three directly with canned commands and static map files.
 *
 * What that buys, concretely: the app's edit-compile-observe loop for an
 * engine change is "rebuild wasm → restart dev stack → log in → navigate
 * into a world → wait for the scene to load". Here it is "rebuild wasm →
 * reload the page". Every rendering question — does a sprite draw, does a
 * map load, does the camera frame it — is answerable without any of the
 * plumbing between the browser and the renderer.
 *
 * It is also a control experiment. Maps here are ordinary relative asset
 * paths (`maps/x.webp` under Bevy's default "assets" root), where the app
 * uses rooted, authenticated `/api/canvas-assets/...` URLs. If a map renders
 * here and not there, the fault is in the plumbing; if it renders in neither,
 * the fault is in the engine.
 */

import { FrameSampler, type FrameStats } from "./frameStats";
import { RAMP_AXES, reset as resetStress, SCENARIOS } from "./stress";
import init, {
  apply_world_command,
  engine_stats,
  set_event_callback,
  start,
} from "@thunderforge/engine";

interface MapEntry {
  name: string;
  image: string;
  widthPx: number;
  heightPx: number;
  pixelsPerGrid: number;
  walls: number;
  portals: number;
  lights: number;
  bytes: number;
}

const readout = document.getElementById("readout") as HTMLDivElement;
const lines: string[] = [];

function log(message: string): void {
  lines.push(message);
  while (lines.length > 12) lines.shift();
  readout.textContent = lines.join("\n");
  console.log(`[sandbox] ${message}`);
}

/** Sends a `WorldCommand` exactly as `apps/web`'s store bridge would. */
function send(command: Record<string, unknown>): void {
  apply_world_command(JSON.stringify(command));
}

/**
 * Reads the framebuffer directly rather than screenshotting.
 *
 * `getContext("webgl2")` returns the *same* context the engine is drawing
 * with (contexts are cached per canvas), and reading inside a rAF callback
 * lands right after the engine's draw for that frame. A screenshot would be
 * unreliable here: wgpu does not request `preserveDrawingBuffer`, so a
 * capture taken outside the frame can come back cleared even when the
 * renderer is working.
 */
function readGpuPixels(): Promise<{
  distinct: number;
  dominantFraction: number;
  dominant: [number, number, number];
  alpha: number;
  contextLost: boolean;
  drawingBuffer: [number, number];
}> {
  return new Promise((resolve, reject) => {
    // The last canvas in the document is the engine's in both attach modes:
    // in real-canvas mode there is only one, and in body mode winit appends
    // its own after everything else.
    const canvases = document.querySelectorAll("canvas");
    const canvas = canvases[canvases.length - 1] as HTMLCanvasElement | undefined;
    if (!canvas) return reject(new Error("no canvas — has the engine started?"));
    const gl = canvas.getContext("webgl2");
    if (!gl) return reject(new Error("no webgl2 context"));

    requestAnimationFrame(() => {
      const size = 240;
      const x = Math.max(0, Math.floor(canvas.width / 2 - size / 2));
      const y = Math.max(0, Math.floor(canvas.height / 2 - size / 2));
      const pixels = new Uint8Array(size * size * 4);
      gl.readPixels(x, y, size, size, gl.RGBA, gl.UNSIGNED_BYTE, pixels);

      const counts = new Map<string, number>();
      for (let i = 0; i < pixels.length; i += 4) {
        const key = `${pixels[i]},${pixels[i + 1]},${pixels[i + 2]}`;
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }

      let bestKey = "0,0,0";
      let bestCount = 0;
      for (const [key, count] of counts) {
        if (count > bestCount) [bestKey, bestCount] = [key, count];
      }

      resolve({
        distinct: counts.size,
        dominantFraction: bestCount / (size * size),
        dominant: bestKey.split(",").map(Number) as [number, number, number],
        alpha: pixels[3],
        // A lost context reads back as transparent black no matter what was
        // drawn, so this distinguishes "renderer drew nothing" from "the GPU
        // context died" — two very different bugs with identical pixels.
        contextLost: gl.isContextLost(),
        drawingBuffer: [gl.drawingBufferWidth, gl.drawingBufferHeight],
      });
    });
  });
}

async function boot(): Promise<void> {
  log("loading wasm…");
  await init();

  set_event_callback((payload: string) => {
    // The engine emits the same events it would send the app's world store.
    console.debug("[sandbox] engine event", payload);
  });

  // `?attach=body` reproduces what `apps/web` does today — it passes
  // `#game-canvas-container`, the id of a <div>. winit's `with_canvas` takes
  // an `Option<HtmlCanvasElement>`, and the cast from a non-canvas element
  // silently yields `None`, so winit creates its own canvas and appends it
  // to <body>. Anything else attaches Bevy to the real <canvas> in the page.
  // Having both is the whole reason this harness can isolate the difference.
  const attachToBody = new URLSearchParams(location.search).get("attach") === "body";
  if (attachToBody) {
    // Drop the page's own canvas first. winit will append its own, and
    // leaving two in the document makes every "read the canvas" probe
    // ambiguous — `querySelector("canvas")` would return the unused one and
    // report a transparent, never-drawn buffer.
    document.getElementById("engine-canvas")?.remove();
  }
  const selector = attachToBody ? "#stage" : "#engine-canvas";
  start(selector);
  log(`engine started (selector ${selector}${attachToBody ? " → winit self-creates a body canvas" : " → real canvas"})`);
  const attachNote = document.getElementById("attach-mode");
  if (attachNote) {
    attachNote.textContent = attachToBody
      ? "attach=body — winit created its own canvas (matches apps/web)"
      : "attached to the page's real <canvas> (add ?attach=body to compare)";
  }

  send({ type: "set_is_game_master", isGameMaster: true });
  send({ type: "set_world", worldId: "sandbox" });

  const maps: MapEntry[] = await fetch("/assets/maps/manifest.json").then((r) => r.json());
  const container = document.getElementById("maps") as HTMLDivElement;

  for (const map of maps) {
    const button = document.createElement("button");
    button.textContent = `${map.name} — ${map.widthPx}×${map.heightPx}`;
    button.title = `${map.walls} walls · ${map.portals} portals · ${map.lights} lights · ${(map.bytes / 1024 / 1024).toFixed(1)}MB`;
    button.addEventListener("click", () => {
      send({
        type: "set_scene_background",
        backgroundImagePath: map.image,
        width: map.widthPx,
        height: map.heightPx,
        worldId: "sandbox",
      });
      // A real import also writes the scene's grid_size from the file's
      // `pixels_per_grid`, so the sandbox does the same — otherwise the grid
      // would keep whatever size the slider last had and visibly disagree
      // with the art.
      currentMap = map;
      gridSize = map.pixelsPerGrid;
      sizeInput.value = String(gridSize);
      sizeValue.textContent = String(gridSize);
      pushGrid();
      log(`set_scene_background → ${map.image} (${map.widthPx}×${map.heightPx}) @ ${map.pixelsPerGrid}px/cell`);
    });
    container.append(button);
  }

  // --- Grid -------------------------------------------------------------
  // The grid is what tokens snap to, what movement is measured in, and what
  // an imported map's `pixels_per_grid` feeds. Driving it from here exercises
  // the same `set_scene_grid` command the app sends.
  let gridType = "square";
  let gridSize = 128;
  let gridVisible = true;
  // Remembered so the grid can anchor to the map, and "fit map to view" knows
  // what it is fitting.
  let currentMap: MapEntry | null = null;

  const pushGrid = () => {
    send({
      type: "set_scene_grid",
      gridType,
      size: gridSize,
      visible: gridVisible,
      // The map's extent, so the engine anchors the lattice to the map's
      // corner. Without it the grid is anchored at the world origin and lands
      // half a cell out on any map with an odd cell count — which is half of
      // the example maps.
      ...(currentMap
        ? { mapWidth: currentMap.widthPx, mapHeight: currentMap.heightPx }
        : {}),
    });
    log(`grid → ${gridType} @ ${gridSize}px ${gridVisible ? "" : "(hidden)"}`);
  };

  const gridControls = document.getElementById("grid-controls") as HTMLDivElement;
  for (const kind of ["square", "hex", "hex_flat", "gridless"]) {
    const button = document.createElement("button");
    button.textContent = kind;
    button.addEventListener("click", () => {
      gridType = kind;
      pushGrid();
    });
    gridControls.append(button);
  }

  const sizeInput = document.getElementById("grid-size") as HTMLInputElement;
  const sizeValue = document.getElementById("grid-size-value") as HTMLSpanElement;
  sizeInput.addEventListener("input", () => {
    gridSize = Number(sizeInput.value);
    sizeValue.textContent = String(gridSize);
    pushGrid();
  });

  const visibleInput = document.getElementById("grid-visible") as HTMLInputElement;
  visibleInput.addEventListener("change", () => {
    gridVisible = visibleInput.checked;
    pushGrid();
  });

  pushGrid();

  // --- Stress ------------------------------------------------------------
  // Named, repeatable load scenarios with frame timing, so "does it hold up"
  // is a number rather than an impression. Exposed on `window` as well, which
  // is what `scripts/stress.mjs` drives headlessly.
  const sampler = new FrameSampler();
  const placed = { tokens: 0, lights: 0, walls: 0 };

  const runScenario = async (name: string, sampleMs = 4000): Promise<FrameStats> => {
    const scenario = SCENARIOS.find((s) => s.name === name);
    if (!scenario) throw new Error(`no scenario ${name}`);

    resetStress(send, placed);
    // Generous: the engine drains its command queue on the next frame, and a
    // large scenario's spawns plus the first shadow rebuild need to settle
    // before timing starts, or the setup cost lands in the sample.
    await new Promise((r) => setTimeout(r, 600));

    scenario.run(send);
    // Track what to clear next time.
    const magnitude = /^(tokens|lights|walls)-(\d+)$/.exec(scenario.name);
    if (magnitude) {
      placed[magnitude[1] as "tokens" | "lights" | "walls"] = Number(magnitude[2]);
    } else {
      Object.assign(placed, { tokens: 60, lights: 24, walls: 120 });
    }

    await new Promise((r) => setTimeout(r, 1200));
    sampler.start();
    await new Promise((r) => setTimeout(r, sampleMs));
    const stats = sampler.stop();
    // The engine's own counters, which see below the vsync ceiling that pins
    // every browser-side measurement at 16.7ms.
    const engine = JSON.parse(engine_stats());
    log(
      `${scenario.name}: present ${stats.p95Ms.toFixed(1)}ms p95 · ` +
        `engine ${engine.frame_time_ms.toFixed(2)}ms · ` +
        `${engine.sprites} sprites ${engine.shadow_quads} shadows`,
    );
    return { ...stats, engine };
  };

  const stressControls = document.getElementById("stress-controls") as HTMLDivElement;
  for (const scenario of SCENARIOS) {
    const button = document.createElement("button");
    button.textContent = `${scenario.name} (${scenario.magnitude})`;
    button.title = scenario.description;
    button.addEventListener("click", () => void runScenario(scenario.name));
    stressControls.append(button);
  }

  /**
   * Pushes one axis until the frame budget breaks, returning the last count
   * that held.
   *
   * This is the measurement that survives vsync. A fixed-load run on a fast
   * machine reports 16.7ms whether the GPU is 5% or 95% busy; a capacity
   * number moves the moment the cost per unit changes.
   */
  const rampAxis = async (
    axis: keyof typeof RAMP_AXES,
    budgetMs: number,
    max = 4000,
  ): Promise<{ axis: string; capacity: number; brokeAt: number | null; p95AtCapacity: number }> => {
    let capacity = 0;
    let p95AtCapacity = 0;

    // Doubling rather than stepping: the interesting range spans orders of
    // magnitude, and a linear walk to 4000 would take longer than anyone
    // will wait.
    for (let count = 25; count <= max; count *= 2) {
      resetStress(send, placed);
      await new Promise((r) => setTimeout(r, 400));

      RAMP_AXES[axis](count).run(send);
      placed[axis] = count;
      await new Promise((r) => setTimeout(r, 900));

      sampler.start();
      await new Promise((r) => setTimeout(r, 2500));
      const stats = sampler.stop();
      const engine = JSON.parse(engine_stats());

      log(
        `ramp ${axis} ${count}: present p95 ${stats.p95Ms.toFixed(1)}ms · ` +
          `engine ${engine.frame_time_ms.toFixed(2)}ms`,
      );

      if (stats.p95Ms > budgetMs) {
        return { axis, capacity, brokeAt: count, p95AtCapacity };
      }
      capacity = count;
      p95AtCapacity = stats.p95Ms;
    }

    // Never broke — capacity is at least `max`, which is itself worth
    // reporting rather than pretending it is a measured limit.
    return { axis, capacity, brokeAt: null, p95AtCapacity };
  };

  (window as unknown as Record<string, unknown>).__stress = {
    engineStats: () => JSON.parse(engine_stats()),
    runScenario,
    rampAxis,
    rampAxes: Object.keys(RAMP_AXES),
    scenarios: SCENARIOS.map((s) => ({
      name: s.name,
      description: s.description,
      magnitude: s.magnitude,
    })),
    setZoom: (zoom: number) => send({ type: "set_camera", zoom }),
    loadMap: (name: string) => {
      const map = maps.find((m) => m.name === name);
      if (!map) throw new Error(`no map ${name}`);
      currentMap = map;
      send({
        type: "set_scene_background",
        backgroundImagePath: map.image,
        width: map.widthPx,
        height: map.heightPx,
        worldId: "sandbox",
      });
      gridSize = map.pixelsPerGrid;
      pushGrid();
    },
  };

  // --- Camera ------------------------------------------------------------
  // `zoom` is world units per screen unit, so larger is zoomed *out*. The
  // engine's zoom drives the orthographic projection rather than the camera
  // transform, which is what keeps the grid and lighting culls correct.
  const cameraControls = document.getElementById("camera-controls") as HTMLDivElement;
  const zoomInput = document.getElementById("zoom") as HTMLInputElement;
  const zoomValue = document.getElementById("zoom-value") as HTMLSpanElement;

  const setZoom = (zoom: number) => {
    zoomInput.value = String(zoom);
    zoomValue.textContent = zoom.toFixed(2);
    send({ type: "set_camera", zoom });
  };

  zoomInput.addEventListener("input", () => setZoom(Number(zoomInput.value)));

  const resetButton = document.createElement("button");
  resetButton.textContent = "Reset camera (1:1, centred)";
  resetButton.addEventListener("click", () => {
    send({ type: "set_camera", x: 0, y: 0, zoom: 1 });
    zoomInput.value = "1";
    zoomValue.textContent = "1.00";
    log("camera reset");
  });
  cameraControls.append(resetButton);

  const fitButton = document.createElement("button");
  fitButton.textContent = "Fit map to view";
  fitButton.addEventListener("click", () => {
    if (!currentMap) {
      log("load a map first");
      return;
    }
    send({
      type: "fit_camera_to",
      centerX: 0,
      centerY: 0,
      width: currentMap.widthPx,
      height: currentMap.heightPx,
    });
    log(`fit ${currentMap.widthPx}x${currentMap.heightPx} to view`);
  });
  cameraControls.append(fitButton);

  // --- Tokens -------------------------------------------------------------
  // Footprint is expressed in cells, so a token's on-screen size follows the
  // scene's grid rather than being stored in pixels — load a map with a
  // different pixels_per_grid and every token resizes with it.
  const sizeControls = document.getElementById("token-sizes") as HTMLDivElement;
  const SIZES: [string, number][] = [
    ["Tiny (0.5)", 0.5],
    ["Medium (1)", 1],
    ["Large (2)", 2],
    ["Huge (3)", 3],
    ["Gargantuan (4)", 4],
  ];
  for (const [label, footprint] of SIZES) {
    const button = document.createElement("button");
    button.textContent = label;
    button.addEventListener("click", () => {
      // The engine's two demo tokens, so the difference is visible side by side.
      for (const tokenId of ["player", "npc"]) {
        send({ type: "set_token_grid", tokenId, footprint, snap: true });
      }
      log(`tokens → ${footprint} cell${footprint === 1 ? "" : "s"} across`);
    });
    sizeControls.append(button);
  }

  const snapInput = document.getElementById("grid-snap") as HTMLInputElement;
  snapInput.addEventListener("change", () => {
    send({ type: "set_grid_snap", enabled: snapInput.checked });
    log(`grid snapping ${snapInput.checked ? "on" : "off"}`);
  });

  // Distance vocabulary. The planned-route label is quoted in whatever the
  // scene says a cell is worth, so the same engine reads correctly for a game
  // measured in feet, metres or abstract units.
  const unitPresets = document.getElementById("unit-presets") as HTMLDivElement;
  for (const [label, perCell, unit] of [
    ["D&D 5e — 5 ft", 5, "ft"],
    ["Metric — 1.5 m", 1.5, "m"],
    ["Abstract — 1 Unit", 1, "Unit"],
  ] as [string, number, string][]) {
    const button = document.createElement("button");
    button.textContent = label;
    button.addEventListener("click", () => {
      send({ type: "set_grid_units", perCell, label: unit });
      log(`units → 1 cell = ${perCell} ${unit}`);
    });
    unitPresets.append(button);
  }

  // --- Lighting ----------------------------------------------------------
  // Lighting is hard to confirm from its output: a hidden token looks the
  // same whether it was occluded, out of a vision cone, or simply unlit. The
  // overlay draws the inputs so the difference is legible.
  const overlayInput = document.getElementById("lighting-overlay") as HTMLInputElement;
  overlayInput.addEventListener("change", () => {
    send({ type: "set_lighting_overlay", enabled: overlayInput.checked });
    log(`lighting overlay ${overlayInput.checked ? "ON" : "off"}`);
  });

  const ambientControls = document.getElementById("ambient-controls") as HTMLDivElement;
  for (const level of ["bright", "dim", "dark"]) {
    const button = document.createElement("button");
    button.textContent = `ambient: ${level}`;
    button.addEventListener("click", () => {
      send({ type: "set_ambient_light", level });
      log(`ambient → ${level}`);
    });
    ambientControls.append(button);
  }

  let lightSeq = 0;
  document.getElementById("add-torch")?.addEventListener("click", () => {
    lightSeq += 1;
    send({
      type: "upsert_light",
      light: {
        id: `sandbox-torch-${lightSeq}`,
        sceneId: "sandbox",
        // Offset each torch so successive clicks are distinguishable.
        x: (lightSeq % 3) * 220 - 220,
        y: 0,
        radius: 320,
        intensity: 1,
        color: "#ffc880",
        attachedTokenId: null,
        castsShadows: true,
      },
    });
    log(`torch ${lightSeq} placed (bright 160 / dim 320)`);
  });

  // The scenario: a torch on the player that does NOT reach the NPC. Without
  // darkvision the NPC is simply not there as far as the player is concerned —
  // the mimic you cannot see until it moves.
  document.getElementById("mimic-demo")?.addEventListener("click", () => {
    send({ type: "set_ambient_light", level: "dark" });
    send({
      type: "upsert_light",
      light: {
        id: "mimic-demo-torch",
        sceneId: "sandbox",
        // On the player token (the engine's demo token sits at -180).
        x: -180,
        y: 0,
        // Reaches 220 — the NPC at +180 is 360 away, comfortably outside.
        radius: 220,
        intensity: 1,
        color: "#ffc880",
        attachedTokenId: null,
        castsShadows: true,
      },
    });
    log("mimic demo: dark scene, torch on the player, NPC outside its reach");
  });

  const darkvisionInput = document.getElementById("darkvision") as HTMLInputElement;
  darkvisionInput.addEventListener("change", () => {
    send({
      type: "set_token_vision",
      tokenId: "player",
      darkvision: darkvisionInput.checked ? 600 : 0,
      fov: Math.PI * 2,
    });
    log(`player darkvision ${darkvisionInput.checked ? "600" : "off"}`);
  });

  let wallSeq = 0;
  document.getElementById("add-wall")?.addEventListener("click", () => {
    wallSeq += 1;
    send({
      type: "upsert_wall",
      wall: {
        id: `sandbox-wall-${wallSeq}`,
        x1: 120,
        y1: -260,
        x2: 120,
        y2: 260,
        blocksVision: true,
        blocksMovement: true,
        doorState: "none",
      },
    });
    log(`wall ${wallSeq} placed — casts a shadow to its right`);
  });

  // The render probe draws through the gizmo pipeline rather than the sprite
  // pipeline. If these shapes appear while sprites do not, the 2D render
  // graph is healthy and the fault is specific to sprites.
  let probeOn = false;
  document.getElementById("gizmos")?.addEventListener("click", () => {
    probeOn = !probeOn;
    send({ type: "set_render_probe", enabled: probeOn });
    log(`render probe ${probeOn ? "ON" : "off"} — magenta rect, green circle, white diagonals`);
  });

  document.getElementById("clear-bg")?.addEventListener("click", () => {
    send({
      type: "set_scene_background",
      backgroundImagePath: null,
      width: 0,
      height: 0,
      worldId: "sandbox",
    });
    log("background cleared");
  });

  document.getElementById("probe")?.addEventListener("click", () => {
    void readGpuPixels().then((result) => {
      const flat = result.dominantFraction > 0.99;
      log(
        `pixels: dominant rgb(${result.dominant.join(", ")}) ` +
          `${(result.dominantFraction * 100).toFixed(1)}% · ${result.distinct} colours · alpha ${result.alpha}` +
          (flat ? "  ← FLAT: renderer drew nothing" : "  ← content present"),
      );
    }).catch((error) => log(`probe failed: ${String(error)}`));
  });

  log(`${maps.length} maps available`);
}

void boot().catch((error) => log(`boot failed: ${String(error)}`));

// Exposed for the headless render check (scripts/render-check.mjs).
(window as unknown as Record<string, unknown>).__sandbox = { readGpuPixels, send };
