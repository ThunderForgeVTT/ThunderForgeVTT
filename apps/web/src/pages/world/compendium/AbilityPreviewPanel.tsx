import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import type { WorldAbilityRecord } from "@/types/ability";
import {
  resolveAbilityLabel,
  toAbilityClassificationKey,
  type AbilityFacetsLookup,
} from "@/utils/abilityFacets";
import { effectTypeLabel } from "@/utils/effectLabels";

export interface AbilityPreviewPanelProps {
  worldId: string;
  ability: WorldAbilityRecord | null;
  /** The active system's `abilityFacets`, if it publishes any (FR-010). */
  facets?: AbilityFacetsLookup;
  onClose: () => void;
}


/**
 * Spec 025 (T026): the Abilities tab's row-select preview. Mirrors
 * `ItemPreviewPanel` — a compact read-only summary docked beside the table,
 * with links out to the full view/edit route. Presentation-only.
 *
 * The classification renders through the active system's facet label
 * (FR-012), and a GM-only ability is clearly marked (FR-024d). Effects stay
 * empty until US2 adds them.
 */
export function AbilityPreviewPanel({
  worldId,
  ability,
  facets,
  onClose,
}: AbilityPreviewPanelProps) {
  if (!ability) {
    return (
      <Card
        className="grid place-items-center p-6 text-center"
        data-testid="ability-preview-panel-empty"
      >
        <p className="text-sm text-muted-foreground">Select an ability to preview it.</p>
      </Card>
    );
  }

  const canEdit = ability.myPermissionLevel !== "VIEWER";
  const classificationLabel = resolveAbilityLabel(
    facets,
    toAbilityClassificationKey(ability.classification),
  );

  return (
    <Card className="grid gap-4 p-5" data-testid="ability-preview-panel">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            {classificationLabel}
          </p>
          <h2 className="text-lg font-semibold">
            {ability.name}
            {ability.gmOnly ? (
              <span
                className="ml-2 rounded bg-muted px-1.5 py-0.5 text-xs font-normal text-muted-foreground"
                data-testid="ability-preview-panel-gm-only"
                title="Hidden from players"
              >
                GM-only
              </span>
            ) : null}
          </h2>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label="Close preview"
          onClick={onClose}
          data-testid="ability-preview-panel-close"
        >
          ×
        </Button>
      </div>

      <div className="grid gap-2">
        <p className="text-sm whitespace-pre-wrap">
          {ability.description || (
            <span className="text-muted-foreground italic">No description.</span>
          )}
        </p>
      </div>

      {ability.effects.length > 0 ? (
        <div className="grid gap-1">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Effects
          </p>
          <ul className="grid gap-1 text-sm">
            {ability.effects.map((effect) => (
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
        <Button asChild variant="secondary" size="sm" data-testid="ability-preview-panel-view">
          <Link to={`/world/${worldId}/ability/${ability.id}/view`}>View</Link>
        </Button>
        {canEdit ? (
          <Button asChild variant="secondary" size="sm" data-testid="ability-preview-panel-edit">
            <Link to={`/world/${worldId}/ability/${ability.id}/edit`}>Edit</Link>
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
