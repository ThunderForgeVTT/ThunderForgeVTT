import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { WorldStore } from "@/engine/world/store";
import type { WorldLight, WorldToken } from "@/engine/world/types";

export interface LightingToolProps {
  worldStore: WorldStore;
  lights: Record<string, WorldLight>;
  selectedLightId: string | null;
  tokens: Record<string, WorldToken>;
}

const NO_TOKEN_VALUE = "__none__";
const DEFAULT_LIGHT_COLOR = "#ffcc66";

/**
 * LightingTool: canvas toolbar button that toggles "place light" mode,
 * plus a property panel for the currently selected light source (radius,
 * intensity, color, cast-shadows toggle, attach-to-token), per
 * specs/001-bevy-canvas-authoring T044.
 *
 * GM-only: the caller (WorldPage) is responsible for only rendering this
 * component for the scene owner (FR-009 — players never see authoring
 * tools). This component itself renders unconditionally once mounted, so
 * it must never be mounted for a non-owner session.
 *
 * Placement itself (click on the canvas to drop a light source) is
 * implemented engine-side (Bevy), mirroring WallTool's "draw mode"
 * pattern exactly: toggling "place mode" here only signals intent via
 * local UI state today, ready to be observed by the engine bridge once
 * that lands.
 */
export function LightingTool({
  worldStore,
  lights,
  selectedLightId,
  tokens,
}: LightingToolProps) {
  const [placeMode, setPlaceMode] = useState(false);

  const selectedLight = selectedLightId ? lights[selectedLightId] : null;

  const togglePlaceMode = useCallback(() => {
    setPlaceMode((active) => !active);
  }, []);

  const updateSelectedLight = useCallback(
    (
      changes: Partial<
        Pick<
          WorldLight,
          "radius" | "intensity" | "color" | "castsShadows" | "attachedTokenId"
        >
      >,
    ) => {
      if (!selectedLight) {
        return;
      }

      worldStore.dispatch(
        {
          type: "update_light",
          lightId: selectedLight.id,
          changes,
        },
        "ui",
      );
    },
    [selectedLight, worldStore],
  );

  const deleteSelectedLight = useCallback(() => {
    if (!selectedLight) {
      return;
    }

    worldStore.dispatch(
      { type: "delete_light", lightId: selectedLight.id },
      "ui",
    );
    worldStore.dispatch({ type: "select_light", lightId: null }, "ui");
  }, [selectedLight, worldStore]);

  const tokenOptions = Object.values(tokens);

  return (
    <div className="grid gap-3" data-testid="lighting-tool">
      <Button
        type="button"
        variant={placeMode ? "primary" : "secondary"}
        icon="torch"
        onClick={togglePlaceMode}
        aria-pressed={placeMode}
      >
        {placeMode ? "Placing lights" : "Place light"}
      </Button>

      {selectedLight ? (
        <Panel variant="stone" className="grid gap-3">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Selected light
          </p>

          <div className="grid gap-1.5">
            <Label htmlFor="light-radius">Radius</Label>
            <Input
              id="light-radius"
              type="number"
              min={0}
              step={1}
              value={selectedLight.radius}
              onChange={(event) =>
                updateSelectedLight({ radius: Number(event.target.value) })
              }
            />
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="light-intensity">Intensity</Label>
            <Input
              id="light-intensity"
              type="number"
              min={0}
              step={0.1}
              value={selectedLight.intensity}
              onChange={(event) =>
                updateSelectedLight({ intensity: Number(event.target.value) })
              }
            />
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="light-color">Color</Label>
            <Input
              id="light-color"
              type="color"
              value={selectedLight.color ?? DEFAULT_LIGHT_COLOR}
              onChange={(event) =>
                updateSelectedLight({ color: event.target.value })
              }
            />
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="light-casts-shadows"
              checked={selectedLight.castsShadows}
              onCheckedChange={(checked) =>
                updateSelectedLight({ castsShadows: checked === true })
              }
            />
            <Label htmlFor="light-casts-shadows">Casts shadows</Label>
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="light-attached-token">Attach to token</Label>
            <Select
              value={selectedLight.attachedTokenId ?? NO_TOKEN_VALUE}
              onValueChange={(value) =>
                updateSelectedLight({
                  attachedTokenId: value === NO_TOKEN_VALUE ? null : value,
                })
              }
            >
              <SelectTrigger id="light-attached-token">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_TOKEN_VALUE}>Not attached</SelectItem>
                {tokenOptions.map((token) => (
                  <SelectItem key={token.id} value={token.id}>
                    {token.label ?? token.id}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <Button
            type="button"
            variant="danger"
            icon="trash"
            onClick={deleteSelectedLight}
          >
            Delete light
          </Button>
        </Panel>
      ) : null}
    </div>
  );
}
