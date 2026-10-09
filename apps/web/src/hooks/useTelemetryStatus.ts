import { useCallback, useEffect, useState } from "react";
import { fetchTelemetryStatus, type TelemetryStatus } from "@/api/telemetry";

export interface UseTelemetryStatusResult {
  status: TelemetryStatus | null;
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

/**
 * Spec 086 FR-035.4: the admin panel's read. A plain fetch, not the world
 * store: telemetry is instance configuration, fixed until the next restart.
 */
export function useTelemetryStatus(): UseTelemetryStatusResult {
  const [status, setStatus] = useState<TelemetryStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const refetch = useCallback(async () => {
    setLoading(true);
    try {
      setStatus(await fetchTelemetryStatus());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  // The mount read sets state only from the promise, never synchronously
  // (react-hooks/set-state-in-effect); `loading` starts true for it.
  useEffect(() => {
    let active = true;
    fetchTelemetryStatus()
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((e: unknown) => {
        if (active) setError(e instanceof Error ? e : new Error(String(e)));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  return { status, loading, error, refetch };
}
