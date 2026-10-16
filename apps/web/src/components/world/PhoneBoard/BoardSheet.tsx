import { useEffect, useRef, type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { X, ChevronUp, ChevronDown } from "lucide-react";
import {
  FantasyIcon,
  type FantasyIconName,
} from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";

export type SheetSize = "half" | "full";

export interface BoardSheetProps {
  /** Which content the sheet holds; `null` is closed. */
  contentId: string | null;
  title: string;
  icon?: FantasyIconName;
  size: SheetSize;
  onSizeChange: (size: SheetSize) => void;
  onClose: () => void;
  children: ReactNode;
}

/** How far a drag on the handle has to travel before it means anything. */
const DRAG_PX = 48;

/**
 * Spec 102 R4: a panel that rises from the bottom of a phone's board.
 *
 * Non-modal on purpose: half open, the top of the map stays live, so a
 * player can read Chat and still tap their token. Tapping the map therefore
 * does not close it; the close button, the handle and Back do.
 *
 * Back closes it because a phone's Back is the first thing a thumb reaches
 * for, and leaving the world over a panel would lose the table. The sheet
 * pushes one history entry when it opens (not when its content changes) and
 * takes it off again when a button closes it.
 */
export function BoardSheet({
  contentId,
  title,
  icon,
  size,
  onSizeChange,
  onClose,
  children,
}: BoardSheetProps) {
  const open = contentId !== null;
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });
  const returnTo = useRef<HTMLElement | null>(null);
  const dragFrom = useRef<number | null>(null);

  useEffect(() => {
    if (!open) return;
    returnTo.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const marker = {
      boardSheet: crypto.getRandomValues(new Uint32Array(1))[0],
    };
    history.pushState({ ...(history.state ?? {}), ...marker }, "");
    const onPop = () => onCloseRef.current();
    window.addEventListener("popstate", onPop);
    return () => {
      window.removeEventListener("popstate", onPop);
      // Closed by a button rather than Back: our entry is still on top.
      if (history.state?.boardSheet === marker.boardSheet) history.back();
    };
  }, [open]);

  const endDrag = (y: number) => {
    const from = dragFrom.current;
    dragFrom.current = null;
    if (from === null) return;
    const moved = y - from;
    if (moved < -DRAG_PX) onSizeChange("full");
    else if (moved > DRAG_PX) {
      if (size === "full") onSizeChange("half");
      else onClose();
    }
  };

  return (
    <DialogPrimitive.Root
      open={open}
      modal={false}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Content
          data-testid="board-sheet"
          data-size={size}
          data-content={contentId ?? ""}
          aria-describedby={undefined}
          onInteractOutside={(event) => event.preventDefault()}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            returnTo.current?.focus();
          }}
          className={cn(
            // Above the row (portrait) or beside it (landscape), so the row
            // stays in reach and a second tile replaces this sheet.
            "fixed inset-x-2 bottom-[calc(max(12px,env(safe-area-inset-bottom))+60px)] z-[1060] flex flex-col rounded-2xl border border-border bg-background/95 text-foreground shadow-2xl backdrop-blur outline-none",
            "landscape:right-[calc(max(12px,env(safe-area-inset-right))+64px)] landscape:bottom-[max(8px,env(safe-area-inset-bottom))]",
            "duration-[180ms] data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom motion-reduce:animate-none",
            size === "half"
              ? "max-h-[60dvh]"
              : "top-[max(8px,env(safe-area-inset-top))]",
          )}
        >
          <div
            className="flex h-6 shrink-0 cursor-grab touch-none items-center justify-center"
            onPointerDown={(event) => {
              dragFrom.current = event.clientY;
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerUp={(event) => endDrag(event.clientY)}
            onPointerCancel={() => {
              dragFrom.current = null;
            }}
            aria-hidden="true"
          >
            <span className="h-1 w-10 rounded-full bg-muted-foreground/40" />
          </div>
          <header className="flex shrink-0 items-center gap-1 border-b border-border pr-1 pl-4">
            <DialogPrimitive.Title className="flex min-w-0 flex-1 items-center gap-2 truncate text-sm font-semibold tracking-tight">
              {icon ? <FantasyIcon name={icon} size={16} /> : null}
              {title}
            </DialogPrimitive.Title>
            <button
              type="button"
              data-testid="board-sheet-expand"
              aria-label={size === "half" ? "Expand" : "Shrink"}
              onClick={() => onSizeChange(size === "half" ? "full" : "half")}
              className="flex size-11 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              {size === "half" ? (
                <ChevronUp className="size-5" aria-hidden="true" />
              ) : (
                <ChevronDown className="size-5" aria-hidden="true" />
              )}
            </button>
            <DialogPrimitive.Close
              data-testid="board-sheet-close"
              aria-label={`Close ${title}`}
              className="flex size-11 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              <X className="size-5" aria-hidden="true" />
            </DialogPrimitive.Close>
          </header>
          {/* min-h-0 lets a long panel scroll inside the sheet rather than
           * pushing the sheet past the viewport. */}
          <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4">
            {children}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
