import { useCallback, useEffect, useState } from "react";
import { getSetupStatus } from "@/services/auth";
import type { SetupStatus } from "@/types/auth";

export function useSetupStatus() {
  const [setupStatus, setSetupStatus] = useState<SetupStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshSetupStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const nextStatus = await getSetupStatus();
      setSetupStatus(nextStatus);
      return nextStatus;
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load setup status.",
      );
      throw nextError;
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    let isActive = true;

    void getSetupStatus()
      .then((status) => {
        if (!isActive) {
          return;
        }

        setSetupStatus(status);
        setError(null);
      })
      .catch((nextError) => {
        if (!isActive) {
          return;
        }

        setError(
          nextError instanceof Error
            ? nextError.message
            : "Failed to load setup status.",
        );
      })
      .finally(() => {
        if (isActive) {
          setIsLoading(false);
        }
      });

    return () => {
      isActive = false;
    };
  }, []);

  return {
    setupStatus,
    isLoading,
    error,
    refreshSetupStatus,
  };
}
