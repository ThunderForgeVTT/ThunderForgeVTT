import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import {
  GM_TOOL_IDS,
  type GmTool,
  type GmToolId,
} from "@/components/world/GmToolRail/GmToolRail";
import type {
  DockSection,
  DockSectionId,
} from "@/components/world/PlayDock/WorldDock";

/** A tool as the phone board needs it: no panel, just what to call it. */
export type RowTool = Pick<GmTool, "id" | "label" | "icon">;
/** A dock section as the phone board needs it. */
export type RowSection = Pick<DockSection, "id" | "label" | "icon">;

/** What tapping an item opens or does. */
export type SheetContent =
  | { kind: "section"; id: DockSectionId }
  | { kind: "tool"; id: GmToolId }
  | { kind: "me" }
  | { kind: "dice" }
  | { kind: "sheet" }
  | { kind: "more" }
  | { kind: "tools" }
  | { kind: "levels" }
  | { kind: "token-tray" }
  | { kind: "fullscreen" };

export interface RowItem {
  id: string;
  label: string;
  icon: FantasyIconName;
  opens: SheetContent;
}

export interface PhoneRows {
  /** The bottom row, in order. */
  row: RowItem[];
  /**
   * The Game Master's Tools sheet (Board, then Table); for a player, the
   * More sheet, whose Board group holds only what the Game Master granted.
   */
  groups: { board: RowItem[]; table: RowItem[] };
}

const fixed = (
  id: string,
  label: string,
  icon: FantasyIconName,
  opens: SheetContent,
): RowItem => ({ id, label, icon, opens });

const ME = fixed("me", "Me", "locate", { kind: "me" });
const DICE = fixed("dice", "Dice", "spark", { kind: "dice" });
const SHEET = fixed("sheet", "Sheet", "spells", { kind: "sheet" });
const MORE = fixed("more", "More", "compass", { kind: "more" });
const TOOLS = fixed("tools", "Tools", "wand", { kind: "tools" });
const LEVELS = fixed("levels", "Levels", "scene", { kind: "levels" });
const TOKEN_TRAY = fixed("token-tray", "Token tray", "inventory", {
  kind: "token-tray",
});
const FULLSCREEN = fixed("fullscreen", "Full screen", "map", {
  kind: "fullscreen",
});

const sectionItem = (s: RowSection): RowItem => ({
  id: s.id,
  label: s.label,
  icon: s.icon,
  opens: { kind: "section", id: s.id },
});

const toolItem = (t: RowTool): RowItem => ({
  id: t.id,
  label: t.label,
  icon: t.icon,
  opens: { kind: "tool", id: t.id },
});

/**
 * Spec 102 R6: which buttons the phone board shows, and where.
 *
 * `tools` and `sections` are already filtered to what this viewer may use
 * (`railTools`, `useDockSections`), so nothing here grants anything: an item
 * whose tool or section is missing from the input is simply not produced
 * (FR-006). A Trusted Player is a player here.
 */
export function phoneRows(
  isGm: boolean,
  tools: RowTool[],
  sections: RowSection[],
): PhoneRows {
  const section = (id: DockSectionId) => sections.find((s) => s.id === id);
  const pick = (order: DockSectionId[]) =>
    order.flatMap((id) => {
      const s = section(id);
      return s ? [sectionItem(s)] : [];
    });
  const orderedTools = GM_TOOL_IDS.flatMap((id) => {
    const t = tools.find((tool) => tool.id === id);
    return t ? [toolItem(t)] : [];
  });

  if (isGm) {
    return {
      row: [TOOLS, DICE, ...pick(["actors", "chat", "combat"])],
      groups: {
        board: [...orderedTools, LEVELS, TOKEN_TRAY],
        table: [
          ...pick(["requests", "settings", "system", "help"]),
          FULLSCREEN,
        ],
      },
    };
  }

  return {
    row: [ME, DICE, SHEET, ...pick(["chat"]), MORE],
    groups: {
      // Select is where a player already is; only the granted extras count.
      board: orderedTools.filter((t) => t.id !== "select"),
      table: [
        ...pick(["actors", "combat", "system", "settings", "help"]),
        FULLSCREEN,
      ],
    },
  };
}
