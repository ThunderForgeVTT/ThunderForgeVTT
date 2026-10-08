import type { RollFacetRecord } from "@/types/roll";

/** `selected` with `id` added or removed, in the order the system declares. */
export function toggleProperty(
  declared: RollFacetRecord[],
  selected: string[],
  id: string,
  on: boolean,
): string[] {
  const next = new Set(selected);
  if (on) next.add(id);
  else next.delete(id);
  return declared.map((p) => p.id).filter((known) => next.has(known));
}

export interface ItemPropertyChecklistProps {
  /** What the world's system declares (`systemItemProperties`). */
  declared: RollFacetRecord[];
  selected: string[];
  disabled: boolean;
  onChange: (properties: string[]) => void;
}

/**
 * Spec 084 research R6: the properties an item is marked with, one checkbox
 * per property its system declares. Which property does what is the pack's
 * business (5e's Great Weapon Fighting reads Two-Handed); this only says
 * which ones the item has. Renders nothing for a system that declares none.
 */
export function ItemPropertyChecklist({
  declared,
  selected,
  disabled,
  onChange,
}: ItemPropertyChecklistProps) {
  if (declared.length === 0) return null;
  return (
    <fieldset className="grid gap-1 text-sm" data-testid="item-properties">
      <legend className="mb-1">Properties</legend>
      <div className="flex flex-wrap gap-x-4 gap-y-1">
        {declared.map((property) => (
          <label key={property.id} className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={selected.includes(property.id)}
              disabled={disabled}
              onChange={(e) =>
                onChange(
                  toggleProperty(
                    declared,
                    selected,
                    property.id,
                    e.target.checked,
                  ),
                )
              }
              data-testid={`item-property-${property.id}`}
            />
            {property.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}
