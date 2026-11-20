import { useMemo, useState, type ReactNode } from "react";
import type {
  GmTool,
  GmToolId,
} from "@/components/world/GmToolRail/GmToolRail";
import type {
  DockSection,
  DockSectionId,
} from "@/components/world/PlayDock/WorldDock";
import { BoardSheet, type SheetSize } from "./BoardSheet";
import { BottomRow } from "./BottomRow";
import { MoreSheet } from "./MoreSheet";
import { phoneRows, type RowItem, type SheetContent } from "./phoneRows";

export interface PhoneBoardProps {
  isGm: boolean;
  /** The tools this viewer may use (`railTools`), each with its panel. */
  tools: GmTool[];
  openToolId: GmToolId | null;
  onOpenToolChange: (id: GmToolId | null) => void;
  sections: DockSection[];
  openSectionId: DockSectionId | null;
  onOpenSectionIdChange: (id: DockSectionId | null) => void;
  /** Notices and chips that sit over the top of the map. */
  topStack: ReactNode;
  /** The dice roller, shown in the Dice sheet. */
  dice: ReactNode;
  /** The viewer's character, shown in the Sheet sheet. */
  characterSheet: (close: () => void) => ReactNode;
  /**
   * Me: find the viewer's token. `null` when it has been selected and put on
   * screen; otherwise what to show instead (a chooser, or why there is none).
   */
  onMe: (close: () => void) => ReactNode | null;
  onOpenTokenTray: () => void;
  /** The scene's levels, for the Game Master's Levels item. */
  levels?: ReactNode;
  /** Player requests waiting, badged on Tools (FR-011). */
  requestCount?: number;
}

type Local =
  | Exclude<SheetContent, { kind: "section" }>
  | { kind: "me-result"; node: ReactNode };

/**
 * Spec 102: the play board's chrome on a phone. The map is the screen; a row
 * of five tiles and one sheet at a time are all that sit on it.
 *
 * Same page state as the desktop rail and dock (R2): an open dock section is
 * still the page's `openSectionId`, so Escape opening Settings works here too,
 * and the armed tool is still the page's. What is local is only what a phone
 * adds: which non-section sheet is open, and how tall.
 */
export default function PhoneBoard({
  isGm,
  tools,
  openToolId,
  onOpenToolChange,
  sections,
  openSectionId,
  onOpenSectionIdChange,
  topStack,
  dice,
  characterSheet,
  onMe,
  onOpenTokenTray,
  levels,
  requestCount = 0,
}: PhoneBoardProps) {
  const [local, setLocal] = useState<Local | null>(null);
  const [size, setSize] = useState<SheetSize>("half");
  const { row, groups } = useMemo(
    () => phoneRows(isGm, tools, sections),
    [isGm, tools, sections],
  );

  const section = openSectionId
    ? (sections.find((s) => s.id === openSectionId) ?? null)
    : null;

  const openLocal = (next: Local) => {
    onOpenSectionIdChange(null);
    setLocal(next);
    setSize("half");
  };
  const close = () => {
    onOpenSectionIdChange(null);
    setLocal(null);
  };

  const pick = (item: RowItem) => {
    const { opens } = item;
    // A second tap on an open tile closes it.
    if (item.id === openId) return close();
    switch (opens.kind) {
      case "section":
        setLocal(null);
        setSize("half");
        onOpenSectionIdChange(opens.id);
        return;
      case "tool":
        onOpenToolChange(opens.id);
        // Select is a mode with nothing to set; the others open their options.
        if (opens.id === "select") return close();
        return openLocal(opens);
      case "me": {
        const instead = onMe(close);
        return instead
          ? openLocal({ kind: "me-result", node: instead })
          : close();
      }
      case "token-tray":
        close();
        return onOpenTokenTray();
      case "fullscreen":
        close();
        if (document.fullscreenElement) void document.exitFullscreen();
        else void document.documentElement.requestFullscreen?.();
        return;
      default:
        return openLocal(opens);
    }
  };

  const openId = section
    ? section.id
    : local?.kind === "me-result"
      ? "me"
      : local?.kind === "tool"
        ? null
        : (local?.kind ?? null);

  const sheet = (() => {
    if (section)
      return {
        id: section.id,
        title: section.label,
        icon: section.icon,
        body: section.content,
      };
    if (!local) return null;
    switch (local.kind) {
      case "dice":
        return {
          id: "dice",
          title: "Dice",
          icon: "spark" as const,
          body: dice,
        };
      case "sheet":
        return {
          id: "sheet",
          title: "Sheet",
          icon: "spells" as const,
          body: characterSheet(close),
        };
      case "me-result":
        return {
          id: "me",
          title: "Me",
          icon: "locate" as const,
          body: local.node,
        };
      case "levels":
        return {
          id: "levels",
          title: "Levels",
          icon: "scene" as const,
          body: levels,
        };
      case "tool": {
        const tool = tools.find((t) => t.id === local.id);
        if (!tool?.content) return null;
        return {
          id: `tool-${tool.id}`,
          title: `${tool.label} options`,
          icon: tool.icon,
          body: tool.content,
        };
      }
      case "tools":
      case "more":
        return {
          id: local.kind,
          title: local.kind === "tools" ? "Tools" : "More",
          icon:
            local.kind === "tools" ? ("wand" as const) : ("compass" as const),
          body: (
            <MoreSheet
              groups={groups}
              armedId={openToolId}
              onPick={pick}
              badges={{ requests: requestCount }}
            />
          ),
        };
      default:
        return null;
    }
  })();

  return (
    <div
      role="region"
      aria-label="Board controls"
      data-testid="phone-board"
      data-role={isGm ? "gm" : "player"}
    >
      <div
        data-testid="phone-top-stack"
        className="pointer-events-none fixed inset-x-2 top-[max(8px,env(safe-area-inset-top))] z-[1070] flex flex-col items-center gap-2 *:pointer-events-auto"
      >
        {topStack}
      </div>
      <BottomRow
        items={row}
        openId={openId}
        onPick={pick}
        badges={{ tools: requestCount }}
      />
      <BoardSheet
        contentId={sheet?.id ?? null}
        title={sheet?.title ?? ""}
        icon={sheet?.icon}
        size={size}
        onSizeChange={setSize}
        onClose={close}
        fill={sheet?.id === "chat"}
      >
        {sheet?.body}
      </BoardSheet>
    </div>
  );
}
