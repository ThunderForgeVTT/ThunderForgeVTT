import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { SceneRecord } from "@/types/scene";

export interface SceneSwitcherProps {
  scenes: SceneRecord[];
  sceneId: string | null;
  onSceneChange: (sceneId: string) => void;
}

/**
 * Scene switcher: lets a GM (or any world participant) pick which scene's
 * canvas is currently loaded — walls/lights/shapes/background all belong
 * to one `sceneId` at a time (see engine/world/sync's per-scene loaders
 * and the engine's `set_scene_background` command). Every world always
 * has at least one scene once created, so this renders nothing useful
 * (and is skipped by the caller) until scenes have loaded.
 */
export function SceneSwitcher({ scenes, sceneId, onSceneChange }: SceneSwitcherProps) {
  if (scenes.length === 0) {
    return null;
  }

  return (
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
  );
}
