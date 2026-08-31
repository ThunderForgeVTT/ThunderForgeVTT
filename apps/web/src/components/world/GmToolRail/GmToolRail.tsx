import { type ReactNode } from "react";
import {
  FantasyIcon,
  type FantasyIconName,
} from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";

export type GmToolId = "walls" | "lights" | "shapes" | "tokens";

export interface GmTool {
  id: GmToolId;
  label: string;
  icon: FantasyIconName;
  /** The tool's own panel, rendered in a flyout when this tool is open. */
  content: ReactNode;
}

export interface GmToolRailProps {
  tools: GmTool[];
  /** Which tool is open, or `null` for none. */
  openToolId: GmToolId | null;
  onOpenToolChange: (toolId: GmToolId | null) => void;
}

/**
 * The GM's left-hand tool rail: a narrow column of tool icons, each opening
 * a flyout with that tool's controls (Shapes' flyout is the five shape
 * sub-tools, so picking a shape is one level in rather than a permanent
 * five-button stack).
 *
 * This replaces the previous fixed stack of always-expanded panels pinned
 * over the top-left and top-right of the map, which covered the canvas at
 * all times whether or not the GM was drawing anything.
 *
 * Only the open tool's content is mounted. ShapeTool in particular attaches
 * a click listener to the canvas container while its text sub-tool is
 * active; keeping every tool mounted would leave those listeners live for
 * tools the GM isn't using.
 *
 * # Which tool is open is the caller's state, not this component's
 *
 * The rail is only rendered once the scene and the viewer's role have both
 * resolved, so it mounts a moment after the play view does — and it was
 * remounting again as those settled. Holding the open tool in local state
 * meant every one of those remounts silently closed whatever the Game Master
 * had just opened: they clicked Walls, the panel appeared, and it vanished
 * again for no reason they could see.
 *
 * Lifting it to the page makes a remount survivable, which is the correct
 * shape anyway — "which tool am I working with" outlives this component.
 */
export function GmToolRail({
  tools,
  openToolId,
  onOpenToolChange,
}: GmToolRailProps) {
  const openTool = tools.find((tool) => tool.id === openToolId) ?? null;

  return (
    <div
      className="pointer-events-none absolute inset-y-0 left-0 z-[1040] flex items-start"
      data-testid="gm-tool-rail"
      data-open-tool={openToolId ?? ""}
    >
      <nav
        className="pointer-events-auto flex w-12 flex-col items-center gap-1 self-stretch border-r border-border bg-background/95 py-3 shadow-xl backdrop-blur"
        aria-label="Game master tools"
      >
        {tools.map((tool) => {
          const isOpen = tool.id === openToolId;
          return (
            <button
              key={tool.id}
              type="button"
              title={tool.label}
              aria-label={tool.label}
              aria-expanded={isOpen}
              data-testid={`gm-tool-${tool.id}`}
              onClick={() =>
                onOpenToolChange(openToolId === tool.id ? null : tool.id)
              }
              className={cn(
                "flex h-9 w-9 items-center justify-center rounded-lg transition-colors",
                isOpen
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <FantasyIcon name={tool.icon} size={18} />
            </button>
          );
        })}
      </nav>

      {openTool ? (
        <section
          // Capped and scrollable rather than sized to content: a tool with
          // a selection panel open (ShapeTool with a shape selected) is
          // considerably taller than the same tool with nothing selected,
          // and must not run off the bottom of the viewport.
          className="pointer-events-auto m-2 max-h-[calc(100vh-1rem)] w-64 overflow-y-auto rounded-xl border border-border bg-background/95 p-3 shadow-xl backdrop-blur"
          aria-label={openTool.label}
          data-testid={`gm-tool-panel-${openTool.id}`}
        >
          <header className="mb-2 flex items-center justify-between gap-2">
            <h2 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              {openTool.label}
            </h2>
            <button
              type="button"
              onClick={() => onOpenToolChange(null)}
              aria-label={`Close ${openTool.label}`}
              className="rounded-md px-1.5 text-lg leading-none text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              ‹
            </button>
          </header>
          {openTool.content}
        </section>
      ) : null}
    </div>
  );
}
