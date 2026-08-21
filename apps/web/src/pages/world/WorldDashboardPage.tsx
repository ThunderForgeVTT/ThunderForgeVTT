import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { deleteWorld, getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { CampaignSettingsPanel } from "@/components/campaign/CampaignSettingsPanel";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import { WorldPlaceholderPanel } from "./components/WorldPlaceholderPanel";

const PLACEHOLDER_COPY = {
  scenes:
    "Scene persistence lands after the world shell is in place, so this panel stands ready for maps and chapter boards.",
  actors:
    "Actor sheets, party rosters, and NPC ledgers will mount here once Phase 4 expands the domain model.",
  tokens:
    "Token orchestration exists elsewhere in the engine, but the world dashboard keeps this placeholder until world-bound management is wired in.",
  events:
    "Event audit trails and world timelines will surface here once world history graduates from placeholder status.",
  gameSystem:
    "The selected placeholder ID is stored now, while full game system metadata remains intentionally deferred.",
  interfacePack:
    "The interface pack ID is reserved now so Phase 3.5 can slot in a real selector without reshaping world creation.",
} as const;

function formatTimestamp(value: string) {
  return new Date(value).toLocaleString();
}

export const worldDashboardPageSeo: SeoConfig = {
  title: "World dashboard",
  description:
    "Inspect world metadata, ownership, and future gameplay domains from the ThunderForge world dashboard.",
  canonicalPath: "/world",
  noindex: true,
};

export default function WorldDashboardPage() {
  const navigate = useNavigate();
  const { id = "" } = useParams();
  const { user } = useAuth();
  const [worldState, setWorldState] = useState<{
    requestedId: string;
    world: WorldRecord | null;
    status: string | null;
    isLoading: boolean;
  }>({
    requestedId: id,
    world: null,
    status: null,
    isLoading: true,
  });
  const [isDeleting, setIsDeleting] = useState(false);

  useEffect(() => {
    let active = true;
    void getWorld(id)
      .then((response) => {
        if (active) {
          setWorldState({
            requestedId: id,
            world: response,
            status: null,
            isLoading: false,
          });
        }
      })
      .catch((error) => {
        if (active) {
          setWorldState({
            requestedId: id,
            world: null,
            status:
              error instanceof Error ? error.message : "Failed to load world.",
            isLoading: false,
          });
        }
      });

    return () => {
      active = false;
    };
  }, [id]);

  const isLoading = worldState.requestedId !== id || worldState.isLoading;
  const world = worldState.requestedId === id ? worldState.world : null;
  const status = worldState.requestedId === id ? worldState.status : null;

  const handleDelete = async () => {
    if (!world || isDeleting) {
      return;
    }

    if (!window.confirm(`Delete '${world.name}' and its related records?`)) {
      return;
    }

    setIsDeleting(true);
    setWorldState((current) => ({
      ...current,
      status: null,
    }));

    try {
      await deleteWorld(world.id);
      void navigate("/worlds");
    } catch (error) {
      setWorldState((current) => ({
        ...current,
        status: error instanceof Error ? error.message : "Failed to delete world.",
      }));
      setIsDeleting(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Opening world dashboard" />;
  }

  return (
    <>
      <SEO
        {...worldDashboardPageSeo}
        title={world ? `${world.name} dashboard` : worldDashboardPageSeo.title}
        canonicalPath={
          id ? `/world/${id}` : worldDashboardPageSeo.canonicalPath
        }
      />
      <Container>
        <main className="grid gap-8 pb-16">
          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {!world ? (
            <Card surface="stone" className="grid gap-3 p-8 text-center">
              <h1 className="text-2xl font-semibold">
                This world could not be opened.
              </h1>
              <p className="text-muted-foreground">
                The world may be missing, or your access does not permit
                viewing this dashboard.
              </p>
              <Button asChild variant="secondary" icon="arrow-left" className="mx-auto">
                <Link to="/worlds">Return to archive</Link>
              </Button>
            </Card>
          ) : (
            <>
              <section className="flex flex-wrap items-start justify-between gap-4">
                <div>
                  <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                    World dashboard
                  </p>
                  <h1 className="text-3xl font-semibold">{world.name}</h1>
                  <p className="mt-1 max-w-2xl text-muted-foreground">
                    {world.description ??
                      "The world has been created. Its scenes, actors, tokens, and events will gather here in later phases."}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button asChild icon="worlds">
                    <Link to={`/world/${world.id}/play`}>Enter world</Link>
                  </Button>
                  <Button variant="secondary" icon="settings" disabled>
                    Manage settings
                  </Button>
                  <Button
                    variant="danger"
                    icon="skull"
                    onClick={() => void handleDelete()}
                    disabled={isDeleting}
                  >
                    {isDeleting ? "Deleting..." : "Delete world"}
                  </Button>
                </div>
              </section>

              {world.createdBy !== user?.id ? (
                <StatusBadge variant="warning">
                  You are viewing this world through administrator access.
                </StatusBadge>
              ) : null}

              <section className="grid gap-6 md:grid-cols-2">
                <Card surface="parchment" className="grid gap-4 p-6">
                  <h2 className="text-xl font-semibold">World metadata</h2>
                  <dl className="grid grid-cols-2 gap-4 text-sm">
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Game system ID
                      </dt>
                      <dd className="font-medium">
                        {world.gameSystemId ?? "Not yet assigned"}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Interface pack ID
                      </dt>
                      <dd className="font-medium">
                        {world.interfacePackId ?? "Not yet assigned"}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Created by
                      </dt>
                      <dd className="font-medium">{world.createdBy}</dd>
                    </div>
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Updated by
                      </dt>
                      <dd className="font-medium">{world.updatedBy}</dd>
                    </div>
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Created at
                      </dt>
                      <dd className="font-medium">
                        {formatTimestamp(world.createdAt)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-xs text-muted-foreground">
                        Updated at
                      </dt>
                      <dd className="font-medium">
                        {formatTimestamp(world.updatedAt)}
                      </dd>
                    </div>
                  </dl>
                </Card>

                <Card surface="leather" className="grid gap-4 p-6">
                  <h2 className="text-xl font-semibold">Quick actions</h2>
                  <p className="text-sm text-muted-foreground">
                    The dashboard remains the ownership-safe control room for
                    the world while deeper management flows come online.
                  </p>
                  <div className="grid gap-2 text-sm">
                    <Link
                      to={`/world/${world.id}/play`}
                      className="text-primary underline-offset-4 hover:underline"
                    >
                      Enter the live workspace
                    </Link>
                    <Link
                      to="/worlds/create"
                      className="text-primary underline-offset-4 hover:underline"
                    >
                      Create another world
                    </Link>
                    <button
                      type="button"
                      onClick={() => void handleDelete()}
                      className="text-left text-destructive underline-offset-4 hover:underline"
                    >
                      Permanently delete this world
                    </button>
                  </div>
                </Card>
              </section>

              <section className="grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-4">
                <WorldPlaceholderPanel
                  title="Scenes"
                  icon="map"
                  copy={PLACEHOLDER_COPY.scenes}
                  items={world.scenes}
                  surface="stone"
                />
                <WorldPlaceholderPanel
                  title="Actors"
                  icon="actors"
                  copy={PLACEHOLDER_COPY.actors}
                  items={world.actors}
                  surface="stone"
                />
                <WorldPlaceholderPanel
                  title="Tokens"
                  icon="tokens"
                  copy={PLACEHOLDER_COPY.tokens}
                  items={world.tokens}
                  surface="stone"
                />
                <WorldPlaceholderPanel
                  title="Events"
                  icon="spark"
                  copy={PLACEHOLDER_COPY.events}
                  items={world.events}
                  surface="stone"
                />
                <WorldPlaceholderPanel
                  title="Game system"
                  icon="wand"
                  copy={PLACEHOLDER_COPY.gameSystem}
                  items={world.gameSystem ? [world.gameSystem] : []}
                  surface="leather"
                />
                <WorldPlaceholderPanel
                  title="Interface pack"
                  icon="rune"
                  copy={PLACEHOLDER_COPY.interfacePack}
                  items={world.interfacePack ? [world.interfacePack] : []}
                  surface="leather"
                />
              </section>

              <CampaignSettingsPanel worldId={world.id} />
            </>
          )}
        </main>
      </Container>
    </>
  );
}
