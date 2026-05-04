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
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import { WorldPlaceholderPanel } from "./components/WorldPlaceholderPanel";
import styles from "./WorldDashboardPage.module.scss";

const PLACEHOLDER_COPY = {
  scenes:
    "Scene persistence lands after the world shell is in place, so this panel stands ready for maps and chapter boards.",
  actors:
    "Actor sheets, party rosters, and NPC ledgers will mount here once Phase 4 expands the domain model.",
  tokens:
    "Token orchestration exists elsewhere in the engine, but the world dashboard keeps this placeholder until world-bound management is wired in.",
  events:
    "Event audit trails and realm timelines will surface here once world history graduates from placeholder status.",
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
        <main className={styles.shell}>
          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {!world ? (
            <Card surface="stone" className={styles.emptyState}>
              <h1>This world could not be opened.</h1>
              <p>
                The realm may be missing, or your stewardship does not permit
                access to this dashboard.
              </p>
              <Button asChild variant="secondary" icon="arrow-left">
                <Link to="/worlds">Return to archive</Link>
              </Button>
            </Card>
          ) : (
            <>
              <section className={styles.hero}>
                <div>
                  <p className={styles.eyebrow}>Guild hall table</p>
                  <h1>{world.name}</h1>
                  <p>
                    {world.description ??
                      "The realm has been founded. Its scenes, actors, tokens, and events will gather around this table in later phases."}
                  </p>
                </div>
                <div className={styles.heroActions}>
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

              <section className={styles.metadataGrid}>
                <Card surface="parchment" className={styles.metadataCard}>
                  <h2>World metadata</h2>
                  <dl className={styles.metadataList}>
                    <div>
                      <dt>Game system ID</dt>
                      <dd>{world.gameSystemId ?? "Not yet assigned"}</dd>
                    </div>
                    <div>
                      <dt>Interface pack ID</dt>
                      <dd>{world.interfacePackId ?? "Not yet assigned"}</dd>
                    </div>
                    <div>
                      <dt>Created by</dt>
                      <dd>{world.createdBy}</dd>
                    </div>
                    <div>
                      <dt>Updated by</dt>
                      <dd>{world.updatedBy}</dd>
                    </div>
                    <div>
                      <dt>Created at</dt>
                      <dd>{formatTimestamp(world.createdAt)}</dd>
                    </div>
                    <div>
                      <dt>Updated at</dt>
                      <dd>{formatTimestamp(world.updatedAt)}</dd>
                    </div>
                  </dl>
                </Card>

                <Card surface="leather" className={styles.metadataCard}>
                  <h2>Quick actions</h2>
                  <p className={styles.quickCopy}>
                    The dashboard remains the ownership-safe control room for
                    the realm while deeper management flows come online.
                  </p>
                  <div className={styles.quickList}>
                    <Link to={`/world/${world.id}/play`}>
                      Enter the live workspace
                    </Link>
                    <Link to="/worlds/create">Charter another world</Link>
                    <button type="button" onClick={() => void handleDelete()}>
                      Permanently delete this realm
                    </button>
                  </div>
                </Card>
              </section>

              <section className={styles.panelGrid}>
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
            </>
          )}
        </main>
      </Container>
    </>
  );
}
