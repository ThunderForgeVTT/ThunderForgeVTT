import { createElement, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { deleteWorld, getWorld } from "@/api/world";
import {
  interfacePackLabel,
  useInterfacePacks,
} from "@/appearance/interface-pack-label";
import { CampaignSettingsPanel } from "@/components/campaign/CampaignSettingsPanel";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { Panel } from "@/components/ui/panel/Panel";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AboutThisTable } from "@/components/world/AboutThisTable";
import { SessionNotesPanel } from "@/components/world/SessionNotesPanel/SessionNotesPanel";
import { SessionSetupInviteLink } from "@/components/world/SessionSetupInviteLink";
import { useActorClaimGate } from "@/hooks/useActorClaimGate";
import { useAuth } from "@/hooks/useAuth";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { IN_DEMO } from "@/lib/demoBuild";
import { reachedThroughAdminAccess } from "@/pages/world/adminAccess";
import { WorldAtAGlance } from "@/pages/world/components/WorldAtAGlance";
import { PartyStrip } from "@/pages/world/overview/PartyStrip";
import { SceneStrip } from "@/pages/world/overview/SceneStrip";
import {
  resumeLabel,
  useSceneHeadlines,
} from "@/pages/world/overview/sceneStrip";
import { resolvePanel } from "@/panels/systemPanels";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

export const worldOverviewPageSeo: SeoConfig = {
  title: "World overview",
  description:
    "Where the session starts, who is coming, and where the story left off.",
  canonicalPath: "/world",
  noindex: true,
};

/**
 * Spec 100 US2: the world's one front door, at `/world/:id`.
 *
 * This was two pages a member walked through on the way to the table: the
 * dashboard here (figures, players, metadata, the campaign's switches) and
 * staging at `/world/:id/staging` (the sidebar, Play, last session's notes).
 * Staging's line told the Game Master to "confirm the scene and roster
 * below", and neither was below. Now one page holds what both were for,
 * inside the sidebar shell, with Play as its first action; `/staging`
 * redirects here.
 *
 * Kept from the dashboard: the actor-claim gate (spec 017 FR-001: a player
 * with no character is sent to choose one), the administrator-access notice,
 * and deleting the world, which is the Owner's alone. Kept from staging: the
 * notes, the invite link, the system's `world-staging` slot, and what a
 * player may read about the table. Dropped: "Enter world", which was the way
 * here, the "Quick actions" card, which only linked on, and the raw ids of
 * whoever created and last changed the world.
 */
export default function WorldOverviewPage() {
  const navigate = useNavigate();
  const { id = "" } = useParams();
  const { isAdmin, user } = useAuth();
  const [state, setState] = useState<{
    requestedId: string;
    world: WorldRecord | null;
    status: string | null;
    isLoading: boolean;
  }>({ requestedId: id, world: null, status: null, isLoading: true });
  const [isDeleting, setIsDeleting] = useState(false);
  const packs = useInterfacePacks();

  useEffect(() => {
    let active = true;
    void getWorld(id)
      .then((world) => {
        if (active) {
          setState({ requestedId: id, world, status: null, isLoading: false });
        }
      })
      .catch((error) => {
        if (active) {
          setState({
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

  const isLoading = state.requestedId !== id || state.isLoading;
  const world = state.requestedId === id ? state.world : null;
  const status = state.requestedId === id ? state.status : null;

  const { cleared: claimGateCleared } = useActorClaimGate(id, world);
  const { role, isGm, loading: roleLoading } = useWorldRole(id, world);
  const scenes = useSceneHeadlines(id);
  // Deleting ends the world for everyone, so it is the Owner's alone — the
  // same line `deleteWorld` draws on the server (`is_owner_of_world`).
  const ownsWorld = role === "Owner";

  /** Whatever the world's system contributes here, if anything. */
  const SystemPanel = resolvePanel(
    world?.gameSystemId ?? null,
    "world-staging",
  );

  const setNotes = (notes: string) =>
    setState((current) =>
      current.world
        ? { ...current, world: { ...current.world, sessionNotes: notes } }
        : current,
    );

  const handleDelete = async () => {
    if (!world || isDeleting) {
      return;
    }
    if (!window.confirm(`Delete '${world.name}' and its related records?`)) {
      return;
    }
    setIsDeleting(true);
    setState((current) => ({ ...current, status: null }));
    try {
      await deleteWorld(world.id);
      void navigate("/worlds");
    } catch (error) {
      setState((current) => ({
        ...current,
        status:
          error instanceof Error ? error.message : "Failed to delete world.",
      }));
      setIsDeleting(false);
    }
  };

  if (isLoading || roleLoading || (world && !claimGateCleared)) {
    return <Loader fullScreen label="Opening the world" />;
  }

  const seo = (
    <SEO
      {...worldOverviewPageSeo}
      title={world ? world.name : worldOverviewPageSeo.title}
      canonicalPath={id ? `/world/${id}` : worldOverviewPageSeo.canonicalPath}
    />
  );

  if (!world) {
    return (
      <>
        {seo}
        <main className="mx-auto grid w-full max-w-xl gap-4 p-6 py-16">
          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}
          <Card surface="stone" className="grid gap-3 p-8 text-center">
            <h1 className="text-2xl font-semibold">
              This world could not be opened.
            </h1>
            <p className="text-muted-foreground">
              The world may be missing, or your access does not permit viewing
              it.
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
        </main>
      </>
    );
  }

  return (
    <>
      {seo}
      <WorldSectionShell
        worldId={id}
        isGm={isGm}
        playLabel={resumeLabel(scenes, world.activeSceneId)}
      >
        <main className="grid w-full gap-6" data-testid="world-staging-page">
          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          <header className="flex flex-wrap items-start justify-between gap-4">
            <div className="min-w-0">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                Overview
              </p>
              <h1 className="text-3xl font-semibold">{world.name}</h1>
              {world.description ? (
                <p className="mt-1 max-w-[75ch] text-muted-foreground">
                  {world.description}
                </p>
              ) : null}
            </div>
            <div className="flex flex-wrap gap-2">
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
              {/* Spec 074 FR-013: the demo has the one world, kept in this
                  browser; deleting worlds is an instance's business. */}
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
          </header>

          {reachedThroughAdminAccess({ isAdmin, role, roleLoading }) ? (
            <StatusBadge
              variant="warning"
              data-testid="world-admin-access-warning"
            >
              You are viewing this world through administrator access.
            </StatusBadge>
          ) : null}

          {/* Spec 100 US3: where we start, who is coming, and where we
              left off — the three things a table settles before it plays. */}
          <SceneStrip
            worldId={world.id}
            isGm={isGm}
            activeSceneId={world.activeSceneId}
            scenes={scenes}
          />

          <div
            className="grid items-start gap-6 xl:grid-cols-2"
            data-testid="world-dashboard-grid"
          >
            <div className="grid min-w-0 content-start gap-6">
              <PartyStrip
                worldId={world.id}
                runsWorld={isGm}
                canInvite={!IN_DEMO}
              />
              {/* Spec 074 FR-013: the demo's world has no one to invite. */}
              {isGm && !IN_DEMO ? (
                <Panel
                  variant="stone"
                  className="grid gap-2 rounded-xl border border-border"
                >
                  <SessionSetupInviteLink worldId={world.id} />
                </Panel>
              ) : null}
            </div>

            {/* Beside the party from `xl`, under it below: catching up on
                where things left off is read next to who is coming. */}
            <Panel
              variant="parchment"
              className="rounded-xl border border-border"
              data-testid="session-notes-panel"
            >
              <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                Last session notes
              </p>
              <SessionNotesPanel
                worldId={world.id}
                notes={world.sessionNotes ?? null}
                isGm={isGm}
                onSaved={setNotes}
              />
            </Panel>

            {/* The world by its figures (spec 088), and its settings. */}
            <div className="grid min-w-0 content-start gap-6">
              <WorldAtAGlance worldId={world.id} />
              <Card surface="parchment" className="grid gap-3 p-6">
                <h2 className="text-xl font-semibold">This table</h2>
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
                          {world.gameSystemId ?? "Not yet assigned"} — manage
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
                </dl>
              </Card>
            </div>

            {/* Spec 018 US7 lives here, but this page does not know whose it
                is: a pack fills the `world-staging` slot, or nothing does. */}
            {SystemPanel ? (
              <div className="min-w-0 xl:col-span-2">
                {createElement(SystemPanel, {
                  worldId: world.id,
                  world,
                  isGm,
                  currentUserId: user?.id,
                })}
              </div>
            ) : null}

            {/* A Player is shown a GIF on the settings page, so what they may
                read there — the licence, the table's settings, the pause
                history — is read here instead. */}
            {!isGm ? (
              <div className="min-w-0 xl:col-span-2">
                <AboutThisTable world={world} />
              </div>
            ) : null}

            {/* The campaign's switches are the Game Master's; every write
                behind them is `is_dm_of_world` on the server. */}
            {isGm ? (
              <div className="min-w-0">
                <CampaignSettingsPanel worldId={world.id} />
              </div>
            ) : null}
          </div>
        </main>
      </WorldSectionShell>
    </>
  );
}
