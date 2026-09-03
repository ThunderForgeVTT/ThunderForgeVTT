import { useEffect, useState } from "react";
import { MapImportTool } from "@/components/canvas-tools/MapImportTool";
import { CachePanel } from "@/components/diagnostics/CachePanel";
import { Switch } from "@/components/ui/switch";
import {
  setEngineMonitorVisible,
  subscribeToEngineMonitor,
} from "@/services/engineMonitor";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import { Button } from "@/components/ui/button/Button";
import type { SceneRecord } from "@/types/scene";

export interface SettingsPanelProps {
  worldId: string;
  sceneId: string | null;
  scenes: SceneRecord[];
  isGm: boolean;
  onSceneChange: (sceneId: string) => void;
  onSceneCreated: (scene: SceneRecord) => void;
  onMapImportComplete: () => void;
  onBackToStaging: () => void;
}

/**
 * Settings: scene selection, map import, scene properties, and the way
 * back to setup.
 *
 * Map import lives here rather than on the canvas toolbar because it is a
 * setup action, not a drawing tool — it was previously the first button in
 * the GM's left-hand tool stack, above "Draw wall", which put a
 * once-per-scene file upload in the same visual weight as the tools used
 * every few seconds.
 */
export function SettingsPanel({
  worldId,
  sceneId,
  scenes,
  isGm,
  onSceneChange,
  onSceneCreated,
  onMapImportComplete,
  onBackToStaging,
}: SettingsPanelProps) {
  const currentScene =
    scenes.find((scene) => scene.sceneId === sceneId) ?? null;

  // Subscribed rather than read once: this panel is unmounted whenever the
  // dock is collapsed, so it has no state of its own to trust on the way
  // back in, and the setting can also change in another tab.
  const [monitorVisible, setMonitorVisible] = useState(false);
  useEffect(() => subscribeToEngineMonitor(setMonitorVisible), []);

  return (
    <div className="grid gap-5" data-testid="settings-panel">
      <section className="grid gap-2">
        <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Scenes
        </h3>
        <SceneSwitcher
          worldId={worldId}
          scenes={scenes}
          sceneId={sceneId}
          onSceneChange={onSceneChange}
          onSceneCreated={onSceneCreated}
          canCreateScene={isGm}
        />
      </section>

      {isGm && sceneId ? (
        <section className="grid gap-2 border-t border-border pt-4">
          <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Import map
          </h3>
          <MapImportTool
            // Remount on scene switch so a previous scene's import summary
            // doesn't linger — same reason WorldPage keyed it before.
            key={sceneId}
            sceneId={sceneId}
            onImportComplete={onMapImportComplete}
            gridMismatch={
              scenes.find((scene) => scene.sceneId === sceneId)
                ?.backgroundGridMismatch
            }
          />
        </section>
      ) : null}

      {currentScene ? (
        <section className="grid gap-2 border-t border-border pt-4">
          <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Scene
          </h3>
          <dl className="grid gap-1.5 text-sm">
            <div className="flex justify-between gap-2">
              <dt className="text-muted-foreground">Name</dt>
              <dd className="truncate">{currentScene.name}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-muted-foreground">Grid</dt>
              <dd>
                {currentScene.gridType} · {currentScene.gridSize}px
              </dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-muted-foreground">Size</dt>
              <dd className="tabular-nums">
                {currentScene.width}×{currentScene.height}
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      {/*
        A display preference for this device, which is why it sits with the
        scene controls rather than in world settings: it changes what this
        screen shows, not anything about the world or anyone else's view of
        it.
      */}
      <section className="grid gap-2 border-t border-border pt-4">
        <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Display
        </h3>
        <label
          className="flex items-center justify-between gap-3 text-sm"
          htmlFor="engine-monitor-toggle"
        >
          <span className="grid gap-0.5">
            <span>Performance readout</span>
            <span className="text-xs text-muted-foreground">
              Frames, server latency and connected players, along the bottom of
              the map.
            </span>
          </span>
          <Switch
            id="engine-monitor-toggle"
            data-testid="engine-monitor-toggle"
            checked={monitorVisible}
            onCheckedChange={setEngineMonitorVisible}
          />
        </label>
      </section>

      {/*
        Spec 028 FR-051/SC-017. What the local cache did for *this* session,
        which is why it is here on the world dock rather than on
        `/settings/storage`: the figures live in the running engine, and a
        settings page has no engine mounted. See `CachePanel` for the whole
        argument. It sits under Display for the same reason that toggle does —
        both are about this device and this screen, not about the world.
      */}
      <section className="border-t border-border pt-4">
        <CachePanel />
      </section>

      <section className="border-t border-border pt-4">
        <Button
          type="button"
          variant="secondary"
          size="sm"
          icon="arrow-left"
          fullWidth
          data-testid="back-to-staging-button"
          onClick={onBackToStaging}
        >
          Back to setup
        </Button>
      </section>
    </div>
  );
}
