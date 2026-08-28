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
  containerRef: React.RefObject<HTMLDivElement>;
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
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const engineRef = useRef<any>(null);
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
        const engine = await mountEngine(
          {
            canvasSelector: options.canvasSelector,
            worldId: options.worldId,
          } as EngineMountOptions,
          setLoadStage,
          setLoadProgress,
        );

        engineRef.current = engine;
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
      clearTimeout(timeoutId);
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
    };
  }, []);

  return {
    containerRef,
    engine: engineRef.current,
    engineReady,
    loadStage,
    loadProgress,
    error,
    retry,
  };
}
