import { useCallback, useEffect, useState, type RefObject } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { createShape } from "@/api/shapes";
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
}

type DrawTool = "none" | "freehand" | "rect" | "ellipse" | "line" | "text";

const DRAW_TOOLS: { value: DrawTool; label: string }[] = [
  { value: "freehand", label: "Freehand" },
  { value: "rect", label: "Rectangle" },
  { value: "ellipse", label: "Ellipse" },
  { value: "line", label: "Line/Arrow" },
  { value: "text", label: "Text" },
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
 * GM-only: the caller (WorldPage) is responsible for only rendering this
 * component for the scene owner (FR-009 — players never see authoring
 * tools), exactly like WallTool's gating.
 *
 * Drawing itself (freehand strokes, rect/ellipse/line click-drag) is
 * implemented engine-side (Bevy): selecting a sub-tool here only signals
 * intent via local UI state today, ready to be observed by the engine
 * bridge once that lands — same placeholder pattern as WallTool's draw
 * mode toggle. The text tool is the one exception: since the engine
 * doesn't handle in-canvas text input, this component itself listens for
 * a click on the canvas container while "text" is active, and dispatches
 * the create_shape (kind TEXT) mutation directly rather than waiting on
 * an engine-emitted event.
 */
export function ShapeTool({
  worldStore,
  shapes,
  selectedShapeId,
  sceneId,
  canvasContainerRef,
}: ShapeToolProps) {
  const [activeTool, setActiveTool] = useState<DrawTool>("none");
  const [textPlacement, setTextPlacement] = useState<TextPlacement | null>(
    null,
  );
  const [textValue, setTextValue] = useState("");
  const [textVisibleToPlayers, setTextVisibleToPlayers] = useState(false);
  const [textSubmitting, setTextSubmitting] = useState(false);

  const selectedShape = selectedShapeId ? shapes[selectedShapeId] : null;

  const toggleTool = useCallback((tool: DrawTool) => {
    setActiveTool((current) => (current === tool ? "none" : tool));
    setTextPlacement(null);
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
      const rect = container.getBoundingClientRect();
      setTextPlacement({
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      });
      setTextValue("");
    };

    container.addEventListener("click", onClick);
    return () => {
      container.removeEventListener("click", onClick);
    };
  }, [activeTool, canvasContainerRef, sceneId]);

  const submitText = useCallback(() => {
    if (!textPlacement || !sceneId || !textValue.trim()) {
      return;
    }

    setTextSubmitting(true);
    void createShape({
      sceneId,
      kind: "TEXT",
      geometry: { x: textPlacement.x, y: textPlacement.y },
      text: textValue.trim(),
      visibleToPlayers: textVisibleToPlayers,
    })
      .then((created) => {
        worldStore.dispatch(
          {
            type: "upsert_shape",
            shape: {
              id: created.shapeId,
              sceneId: created.sceneId,
              kind: "text",
              geometry: created.geometry,
              text: created.text,
              style: created.style,
              visibleToPlayers: created.visibleToPlayers,
            },
          },
          "sync",
        );
      })
      .catch((error) => {
        console.error("Failed to create text shape:", error);
      })
      .finally(() => {
        setTextSubmitting(false);
        setTextPlacement(null);
        setTextValue("");
      });
  }, [sceneId, textPlacement, textValue, textVisibleToPlayers, worldStore]);

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
    (color: string) => {
      if (!selectedShape) {
        return;
      }

      updateSelectedShape({
        style: { ...(selectedShape.style ?? {}), color },
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

  const selectedColor =
    (selectedShape?.style?.color as string | undefined) ?? "#ffffff";

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
          <div className="flex gap-2">
            <Button
              type="button"
              variant="primary"
              onClick={submitText}
              disabled={textSubmitting || !textValue.trim()}
            >
              {textSubmitting ? "Adding..." : "Add text"}
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
            <Label htmlFor="shape-color">Color</Label>
            <Input
              id="shape-color"
              type="color"
              value={selectedColor}
              onChange={(event) => updateSelectedShapeColor(event.target.value)}
            />
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="shape-visible-to-players"
              checked={selectedShape.visibleToPlayers}
              onCheckedChange={(checked) =>
                updateSelectedShape({ visibleToPlayers: checked === true })
              }
            />
            <Label htmlFor="shape-visible-to-players">Visible to players</Label>
          </div>

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
