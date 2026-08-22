import { useState } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { createScene } from "@/api/scenes";
import type { SceneRecord } from "@/types/scene";

export interface SceneSwitcherProps {
  worldId: string;
  scenes: SceneRecord[];
  sceneId: string | null;
  onSceneChange: (sceneId: string) => void;
  /** Called after a new scene is created, so the caller can add it to its
   * scenes list and select it — this component doesn't own scene state. */
  onSceneCreated: (scene: SceneRecord) => void;
  /** New-scene creation is a GM/world-owner action, same as the other
   * canvas authoring tools; the caller decides who sees it. */
  canCreateScene: boolean;
  /** Distinguishes this instance's `data-testid`s from any other mounted
   * `SceneSwitcher` (e.g. the staging page's vs. the full-screen play
   * sidebar's). Without this, a brief overlap during route transitions
   * (the previous route's Suspense-wrapped tree can still be committed
   * for a few ms while the next route's lazy chunk loads) can leave two
   * elements answering to the same testid at once, which e2e locators
   * then resolve ambiguously/racily. Defaults to "" (no prefix). */
  testIdPrefix?: string;
}

/**
 * Scene switcher: lets a GM (or any world participant) pick which scene's
 * canvas is currently loaded — walls/lights/shapes/background all belong
 * to one `sceneId` at a time (see engine/world/sync's per-scene loaders
 * and the engine's `set_scene_background` command). For the GM, also
 * offers creating a new scene (the primary way a world gets a second map
 * to switch to — e.g. importing a different .dd2vtt into each).
 */
export function SceneSwitcher({
  worldId,
  scenes,
  sceneId,
  onSceneChange,
  onSceneCreated,
  canCreateScene,
  testIdPrefix = "",
}: SceneSwitcherProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Scene name is required.");
      return;
    }

    setCreating(true);
    setError(null);

    try {
      const scene = await createScene({ worldId, name: trimmed });
      onSceneCreated(scene);
      onSceneChange(scene.sceneId);
      setDialogOpen(false);
      setName("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create scene");
    } finally {
      setCreating(false);
    }
  };

  return (
    <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
      {scenes.length > 0 ? (
        <Select value={sceneId ?? undefined} onValueChange={onSceneChange}>
          <SelectTrigger aria-label="Scene" data-testid="scene-switcher">
            <SelectValue placeholder="Select a scene" />
          </SelectTrigger>
          <SelectContent>
            {scenes.map((scene) => (
              <SelectItem key={scene.sceneId} value={scene.sceneId}>
                {scene.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : null}

      {canCreateScene ? (
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogTrigger asChild>
            <Button variant="secondary" size="sm" data-testid={`${testIdPrefix}new-scene-button`}>
              New scene
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Create a new scene</DialogTitle>
            </DialogHeader>
            <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
              <Label htmlFor="new-scene-name">Scene name</Label>
              <Input
                id="new-scene-name"
                data-testid={`${testIdPrefix}new-scene-name-input`}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="e.g. The Sunken Crypt"
                autoFocus
              />
              {error ? (
                <p role="alert" style={{ color: "var(--destructive, #dc2626)", fontSize: "0.85rem" }}>
                  {error}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                onClick={() => void handleCreate()}
                disabled={creating}
                data-testid={`${testIdPrefix}create-scene-submit`}
              >
                {creating ? "Creating…" : "Create scene"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      ) : null}
    </div>
  );
}
