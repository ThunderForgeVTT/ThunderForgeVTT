import { Button } from "@thunderforge/host";
import { DND5E_SHEET_FACETS } from "../derived-data.ts";
import { hintClass, sectionHeadingClass } from "./styles";

/**
 * Spec 084: the facets that shape this character's rolls, ticked on the
 * sheet. The server reads them when it rolls; the sheet only records them.
 *
 * With Lucky ticked, the Luck Points left show beside a Reset for the long
 * rest that gives them back.
 */
export default function RollFacetsSection({
  facets,
  luckPointsLeft,
  canEdit,
  disabled,
  onToggle,
  onResetLuck,
}: {
  facets: string[];
  luckPointsLeft: number;
  canEdit: boolean;
  disabled: boolean;
  onToggle: (id: string) => void;
  onResetLuck: () => void;
}) {
  const lucky = facets.includes("lucky");
  return (
    <div className="grid gap-2" data-testid="dnd5e-roll-facets">
      <div className={sectionHeadingClass}>Roll facets</div>
      <div className="flex flex-wrap gap-x-4 gap-y-2">
        {DND5E_SHEET_FACETS.map((facet) => {
          const id = `dnd5e-roll-facet-${facet.id}`;
          const ticked = facets.includes(facet.id);
          return canEdit ? (
            <label
              key={facet.id}
              htmlFor={id}
              className="flex items-center gap-2 text-sm"
            >
              <input
                id={id}
                type="checkbox"
                className="h-5 w-5"
                data-testid={`roll-facet-${facet.id}`}
                checked={ticked}
                disabled={disabled}
                onChange={() => onToggle(facet.id)}
              />
              {facet.label}
            </label>
          ) : (
            <span
              key={facet.id}
              className="text-sm"
              data-testid={`roll-facet-${facet.id}`}
            >
              {facet.label}: {ticked ? "yes" : "no"}
            </span>
          );
        })}
      </div>
      {lucky ? (
        <div className="flex items-center gap-2">
          <span className={hintClass} data-testid="dnd5e-luck-points">
            Luck Points left: {luckPointsLeft}
          </span>
          {canEdit ? (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              data-testid="dnd5e-luck-reset"
              disabled={disabled}
              onClick={onResetLuck}
            >
              Reset
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
