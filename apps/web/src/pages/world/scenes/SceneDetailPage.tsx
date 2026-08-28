import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
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
  const [scene, setScene] = useState<SceneRecord | null | undefined>(undefined);
  const [summaryDraft, setSummaryDraft] = useState("");
  const [isSavingSummary, setIsSavingSummary] = useState(false);
  const [isSavingHidden, setIsSavingHidden] = useState(false);
  const [isLaunching, setIsLaunching] = useState(false);
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

  const handleLaunch = async () => {
    setIsLaunching(true);
    setStatus(null);
    try {
      await launchScene(worldId, sceneId);
      setStatus("Scene launched.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to launch scene");
    } finally {
      setIsLaunching(false);
    }
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
          <Button
            type="button"
            icon="spark"
            onClick={() => void handleLaunch()}
            disabled={isLaunching}
            data-testid="launch-scene-button"
          >
            {isLaunching ? "Launching..." : "Launch"}
          </Button>
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
