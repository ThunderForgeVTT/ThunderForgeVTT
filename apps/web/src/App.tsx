import { lazy, Suspense, useEffect } from "react";
import { useLocation } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { useSetupStatus } from "./hooks/useSetupStatus";
import AppRoutes from "./routes/AppRoutes";
import { pageLoaders, schedulePagePrefetch } from "./routes/pageLoaders";

const StatusPage = lazy(pageLoaders.status);

export default function App() {
  const location = useLocation();
  const { setupStatus, isLoading, error, refreshSetupStatus } =
    useSetupStatus();

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

  // The status page must work even when the setup/GraphQL service is
  // unreachable — that's precisely the situation it exists to diagnose —
  // so it's rendered ahead of any setupStatus loading/error gate below.
  if (location.pathname === "/status") {
    return (
      <Suspense fallback={<Loader fullScreen label="Loading system status" />}>
        <StatusPage />
      </Suspense>
    );
  }

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
          <Card
            style={{ marginBlock: "6rem" }}
            className="grid gap-3 p-6 text-center"
          >
            <h1 className="text-2xl font-semibold">
              ThunderForge could not load the current instance state.
            </h1>
            <p className="text-muted-foreground">
              {error ??
                "The setup service did not respond. Confirm the server is running and try again."}
            </p>
            <div className="flex flex-wrap justify-center gap-3">
              <Button onClick={() => void refreshSetupStatus()}>Retry</Button>
              <Button variant="secondary" asChild>
                <a href="/status">View system status</a>
              </Button>
            </div>
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
        keywords={["virtual tabletop", "React Vite", "Bevy", "ThunderForge"]}
        prefetchHrefs={
          setupStatus.setup_required ? ["/setup"] : ["/login", "/signup"]
        }
      />
      <AppRoutes
        setupStatus={setupStatus}
        onSetupStatusRefresh={refreshSetupStatus}
      />
    </>
  );
}
