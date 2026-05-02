import React, { Suspense } from "react";
import { useSetupStatus } from "./hooks/useSetupStatus";
import AppRoutes from "./routes/AppRoutes";

export default function App() {
  const { setupStatus, isLoading, refreshSetupStatus } = useSetupStatus();

  if (isLoading || !setupStatus) {
    return <div className="app-loading">Checking setup status...</div>;
  }

  return (
    <Suspense fallback={null}>
      <AppRoutes
        setupStatus={setupStatus}
        onSetupStatusRefresh={refreshSetupStatus}
      />
    </Suspense>
  );
}
