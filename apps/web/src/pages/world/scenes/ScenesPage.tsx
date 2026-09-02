import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
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

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${refreshTick}`, () => {
    setScenes(null);
    setError(null);
  });

  useEffect(() => {
    let active = true;

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
      setCreateError(
        err instanceof Error ? err.message : "Failed to create scene",
      );
    } finally {
      setIsCreating(false);
    }
  };

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load scenes: {error.message}
      </p>
    );
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
                {/*
                  Spec 031 FR-023. The list was a column of names, which is
                  the least useful thing to choose a map by: a Game Master
                  picking the next scene mid-session recognises the picture
                  long before they read the title.
                */}
                <th className="p-2 font-semibold">Map</th>
                <th className="p-2 font-semibold">Name</th>
                <th className="p-2 font-semibold">Description</th>
                {isGm ? (
                  <th className="p-2 font-semibold">Visibility</th>
                ) : null}
              </tr>
            </thead>
            <tbody>
              {scenes.map((scene) => (
                <tr
                  key={scene.sceneId}
                  className="border-b border-border last:border-0 hover:bg-muted/40"
                  data-testid={`scene-row-${scene.sceneId}`}
                >
                  <td className="p-2">
                    {scene.previewUrl ? (
                      <img
                        src={scene.previewUrl}
                        alt=""
                        // Empty alt on purpose: the name sits in the very
                        // next cell, so describing the thumbnail would make a
                        // screen reader announce the scene twice.
                        loading="lazy"
                        className="h-12 w-20 rounded border border-border object-cover"
                        data-testid={`scene-preview-${scene.sceneId}`}
                      />
                    ) : (
                      <div className="grid h-12 w-20 place-items-center rounded border border-dashed border-border text-[10px] text-muted-foreground">
                        No map
                      </div>
                    )}
                  </td>
                  <td className="p-2 font-medium">
                    <Link
                      to={`/world/${worldId}/scenes/${scene.sceneId}`}
                      className="hover:underline"
                    >
                      {scene.name}
                    </Link>
                  </td>
                  <td className="max-w-md p-2 text-muted-foreground">
                    {scene.description ? (
                      <span className="line-clamp-2">{scene.description}</span>
                    ) : (
                      <span className="italic">No description</span>
                    )}
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
          {createError ? (
            <p className="text-sm text-destructive sm:col-span-2">
              {createError}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
