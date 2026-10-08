import { describe, expect, it, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SelectionBar, type SelectionBarProps } from "../SelectionBar";
import type { WorldWall } from "@/engine/world/types";

/**
 * Spec 085 T025 — the Select bar.
 *
 * `apps/web` has neither jsdom nor testing-library, so what appears is read
 * from server-rendered markup, and what a press does is read by calling the
 * handler the bar hands its control. The bar holds no state of its own, so
 * both observe exactly what a viewer would get.
 */

function wall(id: string, secret: boolean): WorldWall {
  return {
    id,
    x1: 0,
    y1: 0,
    x2: 1,
    y2: 0,
    blocksVision: true,
    blocksMovement: true,
    doorState: "none",
    secret,
  } as WorldWall;
}

function props(overrides: Partial<SelectionBarProps> = {}): SelectionBarProps {
  return {
    worldStore: { dispatch: vi.fn() },
    isGm: true,
    walls: { w1: wall("w1", false), w2: wall("w2", false) },
    selectedTokenIds: ["t1", "t2", "t3"],
    selectedWallIds: ["w1"],
    selectedLightIds: [],
    selectedShapeIds: [],
    ...overrides,
  };
}

/** The element carrying `data-testid`, found in the tree the bar returns. */
function byTestId(node: ReactNode, testId: string): ReactElement | undefined {
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = byTestId(child, testId);
      if (found) return found;
    }
    return undefined;
  }
  if (!isValidElement(node)) return undefined;
  const elementProps = node.props as Record<string, unknown>;
  if (elementProps["data-testid"] === testId) return node;
  return byTestId(elementProps.children as ReactNode, testId);
}

describe("SelectionBar (spec 085)", () => {
  it("renders nothing for one item or none", () => {
    expect(
      SelectionBar(props({ selectedTokenIds: ["t1"], selectedWallIds: [] })),
    ).toBeNull();
    expect(
      SelectionBar(props({ selectedTokenIds: [], selectedWallIds: [] })),
    ).toBeNull();
  });

  it("shows how many of each kind are selected, and only kinds it holds", () => {
    const html = renderToStaticMarkup(
      SelectionBar(props({ selectedShapeIds: ["s1"] }))!,
    );
    expect(html).toContain("3 tokens");
    expect(html).toContain("1 wall");
    expect(html).toContain("1 shape");
    expect(html).not.toContain("light");
  });

  it("offers Hidden from the table to a GM with walls, and hides the group's walls", () => {
    const p = props({ selectedWallIds: ["w1", "w2"] });
    const toggle = byTestId(SelectionBar(p), "selection-bar-hidden");
    expect(toggle).toBeDefined();
    (
      toggle!.props as { onCheckedChange: (c: boolean) => void }
    ).onCheckedChange(true);
    expect(p.worldStore.dispatch).toHaveBeenCalledWith(
      { type: "set_walls_hidden", wallIds: ["w1", "w2"], hidden: true },
      "ui",
    );
  });

  it("shows the walls as hidden only when every one of them is", () => {
    const some = props({
      walls: { w1: wall("w1", true), w2: wall("w2", false) },
      selectedWallIds: ["w1", "w2"],
    });
    const all = props({
      walls: { w1: wall("w1", true), w2: wall("w2", true) },
      selectedWallIds: ["w1", "w2"],
    });
    const checked = (p: SelectionBarProps) =>
      (
        byTestId(SelectionBar(p), "selection-bar-hidden")!.props as {
          checked: boolean;
        }
      ).checked;
    expect(checked(some)).toBe(false);
    expect(checked(all)).toBe(true);
  });

  it("does not offer it to a player, or to a group without walls", () => {
    expect(
      byTestId(SelectionBar(props({ isGm: false })), "selection-bar-hidden"),
    ).toBeUndefined();
    expect(
      byTestId(
        SelectionBar(props({ selectedWallIds: [] })),
        "selection-bar-hidden",
      ),
    ).toBeUndefined();
  });

  it("Delete dispatches delete_group, for a GM and a player alike", () => {
    for (const isGm of [true, false]) {
      const p = props({ isGm });
      const button = byTestId(SelectionBar(p), "selection-bar-delete");
      (button!.props as { onClick: () => void }).onClick();
      expect(p.worldStore.dispatch).toHaveBeenCalledWith(
        { type: "delete_group" },
        "ui",
      );
    }
  });
});
