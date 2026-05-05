import { useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import type { SeoConfig } from "@/types/seo";

export const setupCallbackPageSeo: SeoConfig = {
  title: "Finishing setup",
  description: "Completing the ThunderForge VTT bootstrap callback.",
  canonicalPath: "/setup/callback",
  noindex: true,
};

interface SetupCallbackPageProps {
  onSetupComplete: () => Promise<unknown>;
}

export default function SetupCallbackPage({
  onSetupComplete,
}: SetupCallbackPageProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  useEffect(() => {
    const oauthError = searchParams.get("oauth_error");
    if (oauthError) {
      navigate(`/setup?oauth_error=${encodeURIComponent(oauthError)}`, {
        replace: true,
      });
      return;
    }

    if (searchParams.get("oauth") === "success") {
      void onSetupComplete()
        .then(() => {
          navigate("/admin?bootstrap=complete", { replace: true });
        })
        .catch((error) => {
          const message =
            error instanceof Error ? error.message : "OAuth setup failed.";
          navigate(`/setup?oauth_error=${encodeURIComponent(message)}`, {
            replace: true,
          });
        });
      return;
    }

    navigate("/setup", { replace: true });
  }, [navigate, onSetupComplete, searchParams]);

  return (
    <>
      <SEO {...setupCallbackPageSeo} />
      <Loader fullScreen label="Finishing OAuth setup" />
    </>
  );
}
