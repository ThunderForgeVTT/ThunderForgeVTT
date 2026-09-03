import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link, useNavigate } from "react-router-dom";
import {
  getScene,
  launchScene,
  updateScene,
  updateSceneHidden,
} from "@/api/scenes";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { MapImportTool } from "@/components/canvas-tools/MapImportTool/MapImportTool";
import { LoreMarkdownRenderer } from "@/pages/world/lore/LoreMarkdownRenderer";
import { SceneSummaryEditor } from "@/pages/world/scenes/SceneSummaryEditor";
import {
  bringPartyToScene,
  describeArrival,
} from "@/pages/world/scenes/bringParty";
import { preloadScene } from "@/services/scenePreload";
import type { SceneRecord } from "@/types/scene";

export interface SceneDetailPageProps {
  worldId: string;
  sceneId: string;
  isGm: boolean;
}

/**
 * Spec 022 (FR-001a, US1, US2): the per-scene detail gateway. GM/Owner
 * gets the full management surface (summary editing, dd2vtt import,
 * hidden toggle, Launch); everyone else gets the read-only summary +
 * preview thumbnail (FR-011). A single component branching on `isGm`
 * rather than separate view/edit routes — unlike lore entries, there's
 * no "player with edit rights" case here (only GM/Owner ever edits a
 * scene), so a view/edit mode split would just be two names for the same
 * non-GM branch.
 */
export function SceneDetailPage({
  worldId,
  sceneId,
  isGm,
}: SceneDetailPageProps) {
  const navigate = useNavigate();
  const [scene, setScene] = useState<SceneRecord | null | undefined>(undefined);
  const [summaryDraft, setSummaryDraft] = useState("");
  const [isSavingSummary, setIsSavingSummary] = useState(false);
  const [isSavingHidden, setIsSavingHidden] = useState(false);
  const [isLaunching, setIsLaunching] = useState(false);
  const [isPreloading, setIsPreloading] = useState(false);
  // Spec 031 FR-019 / ADR-056: off unless the Game Master says otherwise.
  // Most scene changes are prep, a reveal or a cutaway, and a default that
  // moved everyone's character would have the GM undoing tokens far more
  // often than placing them.
  const [bringParty, setBringParty] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(sceneId, () => {
    setScene(undefined);
  });

  useEffect(() => {
    let active = true;

    getScene(sceneId)
      .then((result) => {
        if (active) {
          setScene(result);
          setSummaryDraft(result?.summaryMarkdown ?? "");
        }
      })
      .catch(() => {
        if (active) {
          setScene(null);
        }
      });

    return () => {
      active = false;
    };
  }, [sceneId]);

  // Spec 022: a quiet re-fetch after a dd2vtt import completes (to pick up
  // the new preview thumbnail) — deliberately does NOT reset `scene` to
  // `undefined` first like the effect above. Doing so would unmount this
  // whole page's content behind a full-page Loader, which also unmounts
  // `MapImportTool` and wipes out the "Map imported" success panel it's
  // still showing (found live: the success message disappeared the
  // instant the refetch kicked off).
  const handleMapImported = () => {
    getScene(sceneId)
      .then((result) => {
        if (result) {
          setScene(result);
        }
      })
      .catch(() => {
        // Best-effort refresh only — the import itself already succeeded
        // (MapImportTool shows its own success state independently).
      });
  };

  const handleSaveSummary = async () => {
    setIsSavingSummary(true);
    setStatus(null);
    try {
      const updated = await updateScene(sceneId, {
        summaryMarkdown: summaryDraft,
      });
      setScene(updated);
      setStatus("Summary saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save summary");
    } finally {
      setIsSavingSummary(false);
    }
  };

  const handleToggleHidden = async (hidden: boolean) => {
    setIsSavingHidden(true);
    setStatus(null);
    try {
      const updated = await updateSceneHidden(sceneId, hidden);
      setScene(updated);
    } catch (err) {
      setStatus(
        err instanceof Error ? err.message : "Failed to update visibility",
      );
    } finally {
      setIsSavingHidden(false);
    }
  };

  /**
   * Set the table's scene, then go and stand at it.
   *
   * The navigation is the fix, not a flourish. Launch previously set the
   * active scene and left the Game Master on the scene page reading "Scene
   * launched." — so the players were looking at a map the person who launched
   * it was not, and the only way in was to find the Play link by hand. Spec
   * 031 FR-021 makes entering play part of what Launch means, which is also
   * what separates it from Preload.
   *
   * Navigating only after `launchScene` resolves: arriving at a table whose
   * scene did not actually change would be a worse failure than staying put
   * with the error visible.
   */
  const handleLaunch = async () => {
    setIsLaunching(true);
    setStatus(null);
    try {
      // The party arrives *before* the switch is broadcast, so the table
      // opens on a room that already has them in it. The other order shows
      // every player an empty scene and then pops six tokens into it, which
      // reads as a glitch rather than as the party walking in.
      //
      // A failure here stops the launch. The GM asked for one thing — move
      // the table, with the party — and half of it is worse than neither:
      // the players would be standing in the cellar without their
      // characters, and the fix is a second scene change.
      if (bringParty) {
        const arrival = await bringPartyToScene(sceneId);
        // Said here for the case where the launch below then fails and this
        // page stays put: the party did move, and the GM needs to know that
        // before deciding what to do next. On the ordinary path the map
        // itself is the report, and it is a better one.
        setStatus(describeArrival(arrival));
      }
      await launchScene(worldId, sceneId);
      navigate(`/world/${worldId}/play`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to launch scene");
      setIsLaunching(false);
    }
  };

  /**
   * Warm this scene in *this* browser and tell nobody.
   *
   * Deliberately not a mutation. See `services/scenePreload` for why the
   * server must not hear about it: ADR-046 broadcasts the active scene, so
   * anything server-side would show at the table, which is the opposite of
   * preparing (spec 031 FR-020, SC-004).
   */
  const handlePreload = async () => {
    if (!scene) {
      return;
    }
    setIsPreloading(true);
    setStatus(null);
    const outcome = await preloadScene(scene);
    setStatus(
      outcome.warmed
        ? "Scene preloaded. Players saw nothing."
        : outcome.reason === "no-background"
          ? "Nothing to preload — this scene has no background."
          : "Could not preload. The scene will still open normally.",
    );
    setIsPreloading(false);
  };

  if (scene === undefined) {
    return <Loader label="Loading scene" />;
  }

  if (!scene) {
    return (
      <Card
        className="grid gap-3 p-6 text-center"
        data-testid="scene-not-found"
      >
        <h1 className="text-xl font-semibold">Scene not found</h1>
        <p className="text-muted-foreground">
          This scene doesn't exist, or you don't have access to it.
        </p>
        <Link
          to={`/world/${worldId}/scenes`}
          className="text-primary hover:underline"
        >
          Back to Scenes
        </Link>
      </Card>
    );
  }

  return (
    <div className="grid gap-4">
      <Button
        asChild
        variant="ghost"
        size="sm"
        icon="arrow-left"
        className="justify-self-start"
      >
        <Link to={`/world/${worldId}/scenes`}>Back to Scenes</Link>
      </Button>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold">{scene.name}</h1>
        {isGm ? (
          <div className="grid justify-items-end gap-1">
            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                variant="secondary"
                icon="torch"
                onClick={() => void handlePreload()}
                disabled={isPreloading || isLaunching}
                data-testid="preload-scene-button"
              >
                {isPreloading ? "Preloading..." : "Preload"}
              </Button>
              <Button
                type="button"
                icon="spark"
                onClick={() => void handleLaunch()}
                disabled={isLaunching || isPreloading}
                data-testid="launch-scene-button"
              >
                {isLaunching ? "Launching..." : "Launch"}
              </Button>
            </div>
            {/*
              Attached to Launch, not offered on its own (spec 031 FR-019,
              ADR-056). Launch is what moves the table, and bringing the
              party is a property of that move rather than a separate errand
              — a standalone "bring the party" button would let a GM populate
              a scene nobody is looking at, and would leave the two actions
              free to be done in the wrong order.
            */}
            <label
              className="flex items-center gap-2 text-xs text-muted-foreground"
              data-testid="bring-party-toggle-label"
            >
              <input
                type="checkbox"
                checked={bringParty}
                disabled={isLaunching}
                onChange={(event) => setBringParty(event.target.checked)}
                data-testid="bring-party-toggle"
              />
              Bring the party
            </label>
            {/*
              Said rather than inferred (spec 031 FR-022). The two buttons sit
              together and one of them changes what every player is looking
              at, so which is which cannot be left to be discovered.
            */}
            <p
              className="text-xs text-muted-foreground"
              data-testid="scene-action-explainer"
            >
              Launch moves the table here. Preload only warms this browser.
            </p>
          </div>
        ) : null}
      </div>

      <Card className="grid gap-3 p-4" data-testid="scene-preview-card">
        {scene.previewUrl ? (
          <img
            src={scene.previewUrl}
            alt={`${scene.name} preview`}
            className="max-h-64 w-full rounded-lg border border-border object-contain"
          />
        ) : (
          <div className="grid h-32 place-items-center rounded-lg border border-dashed border-border text-sm text-muted-foreground">
            No map preview yet
          </div>
        )}
      </Card>

      {isGm ? (
        <>
          <Card className="grid gap-3 p-4" data-testid="scene-import-card">
            <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
              Import map
            </h2>
            <MapImportTool
              sceneId={sceneId}
              onImportComplete={handleMapImported}
              gridMismatch={scene?.backgroundGridMismatch}
            />
          </Card>

          <Card className="grid gap-3 p-4" data-testid="scene-hidden-card">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={!scene.hidden}
                disabled={isSavingHidden}
                onChange={(event) =>
                  void handleToggleHidden(!event.target.checked)
                }
                data-testid="scene-hidden-toggle"
              />
              Visible to players
            </label>
            <p className="text-xs text-muted-foreground">
              New scenes start hidden. Un-check to hide it again at any time.
            </p>
          </Card>

          <Card className="grid gap-3 p-4" data-testid="scene-summary-card">
            <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
              Summary
            </h2>
            <SceneSummaryEditor
              value={summaryDraft}
              onChange={setSummaryDraft}
              disabled={isSavingSummary}
            />
            <div className="flex items-center gap-3">
              <Button
                onClick={() => void handleSaveSummary()}
                disabled={isSavingSummary}
              >
                {isSavingSummary ? "Saving..." : "Save summary"}
              </Button>
            </div>
          </Card>

          {status ? (
            <StatusBadge
              variant={status.includes("Failed") ? "danger" : "success"}
            >
              {status}
            </StatusBadge>
          ) : null}
        </>
      ) : (
        <Card className="p-6" data-testid="scene-summary-view">
          {scene.summaryRenderedHtml ? (
            <LoreMarkdownRenderer html={scene.summaryRenderedHtml} />
          ) : (
            <p className="text-sm text-muted-foreground italic">
              The GM hasn't written a summary for this scene yet.
            </p>
          )}
        </Card>
      )}
    </div>
  );
}
