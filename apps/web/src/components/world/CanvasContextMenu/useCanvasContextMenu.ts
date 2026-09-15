import { useCallback, useEffect, useState } from "react";
import {
  boardCentre,
  boardPointToClient,
  onCanvasContextMenu,
} from "@/engine/bevy";
import type { WorldStore } from "@/engine/world/store";

/** A menu asked for, by a right-click or from the keyboard. */
export interface CanvasMenuRequest {
  /** Where the menu opens, in client pixels. */
  at: { x: number; y: number };
  /** The same point on the board. */
  world: { x: number; y: number };
  /** The token it is about, or `null` for bare board. */
  tokenId: string | null;
  /** Where focus goes back to when the menu, and anything it opened, closes. */
  returnFocus: HTMLElement | null;
  /** Bumped per request, so asking twice at one spot is two requests. */
  serial: number;
}

let serial = 0;

const TEXT_ENTRY = new Set(["INPUT", "TEXTAREA", "SELECT"]);

/**
 * True for the key that opens a context menu from the keyboard: the
 * ContextMenu key, or Shift+F10 — the platform convention for both.
 */
export function isContextMenuKey(event: KeyboardEvent): boolean {
  return event.key === "ContextMenu" || (event.shiftKey && event.key === "F10");
}

/**
 * The play field's right-click, and its keyboard equivalent, as one request.
 *
 * A right-click arrives from the engine (`canvas_context_menu`), which knows
 * what is under the pointer. The keyboard path is chrome's own: with the
 * ContextMenu key or Shift+F10, the menu opens on the token selected on the
 * board — or, for a Game Master with nothing selected, on the board where the
 * camera is looking — so nothing the pointer can do is out of a keyboard
 * user's reach.
 *
 * Not while typing, and not from inside another menu or dialog: there the key
 * belongs to what has focus.
 */
export function useCanvasContextMenu(
  worldStore: WorldStore,
  enabled: boolean,
  isGameMaster: boolean,
): { request: CanvasMenuRequest | null; done: () => void } {
  const [request, setRequest] = useState<CanvasMenuRequest | null>(null);
  const done = useCallback(() => setRequest(null), []);

  useEffect(() => {
    if (!enabled) return;
    return onCanvasContextMenu((event) => {
      const canvas = document.querySelector<HTMLCanvasElement>("canvas");
      const box = canvas?.getBoundingClientRect();
      serial += 1;
      setRequest({
        at: {
          x: (box?.left ?? 0) + event.screenX,
          y: (box?.top ?? 0) + event.screenY,
        },
        world: { x: event.worldX, y: event.worldY },
        tokenId: event.tokenIds[0] ?? null,
        returnFocus: canvas ?? null,
        serial,
      });
    });
  }, [enabled]);

  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!isContextMenuKey(event)) return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          TEXT_ENTRY.has(target.tagName) ||
          target.closest(
            '[role="menu"], [role="dialog"], [role="alertdialog"]',
          ))
      ) {
        return;
      }
      const state = worldStore.getState();
      const selected = state.selectedTokenId
        ? state.tokens[state.selectedTokenId]
        : undefined;
      if (!selected && !isGameMaster) return;
      // The browser's own menu would open over ours.
      event.preventDefault();
      const returnFocus =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      void (async () => {
        const world = selected
          ? { x: selected.x, y: selected.y }
          : await boardCentre();
        if (!world) return;
        const at = await boardPointToClient(world.x, world.y);
        if (!at) return;
        serial += 1;
        setRequest({
          at,
          world,
          tokenId: selected?.id ?? null,
          returnFocus,
          serial,
        });
      })();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled, isGameMaster, worldStore]);

  return { request, done };
}
