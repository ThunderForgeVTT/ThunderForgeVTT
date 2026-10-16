import { describe, expect, it } from "vitest";
import { phoneRows, type RowTool, type RowSection } from "../phoneRows";

const tool = (id: RowTool["id"]): RowTool => ({ id, label: id, icon: "rune" });
const section = (id: RowSection["id"]): RowSection => ({
  id,
  label: id,
  icon: "rune",
});

const ALL_TOOLS = (
  ["select", "walls", "lights", "shapes", "tokens", "interactions"] as const
).map(tool);
const GM_SECTIONS = (
  [
    "chat",
    "actors",
    "combat",
    "system",
    "requests",
    "settings",
    "help",
  ] as const
).map(section);
const PLAYER_SECTIONS = (
  ["chat", "actors", "combat", "system", "settings", "help"] as const
).map(section);

const ids = (items: { id: string }[]) => items.map((i) => i.id);

describe("phoneRows", () => {
  it("gives a player Me, Dice, Sheet, Chat, More", () => {
    const { row } = phoneRows(false, [], PLAYER_SECTIONS);
    expect(ids(row)).toEqual(["me", "dice", "sheet", "chat", "more"]);
  });

  it("puts the player's other sections and full screen in More", () => {
    const { groups } = phoneRows(false, [], PLAYER_SECTIONS);
    expect(groups.board).toEqual([]);
    expect(ids(groups.table)).toEqual([
      "actors",
      "combat",
      "system",
      "settings",
      "help",
      "fullscreen",
    ]);
  });

  it("adds the tools a Game Master granted to a player's More", () => {
    const { groups } = phoneRows(
      false,
      [tool("select"), tool("walls")],
      PLAYER_SECTIONS,
    );
    expect(ids(groups.board)).toEqual(["walls"]);
  });

  it("gives a Game Master Tools, Dice, Actors, Chat, Combat", () => {
    const { row } = phoneRows(true, ALL_TOOLS, GM_SECTIONS);
    expect(ids(row)).toEqual(["tools", "dice", "actors", "chat", "combat"]);
  });

  it("groups the Game Master's board and table work", () => {
    const { groups } = phoneRows(true, [...ALL_TOOLS].reverse(), GM_SECTIONS);
    expect(ids(groups.board)).toEqual([
      "select",
      "walls",
      "lights",
      "shapes",
      "tokens",
      "interactions",
      "levels",
      "token-tray",
    ]);
    expect(ids(groups.table)).toEqual([
      "requests",
      "settings",
      "system",
      "help",
      "fullscreen",
    ]);
  });

  it("never offers a section or tool it was not given (FR-006)", () => {
    const { row, groups } = phoneRows(
      true,
      [tool("select")],
      [section("chat"), section("settings")],
    );
    const every = ids([...row, ...groups.board, ...groups.table]);
    for (const absent of [
      "walls",
      "lights",
      "actors",
      "combat",
      "system",
      "requests",
      "help",
    ]) {
      expect(every).not.toContain(absent);
    }
  });

  it("names each item after the section or tool it opens", () => {
    const { row } = phoneRows(
      false,
      [],
      [{ id: "chat", label: "Table talk", icon: "quill" }],
    );
    expect(row.find((i) => i.id === "chat")).toMatchObject({
      label: "Table talk",
      icon: "quill",
      opens: { kind: "section", id: "chat" },
    });
  });
});
