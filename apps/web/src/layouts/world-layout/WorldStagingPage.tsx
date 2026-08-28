import { Panel } from "@/components/ui/panel/Panel";
import { GenieSessionPanel } from "@/components/world/GenieSessionPanel/GenieSessionPanel";
import { SessionNotesPanel } from "@/components/world/SessionNotesPanel/SessionNotesPanel";
import { SessionSetupInviteLink } from "@/components/world/SessionSetupInviteLink";
import type { WorldRecord } from "@/types/world";

export interface WorldStagingPageProps {
  worldId: string;
  world: WorldRecord | null;
  /** Whether the current user may create scenes — GM/Owner only (FR-012). */
  isGm: boolean;
  /** Called when Last Session Notes is saved, so the caller's own world
   * record stays in sync without a full refetch. */
  onSessionNotesSaved: (notes: string) => void;
}

/**
 * Spec 009 (T009, US1): the staging page every world member sees first at
 * `/world/:id/play`. Spec 011 (US3): simplified down to exactly Play,
 * Players, and Last Session Notes — the NPC catalog and the old "Lore —
 * coming soon" placeholder moved to the dedicated `/world/:id/compendium`
 * portal (linked from here). Spec 023: the player roster itself moved to
 * its own dedicated Players sidebar section — this page now only keeps
 * the invite link, not the roster list.
 *
 * Layout reads top-to-bottom as the actual pre-session checklist: confirm
 * the scene and who's in the room in one glanceable strip, catch up on
 * where the story left off, then Play — rather than a grid of
 * equal-weight, disconnected widgets.
 */
export function WorldStagingPage({
  worldId,
  world,
  isGm,
  onSessionNotesSaved,
}: WorldStagingPageProps) {
  return (
    <main className="grid w-full gap-6" data-testid="world-staging-page">
      <header className="grid gap-3">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Overview
          </p>
          <h1 className="text-3xl font-semibold">{world?.name ?? "World"}</h1>
        </div>

        {/* Toolbar row: a slim, subdued strip rather than a plain paragraph.
         * The Compendium/System settings links and the Play button formerly
         * here now live in WorldSectionShell (persistent sidebar + a single
         * top-right Play button shared by every world hub screen). */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg border border-border bg-card/50 px-4 py-2 text-sm text-muted-foreground">
          <span>
            Confirm the scene and roster below, then hand the screen to the
            game.
          </span>
        </div>
      </header>

      {/* The story so far, read before the roster check — catching up on
       * where things left off naturally comes before confirming who's in
       * the room for tonight. */}
      <Panel
        variant="parchment"
        className="rounded-xl border border-border"
        data-testid="session-notes-panel"
      >
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

      {/* Spec 023: the roster itself lives in its own Players sidebar
       * section now — this stays just the invite link. */}
      {isGm ? (
        <Panel
          variant="stone"
          className="grid gap-2 rounded-xl border border-border sm:max-w-xs"
        >
          <SessionSetupInviteLink worldId={worldId} />
        </Panel>
      ) : null}

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
