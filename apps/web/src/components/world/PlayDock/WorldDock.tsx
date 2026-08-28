import { useState, type ReactNode } from "react";
import {
  FantasyIcon,
  type FantasyIconName,
} from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";

export type DockSectionId =
  | "chat"
  | "actors"
  | "combat"
  | "clocks"
  | "settings";

export interface DockSection {
  id: DockSectionId;
  label: string;
  icon: FantasyIconName;
  content: ReactNode;
}

export interface WorldDockProps {
  sections: DockSection[];
  /** Section to open on first render. Omit to start collapsed. */
  defaultSectionId?: DockSectionId;
}

/**
 * The Play view's right-hand dock: a permanent icon rail that expands into
 * one section at a time (Chat, Actors, Combat, Clocks & Timers, Settings).
 *
 * The rail is always visible and always the same width, so the canvas'
 * usable area only changes when a section is actually open — the icons
 * never reflow the map out from under a GM mid-drag.
 *
 * Only the open section's content is mounted. That is deliberate: each
 * panel owns a live subscription and a refetch loop, and mounting all five
 * would keep five of them running against a world the GM is only looking
 * at through one. The trade-off is that switching sections refetches, which
 * for these payload sizes is cheaper than the alternative.
 */
export function WorldDock({ sections, defaultSectionId }: WorldDockProps) {
  const [openSectionId, setOpenSectionId] = useState<DockSectionId | null>(
    defaultSectionId ?? null,
  );

  const openSection =
    sections.find((section) => section.id === openSectionId) ?? null;

  const toggle = (id: DockSectionId) => {
    setOpenSectionId((current) => (current === id ? null : id));
  };

  return (
    <div
      className="pointer-events-none absolute inset-y-0 right-0 z-[1050] flex items-stretch"
      data-testid="world-dock"
      data-open-section={openSectionId ?? ""}
    >
      {openSection ? (
        <section
          className="pointer-events-auto flex w-[22rem] max-w-[85vw] flex-col border-l border-border bg-background/95 shadow-xl backdrop-blur"
          aria-label={openSection.label}
          data-testid={`world-dock-panel-${openSection.id}`}
        >
          <header className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
            <h2 className="flex items-center gap-2 text-sm font-semibold tracking-tight">
              <FantasyIcon name={openSection.icon} size={16} />
              {openSection.label}
            </h2>
            <button
              type="button"
              onClick={() => setOpenSectionId(null)}
              aria-label={`Collapse ${openSection.label}`}
              data-testid="world-dock-collapse"
              className="rounded-md px-2 py-1 text-lg leading-none text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              ›
            </button>
          </header>
          {/* min-h-0 is what lets an overflowing child actually scroll
           * inside this flex column instead of stretching the dock past
           * the viewport. */}
          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {openSection.content}
          </div>
        </section>
      ) : null}

      <nav
        className="pointer-events-auto flex w-12 flex-col items-center gap-1 border-l border-border bg-background/95 py-3 shadow-xl backdrop-blur"
        aria-label="Play panels"
      >
        {sections.map((section) => {
          const isOpen = section.id === openSectionId;
          return (
            <button
              key={section.id}
              type="button"
              onClick={() => toggle(section.id)}
              title={section.label}
              aria-label={section.label}
              aria-expanded={isOpen}
              data-testid={`world-dock-tab-${section.id}`}
              className={cn(
                "flex h-9 w-9 items-center justify-center rounded-lg transition-colors",
                isOpen
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <FantasyIcon name={section.icon} size={18} />
            </button>
          );
        })}
      </nav>
    </div>
  );
}
