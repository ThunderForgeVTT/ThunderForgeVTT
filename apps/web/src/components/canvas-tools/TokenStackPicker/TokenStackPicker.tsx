import { useEffect, useRef } from "react";

import type { ControllableToken } from "@/engine/world/facets";

export interface TokenStackPickerProps {
  /** Members of the double-clicked stack, topmost first. */
  members: ControllableToken[];
  /** Where to anchor, in client (viewport) pixels. */
  at: { x: number; y: number };
  onPick: (tokenId: string) => void;
  onDismiss: () => void;
}

/**
 * The picker a double-click on a stacked square opens.
 *
 * A single click takes the whole stack, which is right for shifting a pile
 * out of a doorway and useless when the intent is "that one, underneath".
 * This is the second gesture: choose a member by name and art rather than
 * by dragging the stack apart to reach it.
 *
 * Dismissing must be free. Opening this changes no selection (the engine
 * emits `disambiguate_tokens` without touching state), so Escape or a click
 * outside leaves the board exactly as it was — which is what makes
 * double-clicking safe to try when you are not sure what is under there.
 *
 * Members a viewer cannot act on are still listed, greyed. Hiding them
 * would mean a stack of three showing two entries and no explanation, and
 * "there is something here you do not control" is information a player
 * needs to make sense of the square.
 */
export function TokenStackPicker({ members, at, onPick, onDismiss }: TokenStackPickerProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // Stopped here so the engine's own Escape handling (which cancels a
        // movement plan) does not also fire while a picker is open.
        event.stopPropagation();
        onDismiss();
      }
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) {
        onDismiss();
      }
    };

    // Capture, because the canvas sits under this and would otherwise
    // process the dismissing click as a fresh selection.
    window.addEventListener("keydown", onKeyDown, { capture: true });
    window.addEventListener("pointerdown", onPointerDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", onKeyDown, { capture: true });
      window.removeEventListener("pointerdown", onPointerDown, { capture: true });
    };
  }, [onDismiss]);

  if (members.length === 0) {
    return null;
  }

  return (
    <div
      ref={containerRef}
      role="listbox"
      aria-label="Tokens in this space"
      data-testid="token-stack-picker"
      className="bg-popover fixed z-[1100] grid max-h-72 w-56 gap-0.5 overflow-y-auto rounded-md border p-1 shadow-lg"
      style={{
        // Clamped so a stack near the right or bottom edge does not open
        // the picker off-screen.
        left: Math.min(at.x, window.innerWidth - 240),
        top: Math.min(at.y, window.innerHeight - 300),
      }}
    >
      <p className="text-muted-foreground px-2 py-1 text-[0.65rem] tracking-widest uppercase">
        {members.length} tokens here
      </p>
      {members.map(({ token, canMove }) => (
        <button
          key={token.id}
          type="button"
          role="option"
          aria-selected={false}
          disabled={!canMove}
          data-testid={`token-stack-option-${token.id}`}
          onClick={() => onPick(token.id)}
          className="hover:bg-accent flex items-center gap-2 rounded px-2 py-1.5 text-left text-sm disabled:opacity-50"
          title={canMove ? undefined : "You do not control this token"}
        >
          <span className="bg-muted grid size-7 shrink-0 place-items-center overflow-hidden rounded">
            {token.photoUrl ? (
              <img src={token.photoUrl} alt="" className="size-full object-contain" />
            ) : (
              <span className="text-muted-foreground text-[0.6rem]">—</span>
            )}
          </span>
          <span className="truncate">{token.label ?? token.id}</span>
        </button>
      ))}
    </div>
  );
}
