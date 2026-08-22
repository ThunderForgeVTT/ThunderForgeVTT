import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import { SessionNotesPanel } from "@/components/world/SessionNotesPanel/SessionNotesPanel";
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
  /** Called when Last Session Notes is saved, so the caller's own world
   * record stays in sync without a full refetch. */
  onSessionNotesSaved: (notes: string) => void;
}

/**
 * Spec 009 (T009, US1): the staging page every world member sees first at
 * `/world/:id/play`. Spec 011 (US3): simplified down to exactly Play,
 * Players, and Last Session Notes — the NPC catalog and the old "Lore —
 * coming soon" placeholder moved to the dedicated `/world/:id/compendium`
 * portal (linked from here).
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
  onSessionNotesSaved,
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
            Confirm the scene and roster before handing the screen to the
            game. Manage NPCs, items, and abilities from the Compendium.
          </p>
        </div>
        <div className="flex flex-col items-end gap-2">
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
          <Button asChild variant="secondary" size="sm" data-testid="compendium-link">
            <Link to={`/world/${worldId}/compendium`}>Open Compendium</Link>
          </Button>
        </div>
      </header>

      <section className="grid gap-4 lg:grid-cols-2">
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
            testIdPrefix="staging-"
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
      </section>

      <section>
        <Panel variant="parchment" className="rounded-xl border border-border" data-testid="session-notes-panel">
          <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Last session notes
          </p>
          <SessionNotesPanel
            worldId={worldId}
            notes={world?.sessionNotes ?? null}
            isGm={isGm}
            onSaved={onSessionNotesSaved}
          />
        </Panel>
      </section>
    </main>
  );
}
