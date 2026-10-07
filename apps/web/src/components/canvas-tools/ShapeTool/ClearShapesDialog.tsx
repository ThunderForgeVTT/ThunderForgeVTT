import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Dialog } from "@/components/ui/dialog/Dialog";
import type { WorldStore } from "@/engine/world/store";

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
