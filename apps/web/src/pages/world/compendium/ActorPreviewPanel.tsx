import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getWorldActorImages, type ActorImageRecord } from "@/api/actors";
import { portraitOf, tokenImageOf } from "@/pages/world/actor/actorImagery";
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
 * and performs no data fetching of its own (research.md §3) — except for its
 * imagery, which is a separate read by ADR-057's design.
 *
 * Spec 031 (FR-036): the portrait is shown as a face and the token as a token.
 * They are deliberately not interchangeable — the point of storing two images
 * is that a map token and a panel portrait are not the same picture.
 */
export function ActorPreviewPanel({
  worldId,
  actor,
  onClose,
}: ActorPreviewPanelProps) {
  const [imagesByActor, setImagesByActor] = useState<
    Record<string, ActorImageRecord[]>
  >({});

  useEffect(() => {
    let active = true;
    getWorldActorImages(worldId)
      .then((byActor) => {
        if (active) {
          setImagesByActor(byActor);
        }
      })
      .catch(() => {
        if (active) {
          setImagesByActor({});
        }
      });
    return () => {
      active = false;
    };
  }, [worldId]);

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
  const images = imagesByActor[actor.id];
  const portrait = portraitOf(images);
  const tokenImage = tokenImageOf(images);

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

      {portrait || tokenImage ? (
        <div className="flex items-start gap-3">
          {portrait ? (
            <img
              src={portrait.thumbnailUrl}
              alt={`Portrait of ${actor.label}`}
              className="h-28 w-20 rounded-md border border-border object-cover"
              data-testid="actor-preview-panel-portrait"
            />
          ) : null}
          {tokenImage ? (
            <img
              src={tokenImage.thumbnailUrl}
              alt={`Map token for ${actor.label}`}
              className="h-14 w-14 rounded-full border border-border object-cover"
              data-testid="actor-preview-panel-token"
            />
          ) : null}
        </div>
      ) : null}

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
        {canEdit && actor.isNpc ? (
          <Button
            asChild
            variant="secondary"
            size="sm"
            data-testid="actor-preview-panel-imagery"
          >
            <Link to={`/world/${worldId}/compendium/npc/${actor.id}/edit`}>
              Details &amp; imagery
            </Link>
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
