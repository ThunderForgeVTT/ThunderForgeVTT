import { useCallback, useEffect, useRef, useState } from "react";
import { DEMO } from "../links.ts";
import "./engine-frame.css";
import { FRAME_FAILED, type FrameFailed, type HeldBack } from "./frameEvents.ts";
import { EngineLoadError, loadEngine, type Engine, type Progress, type Stage } from "./loadEngine.ts";

// Spec 090 FR-011 to FR-019: the real engine in a frame on the page, only when
// the visitor asks for it.
//
// The frame owns everything about the engine's life: the poster before the
// click, the check that the browser can run it, the load and its progress,
// reduced motion, the pause out of view, and the failure with its retry. What
// it draws is the host's business: `setup` sends the scene, and `onState`
// hands the host a way to send commands once the engine runs. The encounter
// mounts the same frame with a different scene.

export type FrameState = "poster" | "unsupported" | "loading" | "running" | "paused" | "failed";
export type Send = (command: Record<string, unknown>) => void;
/** Where a failure happened: a load stage, or the engine stopping later. */
export type FailStage = Stage | "engine";

interface Props {
  /** The frame's DOM id; the engine draws into its canvas. */
  id: string;
  poster: { src: string; width: number; height: number };
  /** The button: "Throw it on the table". */
  action: string;
  /** The scene, sent once the engine has started. */
  setup?: (send: Send) => void;
  /** Every state change; `send` is set while the engine runs or is paused. */
  onState?: (state: FrameState, send: Send | null) => void;
}

const STOPPED = "thunderforge:engine-stopped";
const REDUCE = "(prefers-reduced-motion: reduce)";
/** The board the engine draws, in CSS pixels; the frame is its 16:9. */
const STAGE_W = 960;

/** Why this browser should not get the engine, or null when it should. */
function holdBack(): { reason: HeldBack; words: string } | null {
  const gl = document.createElement("canvas").getContext("webgl2");
  if (!gl) return { reason: "no_webgl2", words: "no WebGL2" };
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  const nav = navigator as Navigator & {
    connection?: { saveData?: boolean };
    deviceMemory?: number;
  };
  if (nav.connection?.saveData) return { reason: "save_data", words: "it asks to save data" };
  if (nav.deviceMemory !== undefined && nav.deviceMemory < 4) {
    return { reason: "low_memory", words: "it reports under 4 GB of memory" };
  }
  return null;
}

/**
 * Told to the page, not to telemetry: `telemetry.ts` listens, as it does for
 * the calls to action, so the frame never calls a telemetry API.
 */
function report(detail: FrameFailed) {
  window.dispatchEvent(new CustomEvent<FrameFailed>(FRAME_FAILED, { detail }));
}

const mb = (n: number) => (n / 1e6).toFixed(1);

export default function EngineFrame({ id, poster, action, setup, onState }: Props) {
  const [state, setState] = useState<FrameState>("poster");
  const [why, setWhy] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [stage, setStage] = useState<FailStage | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const engine = useRef<Engine | null>(null);
  const abort = useRef<AbortController | null>(null);
  // Props read from effects that must not rerun when a parent rerenders.
  const latest = useRef({ setup, onState });
  latest.current = { setup, onState };

  const send = useCallback<Send>((command) => {
    engine.current?.apply_world_command(JSON.stringify(command));
  }, []);

  const go = useCallback(
    (next: FrameState) => {
      setState(next);
      const live = next === "running" || next === "paused";
      latest.current.onState?.(next, live ? send : null);
    },
    [send],
  );

  const fail = useCallback(
    (at: FailStage) => {
      setStage(at);
      go("failed");
      report({ stage: at, reason: "error" });
    },
    [go],
  );

  useEffect(() => {
    const held = holdBack();
    if (held) {
      setWhy(held.words);
      go("unsupported");
      report({ stage: "probe", reason: held.reason });
    }
    return () => abort.current?.abort();
  }, [go]);

  async function boot() {
    abort.current?.abort();
    const ctl = new AbortController();
    abort.current = ctl;
    setProgress(null);
    setStage(null);
    go("loading");
    try {
      const e = await loadEngine(`#${id} canvas`, setProgress, ctl.signal);
      if (ctl.signal.aborted) return;
      engine.current = e;
      send({ type: "set_reduced_motion", reduced: matchMedia(REDUCE).matches });
      latest.current.setup?.(send);
      go("running");
    } catch (err) {
      if (ctl.signal.aborted) return;
      fail(err instanceof EngineLoadError ? err.stage : "start");
    }
  }

  const live = state === "running" || state === "paused";

  // A panic traps the module: every later call throws, so the frame says so.
  useEffect(() => {
    if (!live) return;
    const stopped = () => {
      engine.current = null;
      fail("engine");
    };
    window.addEventListener(STOPPED, stopped);
    return () => window.removeEventListener(STOPPED, stopped);
  }, [live, fail]);

  // FR-016: reduced motion follows the setting, every time it changes.
  useEffect(() => {
    if (!live) return;
    const mq = matchMedia(REDUCE);
    const changed = () => send({ type: "set_reduced_motion", reduced: mq.matches });
    mq.addEventListener("change", changed);
    return () => mq.removeEventListener("change", changed);
  }, [live, send]);

  // FR-017: no frames out of view or in a hidden tab.
  useEffect(() => {
    if (!live || !root.current) return;
    // Running and paused are both `live`, so flipping between them keeps
    // this observer; `on` remembers what the engine was last told.
    let inView = true;
    let on = true;
    const apply = () => {
      const want = inView && !document.hidden;
      if (want === on) return;
      on = want;
      send({ type: "set_rendering", on });
      go(on ? "running" : "paused");
    };
    const io = new IntersectionObserver(([entry]) => {
      inView = entry.isIntersecting;
      apply();
    });
    io.observe(root.current);
    document.addEventListener("visibilitychange", apply);
    return () => {
      io.disconnect();
      document.removeEventListener("visibilitychange", apply);
    };
  }, [live, send, go]);

  // The canvas is a picture of what the tray already says, so it stays out
  // of the tab order; the engine's window setup puts it in, so put it back.
  useEffect(() => {
    const canvas = root.current?.querySelector("canvas");
    if (!canvas) return;
    const mo = new MutationObserver(() => {
      if (canvas.tabIndex !== -1) canvas.tabIndex = -1;
    });
    mo.observe(canvas, { attributes: true, attributeFilter: ["tabindex"] });
    return () => mo.disconnect();
  }, []);

  // The stage is a 960x540 board, scaled to the frame's width.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      el.style.setProperty("--engine-scale", String(el.clientWidth / STAGE_W));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A trapped engine cannot start twice in one page.
  const canRetry = stage !== null && stage !== "start" && stage !== "init" && stage !== "engine";

  return (
    <div id={id} ref={root} className={`engine-frame engine-frame--${state}`}>
      <img
        className="engine-poster"
        src={poster.src}
        width={poster.width}
        height={poster.height}
        alt=""
        decoding="async"
        loading="lazy"
      />
      <div className="engine-stage">
        <canvas aria-hidden="true" tabIndex={-1} />
      </div>
      {!live && (
        <div className="engine-panel">
          {state === "poster" && (
            <button type="button" className="ink-btn ink-btn--blue" onClick={() => void boot()}>
              {action}
            </button>
          )}
          {state === "unsupported" && (
            <p className="engine-note">
              Your browser can't run the table here ({why}). The dice above are the same code.{" "}
              <a href={DEMO}>Try the demo</a> on another device.
            </p>
          )}
          {state === "loading" && (
            <>
              <progress
                max={progress?.total ?? 1}
                value={progress?.loaded ?? 0}
                aria-labelledby={`${id}-readout`}
              />
              <p id={`${id}-readout`} className="engine-note">
                {progress
                  ? `Loading the table: ${mb((progress.loaded / progress.total) * progress.wire)} of ${mb(progress.wire)} MB. Once; the demo reuses it.`
                  : "Finding the table…"}
              </p>
            </>
          )}
          {state === "failed" && (
            <>
              <p className="engine-note" role="alert">
                The table didn't load. The dice above still work.
              </p>
              {canRetry ? (
                <button type="button" className="ink-btn ink-btn--black" onClick={() => void boot()}>
                  Try again
                </button>
              ) : (
                <button type="button" className="ink-btn ink-btn--black" onClick={() => location.reload()}>
                  Reload the page
                </button>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
