import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { deleteWorld, getWorld } from "@/api/world";
import {
  interfacePackLabel,
  useInterfacePacks,
} from "@/appearance/interface-pack-label";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { PlayPausedBanner } from "@/components/world/PlayPausedBanner";
import { useAuth } from "@/hooks/useAuth";
import { IN_DEMO } from "@/lib/demoBuild";
import { useActorClaimGate } from "@/hooks/useActorClaimGate";
import { CampaignSettingsPanel } from "@/components/campaign/CampaignSettingsPanel";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import { WorldAtAGlance } from "@/pages/world/components/WorldAtAGlance";
import { WorldPlayersCard } from "@/pages/world/components/WorldPlayersCard";
import { reachedThroughAdminAccess } from "@/pages/world/adminAccess";
import { useWorldRole } from "@/hooks/useWorldRole";

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
  const { isAdmin } = useAuth();
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
  const packs = useInterfacePacks();

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

  // Spec 017 (FR-001): a non-GM member with no claimed character yet is
  // redirected to Actor Selection before this dashboard renders.
  const { cleared: claimGateCleared } = useActorClaimGate(id, world);
  const { role, isGm, loading: roleLoading } = useWorldRole(id, world);
  // Deleting ends the world for everyone, so it is the Owner's alone — the
  // same line `deleteWorld` draws on the server (`is_owner_of_world`). A Game
  // Master runs the table and is still not shown it.
  const ownsWorld = role === "Owner";

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
        status:
          error instanceof Error ? error.message : "Failed to delete world.",
      }));
      setIsDeleting(false);
    }
  };

  if (isLoading || (world && !claimGateCleared)) {
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
      {/* Spec 088 US4 (FR-033, FR-036, contracts/layouts.md): the page's own
          width, not `Container`'s. One column on a phone, two from `lg`,
          three from `2xl`, and capped at 1800 px so a wide screen is used
          without a line running its whole width. */}
      <div
        className="mx-auto w-full max-w-[1800px] px-4 py-6 sm:px-6 lg:py-10"
        data-testid="world-dashboard"
      >
        <main className="grid gap-8 pb-16">
          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {!world ? (
            <Card surface="stone" className="grid gap-3 p-8 text-center">
              <h1 className="text-2xl font-semibold">
                This world could not be opened.
              </h1>
              <p className="text-muted-foreground">
                The world may be missing, or your access does not permit viewing
                this dashboard.
              </p>
              <Button
                asChild
                variant="secondary"
                icon="arrow-left"
                className="mx-auto"
              >
                <Link to="/worlds">Return to archive</Link>
              </Button>
            </Card>
          ) : (
            <>
              <PlayPausedBanner worldId={world.id} />
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
                    <Link to={`/world/${world.id}/staging`}>Enter world</Link>
                  </Button>
                  {/* This was a permanently disabled placeholder, drawn for
                      every member: a Game Master pressing it got nothing, and a
                      Player was shown a control that was never theirs. It now
                      goes to the world's settings, for the people who run it. */}
                  {isGm ? (
                    <Button
                      asChild
                      variant="secondary"
                      icon="settings"
                      data-testid="world-manage-settings"
                    >
                      <Link to={`/world/${world.id}/settings/system`}>
                        Manage settings
                      </Link>
                    </Button>
                  ) : null}
                  {/* Spec 074 FR-013: the demo has the one world, kept in
                      this browser; deleting or adding worlds is an instance's
                      business. */}
                  {IN_DEMO || !ownsWorld ? null : (
                    <Button
                      data-testid="world-delete"
                      variant="danger"
                      icon="skull"
                      onClick={() => void handleDelete()}
                      disabled={isDeleting}
                    >
                      {isDeleting ? "Deleting..." : "Delete world"}
                    </Button>
                  )}
                </div>
              </section>

              {reachedThroughAdminAccess({
                isAdmin,
                role,
                roleLoading,
              }) ? (
                <StatusBadge
                  variant="warning"
                  data-testid="world-admin-access-warning"
                >
                  You are viewing this world through administrator access.
                </StatusBadge>
              ) : null}

              <div
                className="grid items-start gap-6 lg:grid-cols-2 2xl:grid-cols-3"
                data-testid="world-dashboard-grid"
              >
                {/* Figures first, and only the few scenes touched last. This
                    used to be a bullet per scene, which at forty scenes is the
                    page. See `WorldAtAGlance`. */}
                <div className="grid min-w-0 content-start gap-6 lg:row-span-2 2xl:row-span-1">
                  <WorldAtAGlance worldId={world.id} />
                </div>

                <div className="grid min-w-0 content-start gap-6">
                  <WorldPlayersCard worldId={world.id} runsWorld={isGm} />
                </div>

                <div className="grid min-w-0 content-start gap-6 lg:col-start-2 2xl:col-start-auto">
                  <section className="grid gap-6 md:grid-cols-2 lg:grid-cols-1">
                    <Card surface="parchment" className="grid gap-4 p-6">
                      <h2 className="text-xl font-semibold">World metadata</h2>
                      <dl className="grid grid-cols-2 gap-4 text-sm">
                        <div>
                          <dt className="text-xs text-muted-foreground">
                            Game system
                          </dt>
                          <dd className="font-medium">
                            {isGm ? (
                              <Link
                                to={`/world/${world.id}/settings/system`}
                                className="underline underline-offset-2 hover:text-primary"
                                data-testid="world-system-settings-link"
                              >
                                {world.gameSystemId ?? "Not yet assigned"} —
                                manage
                              </Link>
                            ) : (
                              (world.gameSystemId ?? "Not yet assigned")
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt className="text-xs text-muted-foreground">
                            Interface pack
                          </dt>
                          <dd className="font-medium">
                            {interfacePackLabel(world.interfacePackId, packs)}
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
                        The dashboard remains the ownership-safe control room
                        for the world while deeper management flows come online.
                      </p>
                      <div className="grid gap-2 text-sm">
                        <Link
                          to={`/world/${world.id}/staging`}
                          className="text-primary underline-offset-4 hover:underline"
                        >
                          Enter the live workspace
                        </Link>
                        {IN_DEMO ? null : (
                          <>
                            <Link
                              to="/worlds/create"
                              className="text-primary underline-offset-4 hover:underline"
                            >
                              Create another world
                            </Link>
                            {ownsWorld ? (
                              <button
                                type="button"
                                onClick={() => void handleDelete()}
                                className="text-left text-destructive underline-offset-4 hover:underline"
                                data-testid="world-delete-permanently"
                              >
                                Permanently delete this world
                              </button>
                            ) : null}
                          </>
                        )}
                      </div>
                    </Card>
                  </section>

                  {/* Invite links and the campaign's switches are the Game
                  Master's; every write behind them is `is_dm_of_world` on the
                  server. It used to render for every member, which is how a
                  Player came to see "Generate invite link". */}
                  {isGm ? <CampaignSettingsPanel worldId={world.id} /> : null}
                </div>
              </div>
            </>
          )}
        </main>
      </div>
    </>
  );
}
