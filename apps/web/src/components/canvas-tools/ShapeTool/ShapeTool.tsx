import { useCallback, useEffect, useState, type RefObject } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { getCameraState, setActiveShapeTool } from "@/engine/bevy";
import { screenToWorld } from "@/engine/bevy/screenToWorld";
import type { WorldStore } from "@/engine/world/store";
import type { ShapeKind, WorldShape } from "@/engine/world/types";

export interface ShapeToolProps {
  worldStore: WorldStore;
  shapes: Record<string, WorldShape>;
  selectedShapeId: string | null;
  sceneId: string | null;
  /**
   * Ref to the canvas container element (the div useCanvasEngine mounts
   * Bevy's canvas into on WorldPage). Used only by the text sub-tool: the
   * engine doesn't handle in-canvas text input (a DOM/UI concern), so
   * this component listens for a click on the container itself while the
   * text tool is active, and opens a small popover to collect the text
   * value at that position. All other sub-tools (freehand/rect/ellipse/
   * line) draw entirely engine-side, exactly like WallTool's draw mode.
   */
  canvasContainerRef?: RefObject<HTMLDivElement | null>;
  /**
   * Whether the viewer is the Game Master. Only a Game Master hides a
   * drawing from the players; a player's drawing is always on every board
   * (spec 082 FR-015), so a player is not offered the toggle.
   */
  isGm?: boolean;
}

type DrawTool = "none" | "freehand" | "rect" | "ellipse" | "line" | "text";

const DRAW_TOOLS: { value: DrawTool; label: string }[] = [
  { value: "freehand", label: "Freehand" },
  { value: "rect", label: "Rectangle" },
  { value: "ellipse", label: "Ellipse" },
  { value: "line", label: "Line/Arrow" },
  { value: "text", label: "Text" },
];

/**
 * The colours a shape can be — the engine's own palette, in its order
 * (`systems/shape.rs`'s `COLOR_PALETTE`). A shape's colour is stored as an
 * index into it (`style.colorIndex`), which is what the engine reads. The old
 * free colour picker wrote `style.color`, which nothing read, so changing a
 * colour did nothing (playtest 2026-09-10 P10).
 */
const SHAPE_COLORS: { name: string; hex: string }[] = [
  { name: "Blue", hex: "#66bff2" },
  { name: "Red", hex: "#e64d4d" },
  { name: "Green", hex: "#4de666" },
  { name: "Amber", hex: "#f2b233" },
  { name: "Violet", hex: "#cc66e6" },
];

type TextPlacement = {
  x: number;
  y: number;
};

/**
 * ShapeTool: canvas toolbar with five sub-tool buttons (freehand, rect,
 * ellipse, line/arrow, text) that set the active drawing tool, plus a
 * small style panel for the currently selected shape (color, "visible to
 * players" toggle), per specs/001-bevy-canvas-authoring T059.
 *
 * The Game Master and any player holding the Shapes tool see it (spec 082);
 * WorldPage renders it from the viewer's rail. A player edits only their own
 * drawings — the engine offers no handle on anybody else's, and the server
 * refuses the write regardless.
 *
 * Drawing itself (freehand strokes, rect/ellipse/line click-drag) is
 * implemented engine-side (Bevy): selecting a sub-tool here only signals
 * intent via local UI state today, ready to be observed by the engine
 * bridge once that lands — same placeholder pattern as WallTool's draw
 * mode toggle. The text tool is the one exception: since the engine
 * doesn't handle in-canvas text input, this component itself listens for
 * a click on the canvas container while "text" is active, and dispatches
 * `create_shape` (kind text) into the store itself rather than waiting on
 * an engine-emitted event; the shape bridge sends it (spec 082 FR-014).
 */
export function ShapeTool({
  worldStore,
  shapes,
  selectedShapeId,
  sceneId,
  canvasContainerRef,
  isGm = false,
}: ShapeToolProps) {
  const [activeTool, setActiveTool] = useState<DrawTool>("none");
  const [textPlacement, setTextPlacement] = useState<TextPlacement | null>(
    null,
  );
  const [textValue, setTextValue] = useState("");
  const [textVisibleToPlayers, setTextVisibleToPlayers] = useState(false);

  const selectedShape = selectedShapeId ? shapes[selectedShapeId] : null;

  const toggleTool = useCallback(
    (tool: DrawTool) => {
      const next = activeTool === tool ? "none" : tool;
      setActiveTool(next);
      setTextPlacement(null);
      // Playtest 2026-09-10 P10: the engine decides what a drag draws, and
      // these buttons never used to tell it — so a drag after "Rectangle"
      // drew nothing. "text" disarms the engine: text is placed here.
      void setActiveShapeTool(next);
    },
    [activeTool],
  );

  // Leaving the Shapes panel leaves nothing armed in the engine either.
  useEffect(() => {
    return () => {
      void setActiveShapeTool("none");
    };
  }, []);

  // Listen for a click directly on the canvas container while the text
  // tool is active, so the GM can place a text shape at the clicked
  // position (the one sub-tool this component handles itself rather than
  // deferring to the engine, per T059's coordination note).
  useEffect(() => {
    if (activeTool !== "text" || !sceneId) {
      return;
    }

    const container = canvasContainerRef?.current;
    if (!container) {
      return;
    }

    const onClick = (event: MouseEvent) => {
      // Only clicks on the map itself place text — not on the tool rail or
      // the dock, which are elsewhere in the document this listens on.
      const target = event.target as Node | null;
      const onMap =
        target instanceof HTMLCanvasElement ||
        (target !== null && container.contains(target));
      if (!onMap) {
        return;
      }
      // The map point under the click, not screen pixels: the engine's world
      // is centred on the camera and grows upward, so container-relative
      // pixels put text away from the click and mirrored (P10).
      const canvas =
        target instanceof HTMLCanvasElement
          ? target
          : document.querySelector("canvas");
      const box = (canvas ?? container).getBoundingClientRect();
      const client = { x: event.clientX, y: event.clientY };
      void getCameraState().then((camera) => {
        if (!camera) return;
        setTextPlacement(screenToWorld(client, box, camera));
        setTextValue("");
      });
    };

    // Listened for on the document rather than on `container` itself,
    // because a click on the map never reaches the container: Bevy/winit
    // inserts its real <canvas> as a direct child of <body> and WorldPage
    // gives it `position: fixed; inset: 0` (see WorldPage.tsx's
    // canvasSelector comment), so the canvas covers the container without
    // ever being inside it. Bound to the container, this listener fired for
    // no click a GM could make and the text sub-tool simply did nothing —
    // the only sub-tool whose placement is handled here in the DOM rather
    // than engine-side, so nothing else masked it.
    document.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("click", onClick);
    };
  }, [activeTool, canvasContainerRef, sceneId]);

  const submitText = useCallback(() => {
    if (!textPlacement || !sceneId || !textValue.trim()) {
      return;
    }

    // Into the store like every other drawing: the shape bridge sends the
    // mutation and the confirmed shape comes back as `upsert_shape`.
    worldStore.dispatch(
      {
        type: "create_shape",
        shape: {
          kind: "text",
          geometry: { x: textPlacement.x, y: textPlacement.y },
          text: textValue.trim(),
          visibleToPlayers: isGm ? textVisibleToPlayers : true,
        },
      },
      "ui",
    );
    setTextPlacement(null);
    setTextValue("");
  }, [
    isGm,
    sceneId,
    textPlacement,
    textValue,
    textVisibleToPlayers,
    worldStore,
  ]);

  const updateSelectedShape = useCallback(
    (changes: Partial<Pick<WorldShape, "visibleToPlayers" | "style">>) => {
      if (!selectedShape) {
        return;
      }

      worldStore.dispatch(
        {
          type: "update_shape",
          shapeId: selectedShape.id,
          changes,
        },
        "ui",
      );
    },
    [selectedShape, worldStore],
  );

  const updateSelectedShapeColor = useCallback(
    (colorIndex: number) => {
      if (!selectedShape) {
        return;
      }

      updateSelectedShape({
        style: { ...(selectedShape.style ?? {}), colorIndex },
      });
    },
    [selectedShape, updateSelectedShape],
  );

  const deleteSelectedShape = useCallback(() => {
    if (!selectedShape) {
      return;
    }

    worldStore.dispatch(
      { type: "delete_shape", shapeId: selectedShape.id },
      "ui",
    );
    worldStore.dispatch({ type: "select_shape", shapeId: null }, "ui");
  }, [selectedShape, worldStore]);

  const rawColorIndex = selectedShape?.style?.colorIndex;
  const selectedColorIndex =
    typeof rawColorIndex === "number" ? rawColorIndex : null;

  return (
    <div className="grid gap-3" data-testid="shape-tool">
      <div
        className="grid grid-cols-1 gap-1.5"
        data-testid="shape-tool-buttons"
      >
        {DRAW_TOOLS.map((tool) => (
          <Button
            key={tool.value}
            type="button"
            variant={activeTool === tool.value ? "primary" : "secondary"}
            onClick={() => toggleTool(tool.value)}
            aria-pressed={activeTool === tool.value}
          >
            {tool.label}
          </Button>
        ))}
      </div>

      {activeTool === "text" && textPlacement ? (
        <Panel
          variant="parchment"
          className="grid gap-2 p-3"
          data-testid="shape-text-popover"
        >
          <p className="text-xs text-muted-foreground">
            Placing text at ({Math.round(textPlacement.x)},{" "}
            {Math.round(textPlacement.y)})
          </p>
          <Label htmlFor="shape-text-value">Text</Label>
          <Input
            id="shape-text-value"
            value={textValue}
            onChange={(event) => setTextValue(event.target.value)}
            placeholder="Enter label text"
            autoFocus
          />
          {isGm ? (
            <div className="flex items-center gap-2">
              <Checkbox
                id="shape-text-visible"
                checked={textVisibleToPlayers}
                onCheckedChange={(checked) =>
                  setTextVisibleToPlayers(checked === true)
                }
              />
              <Label htmlFor="shape-text-visible">Visible to players</Label>
            </div>
          ) : null}
          <div className="flex gap-2">
            <Button
              type="button"
              variant="primary"
              onClick={submitText}
              disabled={!textValue.trim()}
            >
              Add text
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => setTextPlacement(null)}
            >
              Cancel
            </Button>
          </div>
        </Panel>
      ) : null}

      {selectedShape ? (
        <Panel variant="stone" className="grid gap-3">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Selected shape
          </p>

          <div className="grid gap-1.5">
            <Label id="shape-color-label">Color</Label>
            <div
              className="flex gap-1.5"
              role="group"
              aria-labelledby="shape-color-label"
              data-testid="shape-color-swatches"
            >
              {SHAPE_COLORS.map((swatch, index) => (
                <button
                  key={swatch.name}
                  type="button"
                  aria-label={swatch.name}
                  aria-pressed={selectedColorIndex === index}
                  onClick={() => updateSelectedShapeColor(index)}
                  className={
                    selectedColorIndex === index
                      ? "size-7 rounded-full border-2 border-foreground"
                      : "size-7 rounded-full border border-border"
                  }
                  style={{ backgroundColor: swatch.hex }}
                />
              ))}
            </div>
          </div>

          {isGm ? (
            <div className="flex items-center gap-2">
              <Checkbox
                id="shape-visible-to-players"
                checked={selectedShape.visibleToPlayers}
                onCheckedChange={(checked) =>
                  updateSelectedShape({ visibleToPlayers: checked === true })
                }
              />
              <Label htmlFor="shape-visible-to-players">
                Visible to players
              </Label>
            </div>
          ) : null}

          <Button
            type="button"
            variant="danger"
            icon="trash"
            onClick={deleteSelectedShape}
          >
            Delete shape
          </Button>
        </Panel>
      ) : null}
    </div>
  );
}

export type { DrawTool as ShapeDrawTool, ShapeKind };
