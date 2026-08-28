import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import type { WorldItemRecord } from "@/types/item";
import { effectTypeLabel } from "@/utils/effectLabels";

export interface ItemPreviewPanelProps {
  worldId: string;
  item: WorldItemRecord | null;
  onClose: () => void;
}

/**
 * Spec 013 (T025): the Compendium Items tab's row-select preview, mirrors
 * ActorPreviewPanel.tsx — a compact read-only summary docked next to the
 * table, with links out to the full view/edit route. Presentation-only.
 */
export function ItemPreviewPanel({
  worldId,
  item,
  onClose,
}: ItemPreviewPanelProps) {
  if (!item) {
    return (
      <Card
        className="grid place-items-center p-6 text-center"
        data-testid="item-preview-panel-empty"
      >
        <p className="text-sm text-muted-foreground">
          Select an item to preview it.
        </p>
      </Card>
    );
  }

  const canEdit = item.myPermissionLevel !== "VIEWER";

  return (
    <Card className="grid gap-4 p-5" data-testid="item-preview-panel">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Item
          </p>
          <h2 className="text-lg font-semibold">{item.name}</h2>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label="Close preview"
          onClick={onClose}
          data-testid="item-preview-panel-close"
        >
          ×
        </Button>
      </div>

      <div className="grid gap-2">
        <p className="text-sm whitespace-pre-wrap">
          {item.description || (
            <span className="text-muted-foreground italic">
              No description.
            </span>
          )}
        </p>
      </div>

      {item.effects.length > 0 ? (
        <div className="grid gap-1">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Effects
          </p>
          <ul className="grid gap-1 text-sm">
            {item.effects.map((effect) => (
              <li key={effect.id} className="text-muted-foreground">
                <span className="font-medium text-foreground">
                  {effectTypeLabel(effect.effectType)}
                </span>{" "}
                — {effect.formula} → {effect.target}
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground italic">No effects.</p>
      )}

      <div className="flex gap-2">
        <Button
          asChild
          variant="secondary"
          size="sm"
          data-testid="item-preview-panel-view"
        >
          <Link to={`/world/${worldId}/item/${item.id}/view`}>View</Link>
        </Button>
        {canEdit ? (
          <Button
            asChild
            variant="secondary"
            size="sm"
            data-testid="item-preview-panel-edit"
          >
            <Link to={`/world/${worldId}/item/${item.id}/edit`}>Edit</Link>
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
