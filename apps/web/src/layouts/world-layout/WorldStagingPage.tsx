import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { GenieSessionPanel } from "@/components/world/GenieSessionPanel/GenieSessionPanel";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import { SessionNotesPanel } from "@/components/world/SessionNotesPanel/SessionNotesPanel";
import { SessionSetupInviteLink } from "@/components/world/SessionSetupInviteLink";
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

/** Same Owner/GM → "Game Master", else "Player" collapse used on the
 * Welcome page hub (roleBadgeLabel there) — kept local since this is the
 * only other place a member's role becomes a badge today. */
function roleBadgeLabel(role: string): "Game Master" | "Player" {
  return role === "Owner" || role === "GM" ? "Game Master" : "Player";
}

/**
 * Spec 009 (T009, US1): the staging page every world member sees first at
 * `/world/:id/play`. Spec 011 (US3): simplified down to exactly Play,
 * Players, and Last Session Notes — the NPC catalog and the old "Lore —
 * coming soon" placeholder moved to the dedicated `/world/:id/compendium`
 * portal (linked from here).
 *
 * Layout reads top-to-bottom as the actual pre-session checklist: confirm
 * the scene and who's in the room in one glanceable strip, catch up on
 * where the story left off, then Play — rather than a grid of
 * equal-weight, disconnected widgets.
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
    <main
      className="mx-auto grid min-h-screen w-full max-w-4xl gap-6 bg-background p-4 sm:p-6"
      data-testid="world-staging-page"
    >
      <header className="grid gap-3">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Session setup
            </p>
            <h1 className="text-3xl font-semibold">{world?.name ?? "World"}</h1>
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
        </div>

        {/* Toolbar row: a slim, subdued strip rather than a plain paragraph.
         * The Compendium/System settings links formerly here now live in
         * the persistent WorldSidebarNav (WorldSectionShell). */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg border border-border bg-card/50 px-4 py-2 text-sm text-muted-foreground">
          <span>Confirm the scene and roster below, then hand the screen to the game.</span>
        </div>
      </header>

      {/* The story so far, read before the roster check — catching up on
       * where things left off naturally comes before confirming who's in
       * the room for tonight. */}
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

      {/* "At a glance" strip: scene + roster together, since checking both
       * is one mental step ("what are we playing, who's here") rather than
       * two separately-weighted panels. */}
      <Panel variant="stone" className="grid gap-5 rounded-xl border border-border sm:grid-cols-[1fr_auto] sm:items-start">
        <div className="grid gap-2">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
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
        </div>

        <div className="grid gap-2 sm:w-64 sm:border-l sm:border-border sm:pl-5">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Players ({members.length})
          </p>
          <ScrollArea className="h-32">
            <ul className="grid gap-2" data-testid="staging-player-list">
              {members.map((member) => (
                <li key={member.id} className="flex items-center justify-between gap-2">
                  <span className="text-sm">{member.display_name ?? member.user_id}</span>
                  <Badge
                    variant={member.role === "Owner" || member.role === "GM" ? "default" : "secondary"}
                  >
                    {roleBadgeLabel(member.role)}
                  </Badge>
                </li>
              ))}
            </ul>
          </ScrollArea>
          {isGm ? <SessionSetupInviteLink worldId={worldId} /> : null}
        </div>
      </Panel>

      {/* Spec 018 US7: the Genie session loop (Wish Pool, Doom Clock,
       * Puzzle Clocks) — only relevant for Genie-system worlds. */}
      {world?.gameSystemId === "genie" ? (
        <Panel
          variant="stone"
          className="grid gap-3 rounded-xl border border-border"
          data-testid="genie-session-panel-wrapper"
        >
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Genie session
          </p>
          <GenieSessionPanel worldId={worldId} isGm={isGm} />
        </Panel>
      ) : null}
    </main>
  );
}
