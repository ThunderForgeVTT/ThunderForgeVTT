import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { createScene, getScenes } from "@/api/scenes";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import type { SceneRecord } from "@/types/scene";

export interface ScenesPageProps {
  worldId: string;
  isGm: boolean;
}

/**
 * Spec 022 (US1/US2): the Scenes section's list view. GM/Owner sees every
 * scene (hidden or not, per FR-009 — the `scenes` query already applies
 * that server-side filtering by caller role, see queries/scene.rs) with a
 * "New scene" form; everyone else sees only the non-hidden ones (FR-010).
 * Every row links to `/world/:id/scenes/:sceneId` — the per-scene detail
 * gateway (FR-001a) both GM management and player browsing share.
 */
export function ScenesPage({ worldId, isGm }: ScenesPageProps) {
  const [scenes, setScenes] = useState<SceneRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [newName, setNewName] = useState("");
  const [isCreating, setIsCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setScenes(null);
    setError(null);

    getScenes(worldId)
      .then((result) => {
        if (active) {
          setScenes(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, refreshTick]);

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) {
      return;
    }
    setIsCreating(true);
    setCreateError(null);
    try {
      await createScene({ worldId, name });
      setNewName("");
      setRefreshTick((current) => current + 1);
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : "Failed to create scene");
    } finally {
      setIsCreating(false);
    }
  };

  if (error) {
    return <p className="text-sm text-destructive">Failed to load scenes: {error.message}</p>;
  }

  if (scenes === null) {
    return <p className="text-sm text-muted-foreground">Loading scenes…</p>;
  }

  return (
    <div className="grid gap-4">
      <header className="grid gap-1">
        <h1 className="text-xl font-semibold">Scenes</h1>
        <p className="text-sm text-muted-foreground">
          {isGm
            ? "Create scenes, import maps, write summaries, and control what players can see."
            : "Browse the maps the GM has made visible."}
        </p>
      </header>

      {scenes.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">No scenes yet.</p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="scenes-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Name</th>
                {isGm ? <th className="p-2 font-semibold">Visibility</th> : null}
              </tr>
            </thead>
            <tbody>
              {scenes.map((scene) => (
                <tr
                  key={scene.sceneId}
                  className="border-b border-border last:border-0 hover:bg-muted/40"
                  data-testid={`scene-row-${scene.sceneId}`}
                >
                  <td className="p-2 font-medium">
                    <Link to={`/world/${worldId}/scenes/${scene.sceneId}`} className="hover:underline">
                      {scene.name}
                    </Link>
                  </td>
                  {isGm ? (
                    <td className="p-2">
                      <Badge variant={scene.hidden ? "secondary" : "default"}>
                        {scene.hidden ? "Hidden" : "Visible"}
                      </Badge>
                    </td>
                  ) : null}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {isGm ? (
        <div className="grid gap-2 sm:grid-cols-[1fr_auto]">
          <Input
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            placeholder="New scene name"
            disabled={isCreating}
            data-testid="new-scene-name-input"
          />
          <Button
            type="button"
            size="sm"
            icon="map"
            onClick={() => void handleCreate()}
            disabled={isCreating || !newName.trim()}
            data-testid="add-scene-button"
          >
            New scene
          </Button>
          {createError ? <p className="text-sm text-destructive sm:col-span-2">{createError}</p> : null}
        </div>
      ) : null}
    </div>
  );
}
