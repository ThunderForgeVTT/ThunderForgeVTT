import { useEffect, useRef, useState } from 'react';
import type { EngineMountOptions, EngineState } from './types';
import { mountEngine } from './index';

interface UseCanvasEngineOptions {
  worldId: string;
  canvasSelector: string;
  onError?: (error: Error) => void;
}

interface UseCanvasEngineResult {
  containerRef: React.RefObject<HTMLDivElement>;
  engine: any; // Engine WASM instance for calling exported functions
  engineReady: boolean;
  error: Error | null;
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
  options: UseCanvasEngineOptions
): UseCanvasEngineResult {
  const containerRef = useRef<HTMLDivElement>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const engineRef = useRef<any>(null);
  const [engineReady, setEngineReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // 🎮 Mount Bevy engine on component mount
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      console.warn('useCanvasEngine: container ref not set');
      return;
    }

    const mountAsync = async () => {
      try {
        const engine = await mountEngine({
          canvasSelector: options.canvasSelector,
          worldId: options.worldId,
        } as EngineMountOptions);

        engineRef.current = engine;
        setEngineReady(true);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        console.error('🎮 Failed to mount Bevy engine:', error);
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
  }, [options.canvasSelector, options.worldId, options.onError]);

  // 📤 Setup ResizeObserver to track container size changes
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      console.warn('useCanvasEngine: ResizeObserver setup failed - no container ref');
      return;
    }

    // Wait for canvas to be created by Bevy
    const checkCanvasInterval = setInterval(() => {
      const canvas = container.querySelector('canvas') as HTMLCanvasElement | null;

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
      console.warn('useCanvasEngine: Canvas not found after 5 seconds');
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
    error,
  };
}
