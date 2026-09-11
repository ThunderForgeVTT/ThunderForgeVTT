import { describe, expect, it, vi } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  StatusPanel,
  type PanelResource,
  type StatusPanelProps,
} from "../StatusPanel";
import { KEEP_ON_SCREEN, keepOnScreen } from "../keepOnScreen";
import type { Disclosed } from "@/engine/sdk/Disclosed";

/**
 * Spec 029 FR-011a/FR-012a (playtest 2026-09-10 P6) — a pinned panel sits
 * where it is put, can be moved and closed, and says nothing when there is
 * nothing pinned.
 *
 * # Why this renders to markup rather than into a DOM
 *
 * `apps/web` has neither jsdom nor testing-library, and adding either means a
 * lockfile change shared with every other workstream for the sake of one
 * file. `StatusPanel` is a pure function of its props — no state, no effects,
 * no hooks — so server-rendering it observes exactly what a viewer would see,
 * and an interactive path is reached by finding the control by its accessible
 * name and invoking the handler it exposes, which is what a click does.
 *
 * A drag with a real pointer, and the position surviving a reload, are not
 * testable here — they are proven in `apps/web/e2e/status-placement.spec.ts`.
 */

function resource(disclosed: Disclosed, label = "Health"): PanelResource {
  return {
    definition: {
      id: "health",
      label,
      kind: "bar",
      order: 0,
      allowStacking: false,
    },
    disclosed,
  };
}

const EXACT: Disclosed = {
  disclosure: "visible",
  entries: [{ current: 7, max: 12, label: null }],
};

function render(props: Partial<StatusPanelProps> = {}): string {
  const full: StatusPanelProps = {
    resources: [resource(EXACT)],
    position: { x: 120, y: 80 },
    ...props,
  };
  return renderToStaticMarkup(React.createElement(StatusPanel, full));
}

/** Find the node with a given accessible name in a rendered element tree. */
function findByAriaLabel(
  node: React.ReactNode,
  label: string,
): React.ReactElement | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByAriaLabel(child, label);
      if (found) return found;
    }
    return null;
  }
  if (!React.isValidElement(node)) return null;
  const props = node.props as Record<string, unknown>;
  if (props["aria-label"] === label) return node;
  return findByAriaLabel(props.children as React.ReactNode, label);
}

function tree(props: Partial<StatusPanelProps>): React.ReactNode {
  return StatusPanel({
    resources: [resource(EXACT)],
    position: { x: 120, y: 80 },
    ...props,
  });
}

describe("A pinned StatusPanel (FR-011a)", () => {
  it("sits where it was put", () => {
    const markup = render({ position: { x: 210, y: 64 } });
    expect(markup).toContain("left:210px");
    expect(markup).toContain("top:64px");
    // And nowhere else: the corner classes of the old model are gone.
    expect(markup).not.toMatch(/status-panel--(top|bottom)-(left|right)/);
  });

  it("offers a close control by name, and reports it", () => {
    const onClose = vi.fn();
    const close = findByAriaLabel(tree({ onClose }), "Unpin status panel");
    expect(
      close,
      "the close control should be reachable by its name",
    ).not.toBeNull();
    (close!.props as { onClick: () => void }).onClick();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("offers no close control when it cannot be closed", () => {
    expect(render()).not.toContain("Unpin status panel");
  });

  it("moves with the arrow keys, not only with a drag", () => {
    const onMove = vi.fn();
    const handle = findByAriaLabel(
      tree({ onMove, position: { x: 100, y: 100 } }),
      "Move status panel",
    );
    expect(handle, "the handle should be reachable by its name").not.toBeNull();
    const onKeyDown = (
      handle!.props as {
        onKeyDown: (event: {
          key: string;
          shiftKey: boolean;
          preventDefault: () => void;
        }) => void;
      }
    ).onKeyDown;

    onKeyDown({ key: "ArrowRight", shiftKey: false, preventDefault: () => {} });
    expect(onMove).toHaveBeenLastCalledWith({ x: 116, y: 100 });

    onKeyDown({ key: "ArrowUp", shiftKey: true, preventDefault: () => {} });
    expect(onMove).toHaveBeenLastCalledWith({ x: 100, y: 36 });

    onKeyDown({ key: "a", shiftKey: false, preventDefault: () => {} });
    expect(onMove).toHaveBeenCalledTimes(2);
  });

  it("is not a handle at all when it cannot move", () => {
    expect(render()).not.toContain("Move status panel");
  });
});

describe("keepOnScreen", () => {
  const viewport = { width: 1280, height: 720 };

  it("leaves a position already on screen alone", () => {
    expect(keepOnScreen({ x: 300, y: 200 }, viewport)).toEqual({
      x: 300,
      y: 200,
    });
  });

  it("never lets the header leave the top or the left", () => {
    expect(keepOnScreen({ x: -500, y: -40 }, viewport)).toEqual({ x: 0, y: 0 });
  });

  it("keeps enough of it inside the right and bottom edges to grab", () => {
    expect(keepOnScreen({ x: 5000, y: 5000 }, viewport)).toEqual({
      x: viewport.width - KEEP_ON_SCREEN,
      y: viewport.height - KEEP_ON_SCREEN,
    });
  });

  it("changes nothing when there is no viewport to ask", () => {
    expect(keepOnScreen({ x: -9, y: 9999 }, null)).toEqual({ x: -9, y: 9999 });
  });
});

describe("StatusPanel with nothing pinned (FR-012a)", () => {
  it("shows no panel at all", () => {
    expect(render({ resources: null })).toBe("");
  });

  it("shows no panel for a token whose system declares no resources", () => {
    // Not an empty frame: a blank panel reads as "this token has nothing
    // left", which is a different claim from "nothing to show".
    expect(render({ resources: [] })).toBe("");
  });

  it("keeps no heading behind either, even one it was given", () => {
    expect(render({ resources: null, title: "Ogre" })).not.toContain("Ogre");
  });
});

describe("StatusPanel says how sure a figure is", () => {
  it("states an exact reading plainly", () => {
    const markup = render({ resources: [resource(EXACT)] });
    expect(markup).toContain("Health: 7 / 12");
    expect(markup).not.toContain("approximately");
  });

  const coarse: [string, Disclosed, string][] = [
    ["chunked", { disclosure: "chunked", quarter: 2 }, "2 of 4"],
    ["percentage", { disclosure: "percentage", proportion: 0.42 }, "42%"],
    ["greyed", { disclosure: "greyed" }, "Not disclosed"],
  ];

  it.each(coarse)(
    "announces a %s figure as an estimate rather than a reading",
    (_kind, disclosed, shown) => {
      const markup = render({ resources: [resource(disclosed)] });
      expect(markup).toContain(`Health: approximately ${shown}`);
    },
  );
});
