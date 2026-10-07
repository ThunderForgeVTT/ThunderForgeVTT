import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog } from "@/components/ui/dialog/Dialog";
import { Label } from "@/components/ui/label";
import type { WorldStore } from "@/engine/world/store";
import { useShapeCreators } from "@/hooks/useShapeCreators";

export interface ClearShapesDialogProps {
  worldStore: WorldStore;
  sceneId: string;
  /** Named in the confirmation, so the Game Master knows which board empties. */
  sceneName: string;
}

/**
 * The Game Master's "Clear all shapes" (spec 082 US3): every drawing on the
 * scene, on every level, whoever drew it, after a confirmation that names
 * the scene.
 *
 * It only asks. The drawings leave every board, this one included, when the
 * server's `deleted` events arrive (`engine/world/sync/shapes.ts`).
 */
export function ClearShapesDialog({
  worldStore,
  sceneId,
  sceneName,
}: ClearShapesDialogProps) {
  const [open, setOpen] = useState(false);

  const clearAll = () => {
    worldStore.dispatch({ type: "clear_shapes", sceneId }, "ui");
    setOpen(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      trigger={
        <Button
          type="button"
          variant="secondary"
          icon="trash"
          data-testid="shape-clear-all"
        >
          Clear all shapes
        </Button>
      }
      title={`Clear every drawing on ${sceneName}?`}
      description="Every drawing on every level of this scene is deleted for everyone, the players' drawings too. This cannot be undone."
      footer={
        <>
          <Button
            type="button"
            variant="ghost"
            onClick={() => setOpen(false)}
            data-testid="shape-clear-cancel"
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="danger"
            onClick={clearAll}
            data-testid="shape-clear-confirm"
          >
            Clear all shapes
          </Button>
        </>
      }
    >
      {null}
    </Dialog>
  );
}

/**
 * The Game Master's "Clear a player's shapes…" (spec 082 US4): a checkbox
 * per player who drew on the scene, with how many shapes each, and only
 * the ticked players' drawings go. Whoever runs the world is not offered.
 */
export function ClearPlayerShapesDialog({
  worldStore,
  sceneId,
  sceneName,
}: ClearShapesDialogProps) {
  const [open, setOpen] = useState(false);
  const [chosen, setChosen] = useState<string[]>([]);
  const { creators, loading, error } = useShapeCreators(sceneId, open);

  const openChanged = (next: boolean) => {
    setOpen(next);
    if (!next) setChosen([]);
  };
  const toggle = (userId: string, on: boolean) =>
    setChosen((now) =>
      on ? [...now, userId] : now.filter((id) => id !== userId),
    );
  const clearChosen = () => {
    worldStore.dispatch(
      { type: "clear_shapes", sceneId, createdBy: chosen },
      "ui",
    );
    openChanged(false);
  };
  const reason = error
    ? "Could not load who drew here."
    : loading
      ? "Looking for the players who drew here…"
      : creators.length === 0
        ? "No player has drawn on this scene."
        : null;

  return (
    <Dialog
      open={open}
      onOpenChange={openChanged}
      trigger={
        <Button
          type="button"
          variant="secondary"
          icon="trash"
          data-testid="shape-clear-player"
        >
          Clear a player's shapes…
        </Button>
      }
      title={`Clear a player's drawings on ${sceneName}`}
      description="The ticked players' drawings on every level of this scene are deleted for everyone. Your drawings and everyone else's stay."
      footer={
        <>
          <Button
            type="button"
            variant="ghost"
            onClick={() => openChanged(false)}
            data-testid="shape-clear-player-cancel"
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="danger"
            onClick={clearChosen}
            disabled={chosen.length === 0}
            data-testid="shape-clear-player-confirm"
          >
            Clear their shapes
          </Button>
        </>
      }
    >
      {reason ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="shape-clear-player-empty"
        >
          {reason}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {creators.map((creator) => {
            const id = `shape-clear-player-${creator.userId}`;
            return (
              <li key={creator.userId} className="flex items-center gap-2">
                <Checkbox
                  id={id}
                  data-testid="shape-clear-player-option"
                  data-user-id={creator.userId}
                  checked={chosen.includes(creator.userId)}
                  onCheckedChange={(v) => toggle(creator.userId, v === true)}
                />
                <Label htmlFor={id} className="text-sm font-normal">
                  {creator.displayName}
                  {creator.isMember ? "" : " (no longer in this world)"} —{" "}
                  {creator.shapeCount === 1
                    ? "1 shape"
                    : `${creator.shapeCount} shapes`}
                </Label>
              </li>
            );
          })}
        </ul>
      )}
    </Dialog>
  );
}
