import { useCallback, useEffect, useState } from "react";
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
import { setWallPrimitive } from "@/engine/bevy";
import { cn } from "@/lib/utils";
import type { WorldStore } from "@/engine/world/store";
import type { DoorState, WorldWall } from "@/engine/world/types";

export interface WallToolProps {
  worldStore: WorldStore;
  walls: Record<string, WorldWall>;
  selectedWallId: string | null;
}

/**
 * What a drag with the wall tool draws (spec 077 FR-011). The ids are the
 * engine's `WallPrimitive` names; Box is the engine's `room`.
 */
export const WALL_PRIMITIVES = [
  {
    id: "segment",
    label: "Segment",
    hint: "Drag a wall; click-click-Enter for a chain",
  },
  { id: "room", label: "Box", hint: "Drag corner to corner" },
  { id: "circle", label: "Circle", hint: "Drag from centre to radius" },
  {
    id: "door",
    label: "Door",
    hint: "Drag a door, or click a wall to make it one",
  },
] as const;

export type WallPrimitiveId = (typeof WALL_PRIMITIVES)[number]["id"];

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
 * Drawing itself (click-drag to create a wall on the canvas) is engine-side
 * (Bevy, T011-T012); this panel chooses *what* a drag draws — a segment, a
 * box, a circle or a door (spec 077 FR-011) — through `setWallPrimitive`,
 * and resets it to the segment when the tool closes so the next time the
 * tool opens a drag means what it meant the first time.
 */
export function WallTool({ worldStore, walls, selectedWallId }: WallToolProps) {
  const [primitive, setPrimitive] = useState<WallPrimitiveId>("segment");

  const selectedWall = selectedWallId ? walls[selectedWallId] : null;

  const choosePrimitive = useCallback((id: WallPrimitiveId) => {
    setPrimitive(id);
    void setWallPrimitive(id);
  }, []);

  useEffect(
    () => () => {
      void setWallPrimitive("segment");
    },
    [],
  );

  const updateSelectedWall = useCallback(
    (
      changes: Partial<
        Pick<WorldWall, "blocksVision" | "blocksMovement" | "doorState">
      >,
    ) => {
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

    worldStore.dispatch({ type: "delete_wall", wallId: selectedWall.id }, "ui");
    worldStore.dispatch({ type: "select_wall", wallId: null }, "ui");
  }, [selectedWall, worldStore]);

  return (
    <div className="grid gap-3" data-testid="wall-tool">
      <div
        role="radiogroup"
        aria-label="Wall shape"
        className="grid grid-cols-4 gap-1"
        data-testid="wall-primitive-picker"
      >
        {WALL_PRIMITIVES.map((option) => {
          const active = option.id === primitive;
          return (
            <button
              key={option.id}
              type="button"
              role="radio"
              aria-checked={active}
              title={option.hint}
              data-testid={`wall-primitive-${option.id}`}
              onClick={() => choosePrimitive(option.id)}
              className={cn(
                "rounded-md px-1 py-1.5 text-xs font-medium transition-colors",
                active
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:text-foreground",
              )}
            >
              {option.label}
            </button>
          );
        })}
      </div>

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

          {/* Spec 085: any wall, door or not. A hidden wall still blocks
              sight and movement; it is only not drawn for the players. */}
          <div className="flex items-center gap-2">
            <Checkbox
              id="wall-hidden"
              data-testid="wall-hidden-toggle"
              checked={selectedWall.secret === true}
              onCheckedChange={(checked) =>
                worldStore.dispatch(
                  {
                    type: "set_walls_hidden",
                    wallIds: [selectedWall.id],
                    hidden: checked === true,
                  },
                  "ui",
                )
              }
            />
            <Label htmlFor="wall-hidden">Hidden from the table</Label>
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

          <Button
            type="button"
            variant="danger"
            icon="trash"
            onClick={deleteSelectedWall}
          >
            Delete wall
          </Button>
        </Panel>
      ) : null}
    </div>
  );
}
