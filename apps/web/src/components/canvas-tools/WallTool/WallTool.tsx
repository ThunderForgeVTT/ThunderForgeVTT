import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { WorldStore } from "@/engine/world/store";
import type { DoorState, WorldWall } from "@/engine/world/types";

export interface WallToolProps {
  worldStore: WorldStore;
  walls: Record<string, WorldWall>;
  selectedWallId: string | null;
}

const DOOR_STATE_OPTIONS: { value: DoorState; label: string }[] = [
  { value: "none", label: "Not a door" },
  { value: "open", label: "Door (open)" },
  { value: "closed", label: "Door (closed)" },
];

/**
 * WallTool: canvas toolbar button that toggles "draw wall" mode, plus a
 * property panel for the currently selected wall (blocks vision / blocks
 * movement / door state), per specs/001-bevy-canvas-authoring T021.
 *
 * GM-only: the caller (WorldPage) is responsible for only rendering this
 * component for the scene owner (FR-009 — players never see authoring
 * tools). This component itself renders unconditionally once mounted, so
 * it must never be mounted for a non-owner session.
 *
 * Drawing itself (click-drag to create a wall segment on the canvas) is
 * implemented engine-side (Bevy, T011-T012) and is out of this
 * component's scope; toggling "draw mode" here only signals intent by
 * dispatching a `set_wall_draw_mode`-shaped local UI state today, ready
 * to be observed by the engine bridge once that lands.
 */
export function WallTool({ worldStore, walls, selectedWallId }: WallToolProps) {
  const [drawMode, setDrawMode] = useState(false);

  const selectedWall = selectedWallId ? walls[selectedWallId] : null;

  const toggleDrawMode = useCallback(() => {
    setDrawMode((active) => !active);
  }, []);

  const updateSelectedWall = useCallback(
    (changes: Partial<Pick<WorldWall, "blocksVision" | "blocksMovement" | "doorState">>) => {
      if (!selectedWall) {
        return;
      }

      worldStore.dispatch(
        {
          type: "update_wall",
          wallId: selectedWall.id,
          changes,
        },
        "ui",
      );
    },
    [selectedWall, worldStore],
  );

  const deleteSelectedWall = useCallback(() => {
    if (!selectedWall) {
      return;
    }

    worldStore.dispatch(
      { type: "delete_wall", wallId: selectedWall.id },
      "ui",
    );
    worldStore.dispatch({ type: "select_wall", wallId: null }, "ui");
  }, [selectedWall, worldStore]);

  return (
    <div className="grid gap-3" data-testid="wall-tool">
      <Button
        type="button"
        variant={drawMode ? "primary" : "secondary"}
        icon="shield"
        onClick={toggleDrawMode}
        aria-pressed={drawMode}
      >
        {drawMode ? "Drawing walls" : "Draw wall"}
      </Button>

      {selectedWall ? (
        <Panel variant="stone" className="grid gap-3">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Selected wall
          </p>

          <div className="flex items-center gap-2">
            <Checkbox
              id="wall-blocks-vision"
              checked={selectedWall.blocksVision}
              onCheckedChange={(checked) =>
                updateSelectedWall({ blocksVision: checked === true })
              }
            />
            <Label htmlFor="wall-blocks-vision">Blocks vision</Label>
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="wall-blocks-movement"
              checked={selectedWall.blocksMovement}
              onCheckedChange={(checked) =>
                updateSelectedWall({ blocksMovement: checked === true })
              }
            />
            <Label htmlFor="wall-blocks-movement">Blocks movement</Label>
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="wall-door-state">Door</Label>
            <Select
              value={selectedWall.doorState}
              onValueChange={(value) =>
                updateSelectedWall({ doorState: value as DoorState })
              }
            >
              <SelectTrigger id="wall-door-state">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {DOOR_STATE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <Button type="button" variant="danger" icon="trash" onClick={deleteSelectedWall}>
            Delete wall
          </Button>
        </Panel>
      ) : null}
    </div>
  );
}
