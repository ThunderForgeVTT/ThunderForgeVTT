import { useEffect } from "react";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { useSetupStatus } from "./hooks/useSetupStatus";
import AppRoutes from "./routes/AppRoutes";
import { schedulePagePrefetch } from "./routes/pageLoaders";

export default function App() {
  const { setupStatus, isLoading, error, refreshSetupStatus } = useSetupStatus();

  useEffect(() => {
    if (!setupStatus) {
      return;
    }

    schedulePagePrefetch(
      setupStatus.setup_required
        ? ["setup", "setupCallback", "counter"]
        : ["login", "signup", "counter"],
    );
  }, [setupStatus]);

  if (isLoading || !setupStatus) {
    if (isLoading) {
      return <Loader fullScreen label="Checking instance setup" />;
    }

    return (
      <main>
        <SEO
          title="Connection issue | ThunderForge VTT"
          description="ThunderForge could not reach the setup service."
          noindex
        />
        <Container>
          <Card style={{ marginBlock: "6rem" }}>
            <h1>ThunderForge could not load the current instance state.</h1>
            <p>
              {error ??
                "The setup service did not respond. Confirm the server is running and try again."}
            </p>
            <Button onClick={() => void refreshSetupStatus()}>Retry</Button>
          </Card>
        </Container>
      </main>
    );
  }

  return (
    <>
      <SEO
        title="ThunderForge VTT"
        description="Persistent worlds, secure first-run setup, and collaborative tabletop play in a fast React and Bevy experience."
        keywords={[
          "virtual tabletop",
          "React Vite",
          "Bevy",
          "tldraw",
          "ThunderForge",
        ]}
        preloadAssets={[
          {
            href: "/brand-mark.svg",
            as: "image",
            type: "image/svg+xml",
          },
        ]}
        prefetchHrefs={setupStatus.setup_required ? ["/setup"] : ["/login", "/signup"]}
      />
      <AppRoutes
        setupStatus={setupStatus}
        onSetupStatusRefresh={refreshSetupStatus}
      />
    </>
  );
}
