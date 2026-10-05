import { useCallback, useEffect, useState } from "react";
import {
  boardCentre,
  boardPointToClient,
  onCanvasContextMenu,
  type CanvasContextMenuEvent,
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
  /**
   * The wall or door it is about, when it is not about a token: a token
   * standing in a doorway is what somebody right-clicking there means.
   */
  wallId: string | null;
  /** The placed light it is about, when it is about one (spec 073). */
  lightId: string | null;
  /** The drawing it is about, when it is about one (spec 073). */
  shapeId: string | null;
  /** A second right-click on the same door, straight after the first. */
  doubled: boolean;
  /** Where focus goes back to when the menu, and anything it opened, closes. */
  returnFocus: HTMLElement | null;
  /** Bumped per request, so asking twice at one spot is two requests. */
  serial: number;
}

let serial = 0;

/** How soon, and how near, a second right-click is the same gesture. */
const DOUBLE_CLICK_MS = 500;
const DOUBLE_CLICK_PX = 12;

/** One right-click on a wall, as the next one needs to remember it. */
export interface WallClick {
  wallId: string;
  at: { x: number; y: number };
  /** Milliseconds, from any one clock. */
  time: number;
}

/** Whether `next` is the second half of a double right-click begun by `previous`. */
export function isDoubleRightClick(
  previous: WallClick | null,
  next: WallClick,
): boolean {
  return (
    previous !== null &&
    previous.wallId === next.wallId &&
    next.time - previous.time <= DOUBLE_CLICK_MS &&
    Math.hypot(next.at.x - previous.at.x, next.at.y - previous.at.y) <=
      DOUBLE_CLICK_PX
  );
}

/** The one thing a right-click is about, of everything that was under it. */
export interface MenuSubject {
  tokenId: string | null;
  lightId: string | null;
  wallId: string | null;
  shapeId: string | null;
}

/**
 * Which of the things under a right-click it is about: at most one.
 *
 * The smaller and the more deliberate thing to aim at wins. A token first,
 * as it always has. Then a light, whose marker is a dot somebody has to mean
 * to hit. Then a wall or a door, a line. A drawing last: a rectangle can
 * cover a whole room, and everything standing in the room is still there to
 * be clicked.
 */
export function menuSubjectOf(
  event: Pick<
    CanvasContextMenuEvent,
    "tokenIds" | "wallId" | "lightId" | "shapeId"
  >,
): MenuSubject {
  const tokenId = event.tokenIds[0] ?? null;
  const lightId = tokenId ? null : (event.lightId ?? null);
  const wallId = tokenId || lightId ? null : (event.wallId ?? null);
  const shapeId = tokenId || lightId || wallId ? null : (event.shapeId ?? null);
  return { tokenId, lightId, wallId, shapeId };
}

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
    let lastWallClick: WallClick | null = null;
    return onCanvasContextMenu((event) => {
      const canvas = document.querySelector<HTMLCanvasElement>("canvas");
      const box = canvas?.getBoundingClientRect();
      const at = {
        x: (box?.left ?? 0) + event.screenX,
        y: (box?.top ?? 0) + event.screenY,
      };
      const { tokenId, lightId, wallId, shapeId } = menuSubjectOf(event);
      const click = wallId ? { wallId, at, time: performance.now() } : null;
      const doubled =
        click !== null && isDoubleRightClick(lastWallClick, click);
      // A third click starts over rather than doubling the second.
      lastWallClick = doubled ? null : click;
      serial += 1;
      setRequest({
        at,
        world: { x: event.worldX, y: event.worldY },
        tokenId,
        wallId,
        lightId,
        shapeId,
        doubled,
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
          wallId: null,
          lightId: null,
          shapeId: null,
          doubled: false,
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
