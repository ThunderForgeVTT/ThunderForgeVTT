import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { deleteWorld, getWorld } from "@/api/world";
import { getScenes } from "@/api/scenes";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { CampaignSettingsPanel } from "@/components/campaign/CampaignSettingsPanel";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";

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
  // T015 (US2): the dashboard's old "Scenes" panel read from `world.scenes`,
  // a GraphQLWorld field that's permanently hardcoded to an empty array at
  // the resolver (never real data). Real scene data lives behind the
  // separate `scenes(worldId)` query WorldPage/SceneSwitcher already use —
  // reusing that here instead (research.md's correction to its original
  // plan).
  const [scenes, setScenes] = useState<SceneRecord[] | null>(null);

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

  useEffect(() => {
    let active = true;
    void getScenes(id)
      .then((result) => {
        if (active) {
          setScenes(result);
        }
      })
      .catch(() => {
        if (active) {
          setScenes([]);
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

              <section className="grid gap-4">
                <Card surface="stone" className="grid gap-3 p-5">
                  <div className="flex items-center gap-3">
                    <span className="inline-grid size-9 shrink-0 place-items-center rounded-full border border-border bg-secondary">
                      <FantasyIcon name="map" size={16} />
                    </span>
                    <div>
                      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                        Scenes
                      </p>
                      <h3 className="text-lg font-semibold">
                        {scenes === null
                          ? "Loading…"
                          : `${scenes.length} scene${scenes.length === 1 ? "" : "s"}`}
                      </h3>
                    </div>
                  </div>
                  {scenes && scenes.length > 0 ? (
                    <ul className="grid list-inside list-disc gap-1 text-sm text-muted-foreground">
                      {scenes.map((scene) => (
                        <li key={scene.sceneId}>{scene.name}</li>
                      ))}
                    </ul>
                  ) : null}
                </Card>
              </section>

              <CampaignSettingsPanel worldId={world.id} />
            </>
          )}
        </main>
      </Container>
    </>
  );
}
