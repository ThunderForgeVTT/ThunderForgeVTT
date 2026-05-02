import { useEffect, useState } from "react";
import { SetupStatus, getSetupStatus } from "../api/auth";

const FALLBACK_SETUP_STATUS: SetupStatus = {
  setup_required: false,
  setup_completed: true,
  configured_oauth_providers: [],
};

export function useSetupStatus() {
  const [setupStatus, setSetupStatus] = useState<SetupStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const refreshSetupStatus = async () => {
    const nextStatus = await getSetupStatus();
    setSetupStatus(nextStatus);
    return nextStatus;
  };

  useEffect(() => {
    let isActive = true;

    void getSetupStatus()
      .then((status) => {
        if (!isActive) {
          return;
        }

        setSetupStatus(status);
      })
      .catch(() => {
        if (!isActive) {
          return;
        }

        setSetupStatus(FALLBACK_SETUP_STATUS);
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
    refreshSetupStatus,
  };
}
