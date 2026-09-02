import { useCallback, useEffect, useRef, useState } from "react";
import type { EngineMountOptions } from "./types";
import {
  mountEngine,
  type EngineLoadProgress,
  type EngineLoadStage,
} from "./index";

interface UseCanvasEngineOptions {
  worldId: string;
  canvasSelector: string;
  onError?: (error: Error) => void;
}

interface UseCanvasEngineResult {
  /*
    `| null` because that is what `useRef<HTMLDivElement>(null)` returns under
    React 19: the ref is empty until the element mounts, and the types now say
    so. Consumers already handle it — every read here is guarded — so this
    declaration was simply describing an older React.
  */
  containerRef: React.RefObject<HTMLDivElement | null>;
  engine: any; // Engine WASM instance for calling exported functions
  engineReady: boolean;
  /** Spec 008 (US1, FR-002): "downloading" until engineReady flips true —
   * never null before that point, so callers can always show status text
   * instead of a silent gap. */
  loadStage: EngineLoadStage;
  /**
   * Spec 028 (US6, FR-030): real bytes, or `null` before the first chunk
   * arrives. Never a synthesised value — callers render an indeterminate
   * state rather than a made-up percentage.
   */
  loadProgress: EngineLoadProgress | null;
  error: Error | null;
  /**
   * FR-032: retry after a failed download or start. Clears the error and
   * remounts, so a transient network failure does not require a page reload.
   */
  retry: () => void;
}

/**
 * Phase 4.7.F1: Canvas Size Synchronization
 *
 * Hook that mounts Bevy engine and synchronizes canvas size with container.
 * Uses ResizeObserver to detect container size changes and updates canvas dimensions.
 *
 * Usage:
 * ```tsx
 * const { containerRef } = useCanvasEngine({
 *   worldId: '123',
 *   canvasSelector: '#game-canvas',
 * });
 *
 * return <div ref={containerRef} style={{ width: '100%', height: '100%' }} />;
 * ```
 */
export function useCanvasEngine(
  options: UseCanvasEngineOptions,
): UseCanvasEngineResult {
  const containerRef = useRef<HTMLDivElement>(null);
  const contextMenuCleanupRef = useRef<(() => void) | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  // The mounted engine is state, not a ref: it is returned to the caller and
  // therefore read during render, which `react-hooks/refs` rightly rejects
  // for a ref. It is set in the same tick as `engineReady` below, so nothing
  // about when callers see it changes.
  const [engine, setEngine] = useState<any>(null);
  const [engineReady, setEngineReady] = useState(false);
  const [loadStage, setLoadStage] = useState<EngineLoadStage>("downloading");
  const [loadProgress, setLoadProgress] = useState<EngineLoadProgress | null>(
    null,
  );
  const [error, setError] = useState<Error | null>(null);
  const [attempt, setAttempt] = useState(0);

  const retry = useCallback(() => {
    setError(null);
    setLoadProgress(null);
    setLoadStage("downloading");
    setAttempt((n) => n + 1);
  }, []);

  // 🎮 Mount Bevy engine on component mount
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      console.warn("useCanvasEngine: container ref not set");
      return;
    }

    const mountAsync = async () => {
      try {
        const mounted = await mountEngine(
          {
            canvasSelector: options.canvasSelector,
            worldId: options.worldId,
          } as EngineMountOptions,
          setLoadStage,
          setLoadProgress,
        );

        setEngine(mounted);
        setEngineReady(true);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        console.error("🎮 Failed to mount Bevy engine:", error);
        setError(error);
        options.onError?.(error);
      }
    };

    mountAsync();

    // Cleanup if needed
    return () => {
      // Engine persists across component unmounts
      // Cleanup on unmountEngine() call if needed
    };
    // `attempt` is in the dependency list so `retry()` re-runs this effect.
  }, [options.canvasSelector, options.worldId, options.onError, attempt]);

  // 📤 Setup ResizeObserver to track container size changes
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      console.warn(
        "useCanvasEngine: ResizeObserver setup failed - no container ref",
      );
      return;
    }

    // Wait for canvas to be created by Bevy
    const checkCanvasInterval = setInterval(() => {
      const canvas = container.querySelector(
        "canvas",
      ) as HTMLCanvasElement | null;

      if (canvas && canvas.clientWidth > 0) {
        clearInterval(checkCanvasInterval);

        /**
         * The map owns right-click; the rest of the app does not.
         *
         * Spec 031 FR-029 wants right-click available as a canvas gesture,
         * which means the browser's own context menu must not open on top of
         * it. Bound to the canvas element itself and nowhere else: panels,
         * lists and editors keep their normal menus, because taking those away
         * would cost a user text selection and inspection for no benefit.
         *
         * It has to be bound here rather than in JSX because the element is
         * not React's. Bevy/winit inserts the real `<canvas>` itself — which is
         * the same fact that made text placement silently fail when a listener
         * was attached to the React container instead.
         */
        const suppressContextMenu = (event: MouseEvent) => {
          event.preventDefault();
        };
        canvas.addEventListener("contextmenu", suppressContextMenu);
        contextMenuCleanupRef.current = () => {
          canvas.removeEventListener("contextmenu", suppressContextMenu);
        };

        // Canvas found, setup ResizeObserver
        resizeObserverRef.current = new ResizeObserver(() => {
          if (!canvas) return;

          const rect = container.getBoundingClientRect();

          // 🔔 Update canvas size to match container
          if (canvas.width !== rect.width || canvas.height !== rect.height) {
            canvas.width = rect.width;
            canvas.height = rect.height;
            // console.debug(`📡 Canvas resized: ${rect.width}x${rect.height}`);
          }
        });

        // Start observing container
        resizeObserverRef.current.observe(container);
      }
    }, 100);

    // Timeout: stop looking for canvas after 5 seconds
    const timeoutId = setTimeout(() => {
      clearInterval(checkCanvasInterval);
      console.warn("useCanvasEngine: Canvas not found after 5 seconds");
    }, 5000);

    // Cleanup
    return () => {
      clearInterval(checkCanvasInterval);
      contextMenuCleanupRef.current?.();
      contextMenuCleanupRef.current = null;
      clearTimeout(timeoutId);
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
    };
  }, []);

  return {
    containerRef,
    engine,
    engineReady,
    loadStage,
    loadProgress,
    error,
    retry,
  };
}
