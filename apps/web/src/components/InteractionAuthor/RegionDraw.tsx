import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Label } from "@/components/ui/label";

/**
 * Drawing the area a region covers.
 *
 * Spec 030, US5. A region is invisible to players *always* — it is not an
 * annotation, and it is not a shape. It exists to be crossed, and the only
 * person who ever needs to see one is the Game Master arranging the scene.
 *
 * That is why regions are carried on the interactive rather than stored in
 * `shapes`: a shape is something the table looks at, and mixing the two would
 * mean every shape query filtering out regions and `visibleToPlayers` doing
 * two unrelated jobs.
 *
 * # Why the numbers are typed rather than dragged
 *
 * Dragging a rectangle on the canvas belongs in the engine, which owns
 * everything spatial (Principle I). This panel is the Game Master's precise
 * control over what they drew — the place to nudge an edge by four pixels
 * without re-drawing the whole thing — and it is what makes a region editable
 * at all before canvas drawing exists for it.
 */

export interface RectRegion {
  shape: "rect";
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface RegionDrawProps {
  value: RectRegion | null;
  onChange: (region: RectRegion) => void;
}

const EMPTY: RectRegion = {
  shape: "rect",
  x: 0,
  y: 0,
  width: 100,
  height: 100,
};

export function RegionDraw({ value, onChange }: RegionDrawProps) {
  const [draft, setDraft] = useState<RectRegion>(value ?? EMPTY);

  const setField = useCallback(
    (key: keyof Omit<RectRegion, "shape">, raw: string) => {
      const parsed = Number(raw);
      // A field mid-edit is not a number, and coercing it to zero would
      // silently move the Game Master's region while they were typing.
      if (!Number.isFinite(parsed)) return;
      setDraft((previous) => ({ ...previous, [key]: parsed }));
    },
    [],
  );

  const encloses = draft.width > 0 && draft.height > 0;

  return (
    <Panel>
      <h3>Region</h3>
      <p>Only you can see this. Players are never shown a region.</p>

      {(["x", "y", "width", "height"] as const).map((key) => (
        <div key={key}>
          <Label htmlFor={`region-${key}`}>{key}</Label>
          <input
            id={`region-${key}`}
            type="number"
            value={draft[key]}
            onChange={(event) => setField(key, event.target.value)}
          />
        </div>
      ))}

      {!encloses && (
        <p role="status">
          A region with no width or height encloses nothing, so nothing could
          ever cross into it.
        </p>
      )}

      <Button disabled={!encloses} onClick={() => onChange(draft)}>
        Save the area
      </Button>
    </Panel>
  );
}
