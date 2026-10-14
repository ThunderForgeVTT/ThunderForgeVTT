import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { DEMO } from "../links.ts";
import "./engine-frame.css";
import { FRAME_FAILED, type FrameFailed, type HeldBack } from "./frameEvents.ts";
import { sendAs, stopped, take, type Send } from "./host.ts";
import { EngineLoadError, type Progress, type Stage } from "./loadEngine.ts";

// Spec 090 FR-011 to FR-019: the real engine in a frame on the page, only when
// the visitor asks for it.
//
// The frame owns everything about the engine's life: the poster before the
// click, the check that the browser can run it, the load and its progress,
// reduced motion, the pause out of view, and the failure with its retry. What
// it draws is the host's business: `setup` sends the scene, and `onState`
// hands the host a way to send commands once the engine runs. The encounter
// mounts the same frame with a different scene.
//
// Spec 091: the engine itself lives in `host.ts`, one per page. A frame takes
// the table from whichever frame had it, and that frame goes `away`.

export type FrameState =
  | "poster"
  | "unsupported"
  | "loading"
  | "running"
  | "paused"
  | "away"
  | "failed";
export type { Send };
/** Where a failure happened: a load stage, or the engine stopping later. */
export type FailStage = Stage | "engine";

interface Props {
  /** The frame's DOM id; the engine draws into its canvas. */
  id: string;
  poster: { src: string; width: number; height: number };
  /** The button: "Throw it on the table". */
  action: string;
  /** The scene, sent from scratch every time this frame takes the table. */
  setup?: (send: Send) => void;
  /** Removes what `setup` sent, when another frame takes the table. */
  teardown?: (send: Send) => void;
  /** Said while another frame has the table: where it is. */
  away: string;
  /** The room: the stage takes the pointer, but never the wheel. */
  interactive?: boolean;
  /** Spec 091 FR-022: the smallest WebGL2 `MAX_TEXTURE_SIZE` this scene needs. */
  maxTextureSize?: number;
  /** Every state change; `send` is set while the engine runs or is paused. */
  onState?: (state: FrameState, send: Send | null) => void;
  /** Drawn over the board, in board pixels (960x540), while the engine runs. */
  children?: ReactNode;
}

const STOPPED = "thunderforge:engine-stopped";
const REDUCE = "(prefers-reduced-motion: reduce)";
/** The board the engine draws, in CSS pixels; the frame is its 16:9. */
const STAGE_W = 960;

/** Why this browser should not get the engine, or null when it should. */
function holdBack(maxTextureSize = 0): { reason: HeldBack; words: string } | null {
  const gl = document.createElement("canvas").getContext("webgl2");
  if (!gl) return { reason: "no_webgl2", words: "no WebGL2" };
  const texture = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  if (texture < maxTextureSize) {
    return { reason: "small_texture", words: `its largest picture is ${texture} pixels` };
  }
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

export default function EngineFrame(props: Props) {
  const { id, poster, action, away, interactive, maxTextureSize } = props;
  const [state, setState] = useState<FrameState>("poster");
  const [why, setWhy] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [stage, setStage] = useState<FailStage | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const abort = useRef<AbortController | null>(null);
  // Props read from effects and the host, which must not rerun or go stale
  // when a parent rerenders.
  const latest = useRef(props);
  latest.current = props;

  const send = useMemo(() => sendAs(id), [id]);

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
    const held = holdBack(maxTextureSize);
    if (held) {
      setWhy(held.words);
      go("unsupported");
      report({ stage: "probe", reason: held.reason });
    }
    return () => abort.current?.abort();
  }, [go, maxTextureSize]);

  async function boot() {
    abort.current?.abort();
    const ctl = new AbortController();
    abort.current = ctl;
    setProgress(null);
    setStage(null);
    go("loading");
    try {
      await take(
        {
          id,
          stage: stageRef.current!,
          setup: (s) => {
            s({ type: "set_reduced_motion", reduced: matchMedia(REDUCE).matches });
            latest.current.setup?.(s);
          },
          teardown: (s) => latest.current.teardown?.(s),
          onTaken: () => go("away"),
        },
        setProgress,
        ctl.signal,
      );
      if (ctl.signal.aborted) return;
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
    const halt = () => {
      stopped();
      fail("engine");
    };
    window.addEventListener(STOPPED, halt);
    return () => window.removeEventListener(STOPPED, halt);
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

  // FR-021: the room's stage takes the pointer for the door icon, but the
  // wheel scrolls the page, so it stops here before the engine's window
  // listener sees it.
  useEffect(() => {
    const stage = stageRef.current;
    if (!interactive || !stage) return;
    const stop = (e: WheelEvent) => e.stopPropagation();
    stage.addEventListener("wheel", stop, { capture: true, passive: true });
    return () => stage.removeEventListener("wheel", stop, { capture: true });
  }, [interactive]);

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
    <div
      id={id}
      ref={root}
      className={`engine-frame engine-frame--${state}${interactive ? " engine-frame--interactive" : ""}`}
    >
      <img
        className="engine-poster"
        src={poster.src}
        width={poster.width}
        height={poster.height}
        alt=""
        decoding="async"
        loading="lazy"
      />
      {/* The host puts the page's one canvas here while this frame holds it. */}
      <div className="engine-stage" ref={stageRef} />
      {props.children !== undefined && <div className="engine-overlay">{props.children}</div>}
      {!live && (
        <div className="engine-panel">
          {state === "poster" && (
            <button type="button" className="ink-btn ink-btn--blue" onClick={() => void boot()}>
              {action}
            </button>
          )}
          {state === "away" && (
            <>
              <p className="engine-note">{away}</p>
              <button type="button" className="ink-btn ink-btn--blue" onClick={() => void boot()}>
                Bring it back
              </button>
            </>
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
