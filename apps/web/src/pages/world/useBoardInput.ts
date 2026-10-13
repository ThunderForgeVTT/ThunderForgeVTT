import {
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import { createDoubleTap } from "@/pages/world/doubleTap";
import { isInOverlay, isTextEntry } from "@/engine/canvasKeyboard";
import { installWorldProbe } from "@/engine/world/probe";
import {
  createWorldFacets,
  type ControllableToken,
} from "@/engine/world/facets";
import type { WorldStore } from "@/engine/world/store";
import type { DockSectionId } from "@/components/world/PlayDock/WorldDock";
import type { GmToolId } from "@/components/world/GmToolRail/GmToolRail";
import type { PanelPosition } from "@/components/StatusPanel/StatusPanel";
import type { AuthUser } from "@/types/auth";

interface UseBoardInputParams {
  id: string;
  sceneId: string | null;
  user: AuthUser | null;
  isSceneOwner: boolean;
  worldStore: WorldStore;
  playView: "staging" | "playing";
  hasToolRail: boolean;
  effectiveGmToolId: GmToolId | null;
  setOpenGmToolId: Dispatch<SetStateAction<GmToolId | null>>;
  setPinned: Dispatch<
    SetStateAction<{ tokenId: string; at: PanelPosition } | null>
  >;
  savedPinRef: RefObject<PanelPosition | null>;
}

/**
 * Window-level gestures over the play view: Escape, and the double-click or
 * double-tap that pins a token's panel or asks which of a stack is meant.
 *
 * Moved out of `WorldPage` unchanged, with the world facets those gestures
 * read and the development-only world probe that sat between them.
 */
export function useBoardInput({
  id,
  sceneId,
  user,
  isSceneOwner,
  worldStore,
  playView,
  hasToolRail,
  effectiveGmToolId,
  setOpenGmToolId,
  setPinned,
  savedPinRef,
}: UseBoardInputParams) {
  // Escape, anywhere on the play view, puts the tool down (owner request
  // 2026-10-06): with an authoring tool open it returns to Select; on Select
  // already — or for a player, who has no rail — it opens the Settings
  // section, and closes it again if it is what is open. A selected token is
  // the exception: the engine's Escape deselects it, and that is the whole
  // press. Menus, dialogs and text fields keep their own Escape; a key one of
  // them has already claimed, or that lands while one is open, is theirs.
  // The canvas is not one of them: the engine's winit calls `preventDefault`
  // on every key it hears, to stop the browser's own handling, so a press
  // with the canvas focused — any time after a click on the board — arrives
  // here already "prevented" without anyone having claimed it.
  const [openDockSectionId, setOpenDockSectionId] =
    useState<DockSectionId | null>(null);
  useEffect(() => {
    if (playView !== "playing") return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (
        event.defaultPrevented &&
        !(event.target instanceof HTMLCanvasElement)
      ) {
        return;
      }
      if (isTextEntry(event.target) || isInOverlay(event.target)) return;
      if (
        document.querySelector(
          '[role="menu"], [role="dialog"], [role="alertdialog"]',
        )
      ) {
        return;
      }
      if (
        hasToolRail &&
        effectiveGmToolId !== null &&
        effectiveGmToolId !== "select"
      ) {
        setOpenGmToolId("select");
        return;
      }
      if (worldStore.getState().selectedTokenIds.length > 0) return;
      setOpenDockSectionId((open) => (open === "settings" ? null : "settings"));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // `setOpenGmToolId` is a state setter from `useGmToolRail`: stable.
  }, [playView, hasToolRail, effectiveGmToolId, worldStore, setOpenGmToolId]);

  // Development-only introspection for the world store; see
  // `engine/world/probe.ts`. Compiled out of production builds.
  useEffect(() => installWorldProbe(worldStore), [worldStore]);

  // Stacked-token gestures. A single click takes the whole stack (handled
  // in the engine); a double-click asks which one, detected below from the
  // DOM's `dblclick`, and the picker's answer goes back to the engine as a
  // `select_token` through the world store.
  const [stackPicker, setStackPicker] = useState<{
    members: ControllableToken[];
    at: { x: number; y: number };
  } | null>(null);

  const facets = useMemo(
    () =>
      createWorldFacets(worldStore, {
        worldId: id,
        sceneId,
        principal: {
          userId: user?.id ?? null,
          authority: isSceneOwner ? "gm" : user ? "player" : "observer",
        },
      }),
    [worldStore, id, sceneId, isSceneOwner, user],
  );

  // Double-click over the canvas asks *which* of the stacked tokens. The
  // click that precedes it has already selected the stack, so this only has
  // to offer a choice between what is selected. Detected here rather than in
  // the engine: two clicks that fast routinely land in one Bevy frame, where
  // the second press is lost outright, while the DOM's `dblclick` is exact.
  useEffect(() => {
    const onDoubleClick = (event: {
      target: EventTarget | null;
      clientX: number;
      clientY: number;
    }) => {
      if (!(event.target instanceof HTMLCanvasElement)) return;
      const stack = facets.selection.disambiguate();
      if (!stack) {
        // Spec 029 FR-011a (playtest 2026-09-10 P6): a double-click on one
        // token pins its status panel — where the viewer last left one, or
        // here if they never have. The first click of the pair has already
        // selected the token through the engine's own hit test; should that
        // not have reached the store yet, it is asked once more a moment
        // later rather than guessed at.
        const pin = () => {
          const tokenId = worldStore.getState().selectedTokenId;
          if (!tokenId) return false;
          setPinned({
            tokenId,
            at: savedPinRef.current ?? { x: event.clientX, y: event.clientY },
          });
          return true;
        };
        if (!pin()) window.setTimeout(pin, 50);
        return;
      }
      setStackPicker({
        members: stack.members,
        at: { x: event.clientX, y: event.clientY },
      });
    };

    // A finger's two taps are not a `dblclick` over the canvas — see
    // `doubleTap.ts` — so they are counted, and answered the same way. Should
    // a browser send the `dblclick` as well, it asks for what is already so.
    const taps = createDoubleTap();
    const onPointerDown = (event: PointerEvent) => {
      if (event.pointerType !== "touch" || !event.isPrimary) return;
      taps.down({ x: event.clientX, y: event.clientY, at: event.timeStamp });
    };
    const onPointerUp = (event: PointerEvent) => {
      if (event.pointerType !== "touch" || !event.isPrimary) return;
      if (
        taps.up({ x: event.clientX, y: event.clientY, at: event.timeStamp })
      ) {
        onDoubleClick(event);
      }
    };

    window.addEventListener("dblclick", onDoubleClick, { capture: true });
    window.addEventListener("pointerdown", onPointerDown, { capture: true });
    window.addEventListener("pointerup", onPointerUp, { capture: true });
    return () => {
      window.removeEventListener("dblclick", onDoubleClick, { capture: true });
      window.removeEventListener("pointerdown", onPointerDown, {
        capture: true,
      });
      window.removeEventListener("pointerup", onPointerUp, { capture: true });
      facets.stop();
    };
    // `setPinned` and `savedPinRef` are the page's state setter and ref:
    // stable, so this still re-binds only when the facets or store change.
  }, [facets, worldStore, setPinned, savedPinRef]);

  return {
    openDockSectionId,
    setOpenDockSectionId,
    stackPicker,
    setStackPicker,
    facets,
  };
}
