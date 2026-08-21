import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getAllWorlds, getMyWorlds } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Checkbox } from "@/components/ui/checkbox";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import { WorldCard } from "./components/WorldCard";

export const worldListPageSeo: SeoConfig = {
  title: "World archive",
  description:
    "Browse your ThunderForge worlds, create new ones, and inspect the full archive from a clean dashboard.",
  canonicalPath: "/worlds",
  noindex: true,
};

export default function WorldListPage() {
  const { isAdmin } = useAuth();
  const [includeAll, setIncludeAll] = useState(false);
  const scopeKey = isAdmin && includeAll ? "all" : "mine";
  const [archiveState, setArchiveState] = useState<{
    requestedScope: string;
    worlds: WorldRecord[];
    status: string | null;
    isLoading: boolean;
  }>({
    requestedScope: scopeKey,
    worlds: [],
    status: null,
    isLoading: true,
  });

  useEffect(() => {
    let active = true;
    const request = isAdmin && includeAll ? getAllWorlds() : getMyWorlds();
    void request
      .then((response) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            worlds: response,
            status: null,
            isLoading: false,
          });
        }
      })
      .catch((error) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            worlds: [],
            status:
              error instanceof Error
                ? error.message
                : "Failed to load world archive.",
            isLoading: false,
          });
        }
      });

    return () => {
      active = false;
    };
  }, [includeAll, isAdmin, scopeKey]);

  const isLoading =
    archiveState.requestedScope !== scopeKey || archiveState.isLoading;
  const worlds =
    archiveState.requestedScope === scopeKey ? archiveState.worlds : [];
  const status =
    archiveState.requestedScope === scopeKey ? archiveState.status : null;

  return (
    <>
      <SEO {...worldListPageSeo} />
      <Container>
        <main className="grid gap-6 pb-16">
          <section className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                World archive
              </p>
              <h1 className="text-3xl font-semibold">
                Every world in your library.
              </h1>
              <p className="mt-1 max-w-2xl text-muted-foreground">
                Create a new world, revisit an existing one, or inspect the
                wider archive when administrator privileges permit it.
              </p>
            </div>
            <div className="grid gap-3">
              <Link to="/worlds/create">
                <Button icon="quill">Create world</Button>
              </Link>
              {isAdmin ? (
                <label className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Checkbox
                    checked={includeAll}
                    onCheckedChange={(checked) =>
                      setIncludeAll(checked === true)
                    }
                  />
                  <span>Show every world in the archive</span>
                </label>
              ) : null}
            </div>
          </section>

          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {isLoading ? (
            <Loader label="Opening world archive" />
          ) : worlds.length > 0 ? (
            <section className="grid grid-cols-[repeat(auto-fit,minmax(280px,1fr))] gap-6">
              {worlds.map((world) => (
                <WorldCard
                  key={world.id}
                  world={world}
                  showOwner={Boolean(includeAll && isAdmin)}
                />
              ))}
            </section>
          ) : (
            <Card
              surface="leather"
              className="grid gap-3 p-8 text-center"
            >
              <h2 className="text-xl font-semibold">
                No worlds have been created yet.
              </h2>
              <p className="text-muted-foreground">
                Begin with a fresh world and ThunderForge will carry its
                scenes, actors, events, and interface hooks through later
                phases.
              </p>
              <Link to="/worlds/create" className="mx-auto">
                <Button variant="secondary" icon="worlds">
                  Create the first world
                </Button>
              </Link>
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}
