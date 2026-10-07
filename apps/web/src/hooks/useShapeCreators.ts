/**
 * useShapeCreators.ts — spec 082 US4: who drew on a scene, for the Game
 * Master's "Clear a player's shapes…" picker.
 *
 * Usage:
 *   const { creators, loading, error, refetch } = useShapeCreators(sceneId, open);
 *
 * A plain fetch, not store state: it is asked when the picker opens
 * (`enabled`), and again through `refetch()`.
 */

import { useCallback, useEffect, useState } from "react";

import { getShapeCreators } from "@/api/shapes";
import type { ShapeCreator } from "@/types/shape";

export interface UseShapeCreatorsResult {
  creators: ShapeCreator[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export function useShapeCreators(
  sceneId: string,
  enabled: boolean,
): UseShapeCreatorsResult {
  const [creators, setCreators] = useState<ShapeCreator[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const refetch = useCallback(async () => {
    setLoading(true);
    try {
      setCreators(await getShapeCreators(sceneId));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [sceneId]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    getShapeCreators(sceneId).then(
      (answer) => {
        if (cancelled) return;
        setCreators(answer);
        setError(null);
        setLoading(false);
      },
      (err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err : new Error(String(err)));
        setLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [sceneId, enabled]);

  return { creators, loading, error, refetch };
}
