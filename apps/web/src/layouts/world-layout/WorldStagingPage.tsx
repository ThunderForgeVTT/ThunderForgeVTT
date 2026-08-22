import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import { NpcRoster } from "@/components/world/NpcRoster/NpcRoster";
import { useWorldMembers } from "@/hooks/useWorldMembers";
import type { SceneRecord } from "@/types/scene";
import type { WorldRecord } from "@/types/world";

export interface WorldStagingPageProps {
  worldId: string;
  world: WorldRecord | null;
  scenes: SceneRecord[];
  sceneId: string | null;
  onSceneChange: (sceneId: string) => void;
  onSceneCreated: (scene: SceneRecord) => void;
  /** Whether the current user may create scenes — GM/Owner only (FR-012). */
  isGm: boolean;
  onPlay: () => void;
}

/**
 * Spec 009 (T009, US1): the staging page every world member sees first at
 * `/world/:id/play` — replaces the old `WorldLayout.tsx` placeholder shell.
 * Real scenes, real players, real NPCs, one "Play" action into full-screen
 * canvas mode. Lore is a labeled extension point only (FR-005) — not real
 * content in this pass.
 */
export function WorldStagingPage({
  worldId,
  world,
  scenes,
  sceneId,
  onSceneChange,
  onSceneCreated,
  isGm,
  onPlay,
}: WorldStagingPageProps) {
  const { members } = useWorldMembers(worldId);

  return (
    <main className="grid min-h-screen gap-4 bg-background p-4" data-testid="world-staging-page">
      <header className="flex items-start justify-between gap-4 rounded-xl border border-border bg-card p-5">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Session setup
          </p>
          <h1 className="text-2xl font-semibold">{world?.name ?? "World"}</h1>
          <p className="mt-2 max-w-3xl text-muted-foreground">
            Confirm the scene, roster, and NPCs before handing the screen to
            the game.
          </p>
        </div>
        <Button
          type="button"
          variant="primary"
          size="lg"
          icon="spark"
          data-testid="play-button"
          onClick={onPlay}
        >
          Play
        </Button>
      </header>

      <section className="grid gap-4 lg:grid-cols-3">
        <Panel variant="stone" className="rounded-xl border border-border">
          <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Scene
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

        <Panel variant="leather" className="rounded-xl border border-border">
          <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Players
          </p>
          <ScrollArea className="h-40">
            <ul className="grid gap-2" data-testid="staging-player-list">
              {members.map((member) => (
                <li key={member.id} className="flex items-center justify-between gap-2">
                  <span className="text-sm">{member.display_name ?? member.user_id}</span>
                  <small className="text-xs text-muted-foreground uppercase tracking-wide">
                    {member.role}
                  </small>
                </li>
              ))}
            </ul>
          </ScrollArea>
        </Panel>

        <Panel variant="parchment" className="rounded-xl border border-border">
          <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            NPCs
          </p>
          <ScrollArea className="h-40">
            <NpcRoster worldId={worldId} />
          </ScrollArea>
        </Panel>
      </section>

      <Card className="grid gap-2 opacity-60">
        <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          <FantasyIcon name="quill" size={14} />
          Lore — coming soon
        </p>
        <p className="text-sm text-muted-foreground">
          A dedicated lore/notes section for this world will live here in a
          future update.
        </p>
      </Card>
    </main>
  );
}
