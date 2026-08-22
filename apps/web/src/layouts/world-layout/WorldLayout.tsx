import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { TokenAvatar } from "@/components/ui/token-avatar/TokenAvatar";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import { NpcRoster } from "@/components/world/NpcRoster/NpcRoster";
import type { WorldToken } from "@/engine/world/types";
import type { SceneRecord } from "@/types/scene";

interface WorldLayoutProps {
  worldId: string;
  canvas: ReactNode;
  tokens: WorldToken[];
  onBackToStaging: () => void;
  scenes: SceneRecord[];
  sceneId: string | null;
  onSceneChange: (sceneId: string) => void;
  onSceneCreated: (scene: SceneRecord) => void;
  isGm: boolean;
}

/**
 * Spec 009 (T011/T014): full-screen canvas mode. Replaces the old
 * permanent placeholder shell — the canvas occupies the entire viewport;
 * only a small on-screen "back to setup" control and a collapsible
 * sidebar (scenes, NPC/combat, trackers/settings, lore extension point)
 * sit on top of it. The canvas itself (passed as `canvas`) is expected to
 * stay mounted across staging/full-screen toggles by the caller
 * (`WorldPage.tsx`) — this component does not control that.
 */
export function WorldLayout({
  worldId,
  canvas,
  tokens,
  onBackToStaging,
  scenes,
  sceneId,
  onSceneChange,
  onSceneCreated,
  isGm,
}: WorldLayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const currentScene = scenes.find((scene) => scene.sceneId === sceneId) ?? null;

  return (
    <div className="relative h-screen w-screen overflow-hidden" data-world-id={worldId}>
      <div className="absolute inset-0">{canvas}</div>

      {/* Bottom-left, deliberately clear of the existing canvas tool
       * panels (wall/light/map/token tools top-left, shape tool top-right,
       * token panel toggle bottom-right — all z-900/1000, see WorldPage.tsx)
       * so this control is never covered by them. */}
      <div className="absolute bottom-4 left-4 z-40 flex gap-2">
        <Button
          type="button"
          variant="secondary"
          size="sm"
          icon="arrow-left"
          data-testid="back-to-staging-button"
          onClick={onBackToStaging}
        >
          Back to setup
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          icon="worlds"
          data-testid="sidebar-toggle-button"
          onClick={() => setSidebarOpen((current) => !current)}
        >
          {sidebarOpen ? "Hide tools" : "Tools"}
        </Button>
      </div>

      {sidebarOpen ? (
        <aside
          // z-[1050], well above the canvas and existing tool panels, so
          // this sidebar's own controls (SceneSwitcher, etc.) are always
          // reliably clickable. Vertically inset (top-28/bottom-24) rather
          // than full-height so it doesn't spatially cover the two
          // right-docked existing controls that would otherwise sit behind
          // it: ShapeTool (top-right, ~1rem-6rem) and the token panel
          // toggle/panel (bottom-right, bottom ~1rem) — both keep their
          // original z-900 and stay reachable in the gap this leaves.
          className="absolute top-28 right-0 bottom-24 z-[1050] grid w-80 gap-4 overflow-y-auto rounded-l-xl border-y border-l border-border bg-background/95 p-4 shadow-lg backdrop-blur"
          data-testid="world-sidebar"
        >
          <Panel variant="stone" className="rounded-xl border border-border">
            <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Scenes
            </p>
            <SceneSwitcher
              worldId={worldId}
              scenes={scenes}
              sceneId={sceneId}
              onSceneChange={onSceneChange}
              onSceneCreated={onSceneCreated}
              canCreateScene={isGm}
            />
          </Panel>

          <Panel variant="parchment" className="rounded-xl border border-border">
            <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              NPC / Combat
            </p>
            <NpcRoster worldId={worldId} />
          </Panel>

          <Panel variant="leather" className="overflow-hidden rounded-xl border border-border">
            <Tabs
              defaultValue="trackers"
              items={[
                {
                  value: "trackers",
                  label: "Trackers",
                  icon: "tokens",
                  content: (
                    <ScrollArea className="h-56">
                      <div className="grid gap-4">
                        {tokens.length === 0 ? (
                          <p className="text-sm text-muted-foreground">
                            No tokens on this scene yet.
                          </p>
                        ) : (
                          tokens.map((token) => (
                            <div key={token.id} className="flex items-center gap-3">
                              <TokenAvatar
                                seed={token.id}
                                label={token.label ?? token.id}
                              />
                              <div>
                                <strong className="block">
                                  {token.label ?? token.id}
                                </strong>
                                <small className="mt-1 block text-[0.72rem] tracking-widest text-muted-foreground uppercase">
                                  X {Math.round(token.x)} / Y {Math.round(token.y)}
                                </small>
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </ScrollArea>
                  ),
                },
                {
                  value: "settings",
                  label: "Settings",
                  icon: "settings",
                  content: currentScene ? (
                    <dl className="grid gap-2 text-sm">
                      <div className="flex justify-between">
                        <dt className="text-muted-foreground">Scene</dt>
                        <dd>{currentScene.name}</dd>
                      </div>
                      <div className="flex justify-between">
                        <dt className="text-muted-foreground">Grid</dt>
                        <dd>
                          {currentScene.gridType} · {currentScene.gridSize}px
                        </dd>
                      </div>
                      <div className="flex justify-between">
                        <dt className="text-muted-foreground">Size</dt>
                        <dd>
                          {currentScene.width}×{currentScene.height}
                        </dd>
                      </div>
                    </dl>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      No scene selected.
                    </p>
                  ),
                },
              ]}
            />
          </Panel>

          <Panel variant="veil" className="rounded-xl border border-border opacity-60">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Lore — coming soon
            </p>
          </Panel>
        </aside>
      ) : null}
    </div>
  );
}
