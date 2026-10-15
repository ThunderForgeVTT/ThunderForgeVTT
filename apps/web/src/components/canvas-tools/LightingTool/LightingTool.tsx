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
import {
  AMBIENT_LEVELS,
  type AmbientLevel,
} from "@/engine/world/sync/sceneLighting";
import type { WorldLight, WorldToken } from "@/engine/world/types";
import type { SceneUnits } from "@/types/light";
import { LightReachFields } from "./LightReachFields";
import { DEFAULT_SCENE_UNITS } from "./sceneUnits";

export interface LightingToolProps {
  worldStore: WorldStore;
  lights: Record<string, WorldLight>;
  selectedLightId: string | null;
  tokens: Record<string, WorldToken>;
  /** Playtest 2026-09-10 P9: the scene's baseline light. */
  ambientLight?: AmbientLevel;
  /** Set the scene's light; the control is hidden without it. */
  onAmbientLightChange?: (level: AmbientLevel) => void;
  /** Spec 099 US5: the scene's hour, or `null` for no clock. */
  timeOfDay?: number | null;
  /** Set the scene's hour, over `overSecs` when given; hidden without it. */
  onTimeOfDayChange?: (hour: number | null, overSecs?: number) => void;
  /**
   * Spec 045 US7: whether this scene remembers where players have been.
   *
   * Beside the scene's light because it is the same question from the
   * player's chair — what of this board can I see, and what do I remember
   * seeing. Splitting them across two tools would make a Game Master hunt.
   */
  explorationEnabled?: boolean;
  onExplorationChange?: (enabled: boolean) => void;
  /** Reset the fog. `null` means everyone; a user id means one player. */
  onExplorationReset?: (forUser: string | null) => void;
  /** Who is at the table, for a reset aimed at one of them. */
  players?: { userId: string; name: string }[];
  /**
   * What this scene's distances are measured in, so a light's reach is set
   * in the system's units (spec 045 FR-061). Five-foot squares until known.
   */
  units?: SceneUnits;
}

const NO_TOKEN_VALUE = "__none__";
const DEFAULT_LIGHT_COLOR = "#ffcc66";

const AMBIENT_LABELS: Record<AmbientLevel, string> = {
  bright: "Bright",
  dim: "Dim",
  dark: "Dark",
};

/** 22.5 → "22:30", for the time input. */
function hourToClock(hour: number): string {
  const minutes = Math.round(hour * 60) % (24 * 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

/** "22:30" → 22.5; anything unreadable is `null`. */
function clockToHour(clock: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(clock);
  if (!match) return null;
  const hour = Number(match[1]) + Number(match[2]) / 60;
  return hour < 24 ? hour : null;
}

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
  ambientLight = "bright",
  onAmbientLightChange,
  timeOfDay = null,
  onTimeOfDayChange,
  explorationEnabled = false,
  onExplorationChange,
  onExplorationReset,
  players = [],
  units = DEFAULT_SCENE_UNITS,
}: LightingToolProps) {
  const [placeMode, setPlaceMode] = useState(false);
  const [clockDraft, setClockDraft] = useState<string | null>(null);
  const [passSecs, setPassSecs] = useState("0");
  const clockValue =
    clockDraft ?? (timeOfDay === null ? "" : hourToClock(timeOfDay));
  const draftHour = clockToHour(clockValue);

  const selectedLight = selectedLightId ? lights[selectedLightId] : null;

  const togglePlaceMode = useCallback(() => {
    setPlaceMode((active) => !active);
  }, []);

  const updateSelectedLight = useCallback(
    (
      changes: Partial<
        Pick<
          WorldLight,
          | "radius"
          | "brightRadius"
          | "intensity"
          | "color"
          | "castsShadows"
          | "attachedTokenId"
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
      {onAmbientLightChange ? (
        <div className="grid gap-1.5">
          <p
            id="scene-ambient-label"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Scene light
          </p>
          <div
            role="group"
            aria-labelledby="scene-ambient-label"
            className="flex gap-1"
            data-testid="scene-ambient"
          >
            {AMBIENT_LEVELS.map((level) => (
              <Button
                key={level}
                type="button"
                size="sm"
                variant={ambientLight === level ? "primary" : "secondary"}
                aria-pressed={ambientLight === level}
                onClick={() => onAmbientLightChange(level)}
                data-testid={`scene-ambient-${level}`}
              >
                {AMBIENT_LABELS[level]}
              </Button>
            ))}
          </div>
          <p className="text-xs text-muted-foreground">
            Walls cast shadows in dim and dark scenes.
          </p>
        </div>
      ) : null}

      {onTimeOfDayChange ? (
        <div className="grid gap-1.5" data-testid="scene-time">
          <p
            id="scene-time-label"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Time of day
          </p>
          <div className="flex items-end gap-1">
            <div className="grid gap-1">
              <Label htmlFor="scene-time-hour" className="text-xs">
                Hour
              </Label>
              <Input
                id="scene-time-hour"
                type="time"
                value={clockValue}
                onChange={(event) => setClockDraft(event.target.value)}
                data-testid="scene-time-hour"
              />
            </div>
            <div className="grid gap-1">
              <Label htmlFor="scene-time-over" className="text-xs">
                Over (secs)
              </Label>
              <Input
                id="scene-time-over"
                type="number"
                min={0}
                max={600}
                value={passSecs}
                onChange={(event) => setPassSecs(event.target.value)}
                className="w-20"
                data-testid="scene-time-over"
              />
            </div>
          </div>
          <div className="flex gap-1">
            <Button
              type="button"
              size="sm"
              disabled={draftHour === null}
              onClick={() => {
                if (draftHour === null) return;
                const over = Math.min(600, Math.max(0, Number(passSecs) || 0));
                onTimeOfDayChange(draftHour, over > 0 ? over : undefined);
                setClockDraft(null);
              }}
              data-testid="scene-time-set"
            >
              Set time
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={timeOfDay === null}
              onClick={() => {
                onTimeOfDayChange(null);
                setClockDraft(null);
              }}
              data-testid="scene-time-clear"
            >
              No clock
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            {timeOfDay === null
              ? "No clock: the scene light above rules."
              : `The table's clock reads ${hourToClock(timeOfDay)}.`}
          </p>
        </div>
      ) : null}

      {onExplorationChange ? (
        <div className="grid gap-1.5" data-testid="scene-exploration">
          <p
            id="scene-exploration-label"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Explored areas
          </p>
          <Button
            type="button"
            size="sm"
            variant={explorationEnabled ? "primary" : "secondary"}
            aria-pressed={explorationEnabled}
            aria-labelledby="scene-exploration-label"
            onClick={() => onExplorationChange(!explorationEnabled)}
            data-testid="scene-exploration-toggle"
          >
            {explorationEnabled ? "Remembering" : "Not remembering"}
          </Button>
          <p className="text-xs text-muted-foreground">
            Each player keeps their own map, in their own browser.
          </p>

          {explorationEnabled && onExplorationReset ? (
            <div className="grid gap-1">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => onExplorationReset(null)}
                data-testid="scene-exploration-reset-all"
              >
                Reset for everyone
              </Button>
              {players.map((player) => (
                <Button
                  key={player.userId}
                  type="button"
                  size="sm"
                  variant="secondary"
                  onClick={() => onExplorationReset(player.userId)}
                  data-testid={`scene-exploration-reset-${player.userId}`}
                >
                  Reset for {player.name}
                </Button>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}

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

          <LightReachFields
            lightId={selectedLight.id}
            radius={selectedLight.radius}
            brightRadius={selectedLight.brightRadius}
            units={units}
            onChange={updateSelectedLight}
          />

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
