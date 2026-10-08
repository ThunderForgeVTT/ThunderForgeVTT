import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import type { WorldStore } from "@/engine/world/store";
import type { WorldWall } from "@/engine/world/types";

export interface SelectionBarProps {
  worldStore: Pick<WorldStore, "dispatch">;
  isGm: boolean;
  walls: Record<string, WorldWall>;
  selectedTokenIds: readonly string[];
  selectedWallIds: readonly string[];
  selectedLightIds: readonly string[];
  selectedShapeIds: readonly string[];
}

function counted(count: number, one: string): string | null {
  if (count === 0) return null;
  return `${count} ${one}${count === 1 ? "" : "s"}`;
}

/**
 * What a group in Select holds, and the two things done to it at once
 * (spec 085 FR-018, research R11).
 *
 * It renders only while the group holds more than one item: one item has its
 * own panel. It only dispatches. **Hidden from the table** is the walls
 * bridge's `set_walls_hidden`, and **Delete** is `delete_group`, which the
 * engine hears and turns into the group's own deletes. So the bar's Delete
 * and the Delete key take one path, and the engine decides what a player may
 * delete.
 *
 * Holds no state, so a test can call it and read what it returns.
 */
export function SelectionBar({
  worldStore,
  isGm,
  walls,
  selectedTokenIds,
  selectedWallIds,
  selectedLightIds,
  selectedShapeIds,
}: SelectionBarProps) {
  const total =
    selectedTokenIds.length +
    selectedWallIds.length +
    selectedLightIds.length +
    selectedShapeIds.length;
  if (total < 2) return null;

  const parts = [
    counted(selectedTokenIds.length, "token"),
    counted(selectedWallIds.length, "wall"),
    counted(selectedLightIds.length, "light"),
    counted(selectedShapeIds.length, "shape"),
  ].filter((part): part is string => part !== null);

  const offerHidden = isGm && selectedWallIds.length > 0;
  const allHidden = selectedWallIds.every((id) => walls[id]?.secret === true);

  return (
    <div
      role="toolbar"
      aria-label="Selection"
      data-testid="selection-bar"
      className="pointer-events-auto flex items-center gap-3 rounded-md border border-border bg-background/95 px-3 py-1.5 text-sm shadow-md"
    >
      <span data-testid="selection-bar-counts" aria-live="polite">
        {parts.join(", ")}
      </span>
      {offerHidden ? (
        <div className="flex items-center gap-2">
          <Checkbox
            id="selection-bar-hidden"
            data-testid="selection-bar-hidden"
            checked={allHidden}
            onCheckedChange={(checked) =>
              worldStore.dispatch(
                {
                  type: "set_walls_hidden",
                  wallIds: [...selectedWallIds],
                  hidden: checked === true,
                },
                "ui",
              )
            }
          />
          <Label htmlFor="selection-bar-hidden">Hidden from the table</Label>
        </div>
      ) : null}
      <Button
        variant="danger"
        size="sm"
        data-testid="selection-bar-delete"
        onClick={() => worldStore.dispatch({ type: "delete_group" }, "ui")}
      >
        Delete
      </Button>
    </div>
  );
}
