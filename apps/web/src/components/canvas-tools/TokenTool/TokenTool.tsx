import { useCallback } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import type { WorldStore } from "@/engine/world/store";
import type { WorldToken } from "@/engine/world/types";

export interface TokenToolProps {
  worldStore: WorldStore;
  tokens: Record<string, WorldToken>;
  selectedTokenId: string | null;
}

const MIN_TOKEN_SCALE = 1;
const MAX_TOKEN_SCALE = 5;
const ROTATE_STEP_DEGREES = 30;

/**
 * TokenTool: GM-only property panel for the currently selected token's
 * size and facing, per specs/004-token-canvas-authoring T020.
 *
 * Resize/rotate were shipped in an earlier session as keyboard-only
 * shortcuts (`]`/`[` resize, `,`/`.` rotate — see
 * `src/engine/src/systems/selection.rs`'s `handle_token_resize_rotate_keyboard`)
 * with no UI surface at all, which meant a GM had no way to discover or
 * verify the capability existed. This panel closes that gap: it displays
 * the selected token's current scale/rotation and offers equivalent
 * buttons, without introducing a second implementation of the
 * resize/rotate logic itself.
 *
 * The buttons work by dispatching the exact same `upsert_token` world
 * command the engine's own keyboard handler emits (see
 * `ExternalCommand::UpsertToken` in `src/engine/src/lib.rs`) — so the
 * click path and the keypress path converge on one mechanism. Per
 * Constitution Principle I, this component never renders or simulates
 * the token itself; it only issues the same confirmed-command round trip
 * WallTool.tsx's property panel already uses for walls, and the engine
 * (via `bindWorldStore`'s `apply_world_command` forwarding) is what
 * actually updates the on-canvas Transform.
 *
 * GM-only: the caller (WorldPage) is responsible for only mounting this
 * for the scene owner, mirroring WallTool.tsx's own documented contract —
 * this component does not re-check GM status itself.
 */
export function TokenTool({ worldStore, tokens, selectedTokenId }: TokenToolProps) {
  const selectedToken = selectedTokenId ? tokens[selectedTokenId] : null;

  const applyTokenChange = useCallback(
    (changes: Partial<Pick<WorldToken, "scale" | "rotation">>) => {
      if (!selectedToken) {
        return;
      }

      worldStore.dispatch(
        {
          type: "upsert_token",
          token: {
            ...selectedToken,
            ...changes,
          },
        },
        "ui",
      );
    },
    [selectedToken, worldStore],
  );

  if (!selectedToken) {
    return null;
  }

  const currentScale = selectedToken.scale ?? 1;
  const currentRotationDegrees = Math.round(
    (((selectedToken.rotation ?? 0) * 180) / Math.PI) % 360,
  );

  const resize = (delta: number) => {
    const nextScale = Math.min(
      MAX_TOKEN_SCALE,
      Math.max(MIN_TOKEN_SCALE, currentScale + delta),
    );
    applyTokenChange({ scale: nextScale });
  };

  const rotate = (deltaDegrees: number) => {
    const nextRotation = (selectedToken.rotation ?? 0) + (deltaDegrees * Math.PI) / 180;
    applyTokenChange({ rotation: nextRotation });
  };

  return (
    <Panel variant="stone" className="grid gap-3" data-testid="token-tool">
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Selected token
      </p>

      <div className="grid gap-1.5">
        <span className="text-sm" data-testid="token-tool-scale">
          Size: {currentScale}x
        </span>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-shrink"
            onClick={() => resize(-1)}
            disabled={currentScale <= MIN_TOKEN_SCALE}
          >
            Shrink
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-grow"
            onClick={() => resize(1)}
            disabled={currentScale >= MAX_TOKEN_SCALE}
          >
            Grow
          </Button>
        </div>
        <span className="text-xs text-muted-foreground">
          Keyboard: {"]"} grow, {"["} shrink (whole grid cells only)
        </span>
      </div>

      <div className="grid gap-1.5">
        <span className="text-sm" data-testid="token-tool-rotation">
          Facing: {currentRotationDegrees}°
        </span>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-rotate-left"
            onClick={() => rotate(ROTATE_STEP_DEGREES)}
          >
            Rotate left
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-rotate-right"
            onClick={() => rotate(-ROTATE_STEP_DEGREES)}
          >
            Rotate right
          </Button>
        </div>
        <span className="text-xs text-muted-foreground">
          Keyboard: , rotate left, . rotate right
        </span>
      </div>
    </Panel>
  );
}
