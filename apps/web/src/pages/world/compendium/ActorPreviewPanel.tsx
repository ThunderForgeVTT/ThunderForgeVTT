import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import type { WorldActorRecord } from "@/types/actor";

export interface ActorPreviewPanelProps {
  worldId: string;
  actor: WorldActorRecord | null;
  onClose: () => void;
}

/**
 * Spec 011 (FR-004/FR-005): the Compendium NPCs tab's row-select preview —
 * a compact, read-only summary of the selected actor docked next to the
 * table, with links out to the full view/edit route (contracts/compendium-npcs.md).
 * Presentation-only: it receives an already-resolved `WorldActorRecord`
 * and performs no data fetching of its own (research.md §3).
 */
export function ActorPreviewPanel({
  worldId,
  actor,
  onClose,
}: ActorPreviewPanelProps) {
  if (!actor) {
    return (
      <Card
        className="grid place-items-center p-6 text-center"
        data-testid="actor-preview-panel-empty"
      >
        <p className="text-sm text-muted-foreground">
          Select an NPC to preview it.
        </p>
      </Card>
    );
  }

  const canEdit = actor.myPermissionLevel !== "VIEWER";

  return (
    <Card className="grid gap-4 p-5" data-testid="actor-preview-panel">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            {actor.isNpc ? "NPC" : "Player Character"}
          </p>
          <h2 className="text-lg font-semibold">{actor.label}</h2>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label="Close preview"
          onClick={onClose}
          data-testid="actor-preview-panel-close"
        >
          ×
        </Button>
      </div>

      <div className="grid gap-2">
        <p className="text-sm text-muted-foreground">
          Classification:{" "}
          {actor.isNpc ? "Non-Player Character" : "Player Character"}
        </p>
        <p className="text-sm text-muted-foreground">Type: {actor.actorType}</p>
        {actor.gameSystemId ? (
          <p className="text-sm text-muted-foreground">
            Game system: {actor.gameSystemId}
          </p>
        ) : null}
        <p className="text-sm whitespace-pre-wrap">
          {actor.description || (
            <span className="text-muted-foreground italic">
              No description.
            </span>
          )}
        </p>
      </div>

      <div className="flex gap-2">
        <Button
          asChild
          variant="secondary"
          size="sm"
          data-testid="actor-preview-panel-view"
        >
          <Link to={`/world/${worldId}/actor/${actor.id}/view`}>View</Link>
        </Button>
        {canEdit ? (
          <Button
            asChild
            variant="secondary"
            size="sm"
            data-testid="actor-preview-panel-edit"
          >
            <Link to={`/world/${worldId}/actor/${actor.id}/edit`}>Edit</Link>
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
