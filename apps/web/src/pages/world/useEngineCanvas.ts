import { useCallback, useEffect, useRef } from "react";
import { useCanvasEngine } from "@/engine/bevy/useCanvasEngine";
import { startCanvasKeyboardRouting } from "@/engine/canvasKeyboard";

interface UseEngineCanvasParams {
  id: string;
  canvasContainerId: string;
  playView: "staging" | "playing";
}

/**
 * The Bevy engine's mount and the body-level canvas it draws into.
 *
 * Moved out of `WorldPage` unchanged: mounting the engine, showing, sizing and
 * hiding the canvas Bevy inserts under <body>, and routing the keyboard to it.
 */
export function useEngineCanvas({
  id,
  canvasContainerId,
  playView,
}: UseEngineCanvasParams) {
  // Spec 009: stabilized with useCallback — useCanvasEngine's mount effect
  // depends on this callback's identity (see useCanvasEngine.ts), so a
  // fresh inline function here re-triggers mountEngine() on every render.
  // That was always latent, but WorldPage.tsx didn't previously have a
  // render-triggering subscription (useWorldRole -> useWorldMembers, added
  // in this spec) frequent enough to actually surface it as an infinite
  // "Maximum update depth exceeded" loop.
  const handleEngineError = useCallback((err: Error) => {
    console.error("Bevy engine failed to mount:", err);
  }, []);

  // 🎮 Phase 4.7.F1: Use canvas engine hook for responsive sizing
  const {
    containerRef,
    engineReady,
    loadProgress,
    error: engineError,
    retry: retryEngine,
    stopped: engineStopped,
    sizeCeiling: boardSizeCeiling,
  } = useCanvasEngine({
    worldId: id,
    canvasSelector: `#${canvasContainerId}`,
    onError: handleEngineError,
  });

  // Spec 009: Bevy/winit inserts the real <canvas> as a direct child of
  // <body> (verified via live inspection), not as a descendant of
  // `#game-canvas-container` — the container div only reserves layout
  // space for it, it never actually contains it. Two consequences this
  // effect handles directly, since neither can be solved with CSS on our
  // own React tree:
  // 1. `display:none` on any wrapper around the container never hides the
  //    real canvas — it must be toggled on the canvas element itself.
  // 2. With no positioning of its own, the canvas sits in normal document
  //    flow as the last child of <body> — i.e. *after* the full-viewport
  //    `WorldLayout`/`WorldStagingPage` wrappers that come before it in the
  //    DOM, landing one viewport-height below where it's meant to render.
  //    `position: fixed; inset: 0` removes it from that flow entirely so
  //    it always fills the viewport regardless of surrounding page content.
  // A plain effect keyed on [playView] can miss the exact moment the
  // canvas is inserted (e.g. right after a reload, mid-WASM-boot) and
  // then never re-fire since nothing else changes — a MutationObserver on
  // <body>, plus applying playViewRef's current value whenever a canvas
  // shows up, catches it regardless of timing.
  const playViewRef = useRef(playView);
  // Synced in an effect rather than assigned during render: a ref write
  // while rendering is exactly what `react-hooks/refs` warns about under a
  // render React may discard. The initial value is already in the ref, and
  // this effect is declared above both readers below, so it commits before
  // either of them can observe a stale value.
  useEffect(() => {
    playViewRef.current = playView;
  }, [playView]);
  // The canvas's largest allowed CSS size, same ref pattern: the styling
  // callback runs from a MutationObserver, outside React's render.
  const boardSizeCeilingRef = useRef(boardSizeCeiling);
  useEffect(() => {
    boardSizeCeilingRef.current = boardSizeCeiling;
  }, [boardSizeCeiling]);

  const applyCanvasVisibility = useCallback((canvas: HTMLCanvasElement) => {
    if (playViewRef.current === "playing") {
      canvas.style.display = "";
      canvas.style.position = "fixed";
      canvas.style.inset = "0";
      // A surface wider than the device's largest texture does not draw, it
      // panics (`Surface::configure`, seen on a 2048px device in a 2560px
      // window). The windowing layer sizes the backing store from this
      // element's CSS box times the pixel ratio, so the cap has to be here,
      // on the canvas itself, not on the container that never holds it.
      // `inset: 0` with `margin: auto` centres the smaller box.
      const ceiling = boardSizeCeilingRef.current;
      canvas.style.maxWidth = ceiling === null ? "" : `${ceiling}px`;
      canvas.style.maxHeight = ceiling === null ? "" : `${ceiling}px`;
      canvas.style.margin = ceiling === null ? "" : "auto";
    } else {
      canvas.style.display = "none";
    }
  }, []);

  /** Hides the body-level Bevy canvas without tearing the engine down. */
  const hideEngineCanvas = useCallback(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (canvas) {
      canvas.style.display = "none";
    }
  }, []);

  useEffect(() => {
    const existing = document.querySelector<HTMLCanvasElement>("canvas");
    if (existing) {
      applyCanvasVisibility(existing);
    }

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        mutation.addedNodes.forEach((node) => {
          if (node instanceof HTMLCanvasElement) {
            applyCanvasVisibility(node);
          }
        });
      }
    });
    observer.observe(document.body, { childList: true });

    return () => {
      observer.disconnect();
      // Bug fix: leaving Play (e.g. "Back to setup") unmounts this page,
      // but the canvas is a child of <body> with `position: fixed; inset: 0`
      // — it is not inside this component's tree, so React unmounting does
      // nothing to it and it stayed pinned over the whole viewport,
      // covering the staging page underneath. `playView` never leaves
      // "playing" (see its declaration), so the visibility effect below
      // could never hide it either: nothing in the component's own state
      // changes on navigation away. Hiding it here, in the one callback
      // that reliably runs on unmount, is what actually tears the canvas
      // down visually. The engine itself deliberately stays alive and
      // booted — spec 009's research.md §1 found invalidating its canvas
      // handle load-bearing not to do — so this hides, never destroys, and
      // the mount path above re-shows it on the way back in.
      hideEngineCanvas();
    };
  }, [applyCanvasVisibility, hideEngineCanvas]);

  useEffect(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (canvas) {
      applyCanvasVisibility(canvas);
    }
  }, [playView, engineReady, boardSizeCeiling, applyCanvasVisibility]);

  // Keyboard input is routed to the canvas from the window rather than
  // relying on the canvas holding focus — clicking any control in the dock
  // or the tool rail used to stop keyboard movement dead until the map was
  // clicked again. See `engine/canvasKeyboard.ts` for why winit makes this
  // necessary.
  useEffect(() => startCanvasKeyboardRouting(), []);

  return {
    containerRef,
    engineReady,
    loadProgress,
    engineError,
    retryEngine,
    engineStopped,
  };
}
