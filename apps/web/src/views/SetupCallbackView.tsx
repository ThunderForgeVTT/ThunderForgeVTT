import React, { useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

interface SetupCallbackViewProps {
  onSetupComplete: () => Promise<void>;
}

export default function SetupCallbackView({ onSetupComplete }: SetupCallbackViewProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  useEffect(() => {
    const oauthError = searchParams.get("oauth_error");
    if (oauthError) {
      navigate(`/setup?oauth_error=${encodeURIComponent(oauthError)}`, { replace: true });
      return;
    }

    if (searchParams.get("oauth") === "success") {
      void onSetupComplete().then(() => {
        navigate("/counter?bootstrap=complete", { replace: true });
      });
      return;
    }

    navigate("/setup", { replace: true });
  }, [navigate, onSetupComplete, searchParams]);

  return <div className="app-loading">Finishing OAuth setup...</div>;
}
