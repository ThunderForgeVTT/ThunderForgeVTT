import type { ReactNode } from "react";
import type { BaseMap } from "@/api/baseMaps";
import { FullMapCredit } from "@/components/world/MapCredit";

/**
 * Which map a new world's Starting Scene opens on (spec 088 FR-025, FR-027).
 *
 * Native radio inputs, one per map plus **None**, so arrow keys move the
 * choice and a screen reader announces a group of options with no extra
 * wiring. Each input sits inside its card's label, so the whole card is the
 * target, and the card shows its focus ring from the input's own focus.
 *
 * The maps share one author and one licence, so the credit is shown once,
 * under the group, rather than on every card.
 */
export function BaseMapPicker({
  maps,
  value,
  onChange,
}: {
  maps: BaseMap[];
  /** The chosen map's id, or `null` for **None**. */
  value: string | null;
  onChange: (id: string | null) => void;
}) {
  const credit = maps[0]?.credit;
  return (
    <fieldset className="grid gap-3" data-testid="base-map-picker">
      <legend className="mb-2 text-sm font-medium">Starting map</legend>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        {maps.map((map) => (
          <Card
            key={map.id}
            id={map.id}
            label={map.name}
            detail={`${Math.round(map.width / map.gridSize)} × ${Math.round(map.height / map.gridSize)} squares`}
            checked={value === map.id}
            onSelect={() => onChange(map.id)}
          >
            <img
              src={map.thumbnailUrl}
              alt=""
              // The name is the card's label; describing the picture as well
              // would announce each map twice.
              loading="lazy"
              className="aspect-[4/3] w-full rounded object-cover"
            />
          </Card>
        ))}
        <Card
          id="none"
          label="None"
          detail="A blank scene"
          checked={value === null}
          onSelect={() => onChange(null)}
        >
          <div className="grid aspect-[4/3] w-full place-items-center rounded border border-dashed border-border text-xs text-muted-foreground">
            No map
          </div>
        </Card>
      </div>
      {credit ? (
        <FullMapCredit
          credit={credit}
          className="text-xs text-muted-foreground"
        />
      ) : null}
    </fieldset>
  );
}

function Card({
  id,
  label,
  detail,
  checked,
  onSelect,
  children,
}: {
  id: string;
  label: string;
  detail: string;
  checked: boolean;
  onSelect: () => void;
  children: ReactNode;
}) {
  return (
    <label
      className="grid cursor-pointer gap-1 rounded-lg border border-border p-2 text-sm has-[:checked]:border-primary has-[:checked]:ring-2 has-[:checked]:ring-primary has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-ring"
      data-testid={`base-map-option-${id}`}
    >
      <input
        type="radio"
        name="base-map"
        value={id}
        checked={checked}
        onChange={onSelect}
        className="sr-only"
      />
      {children}
      <span className="font-medium">{label}</span>
      <span className="text-xs text-muted-foreground">{detail}</span>
    </label>
  );
}
