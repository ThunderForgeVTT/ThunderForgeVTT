import { describe, expect, it, vi } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  StatusPanel,
  type PanelCorner,
  type PanelResource,
  type StatusPanelProps,
} from "../StatusPanel";
import type { Disclosed } from "@/engine/sdk/Disclosed";

/**
 * Spec 029, User Story 5 — where the panel sits, and what it says when there
 * is nothing to say.
 *
 * # Why this renders to markup rather than into a DOM
 *
 * `apps/web` has neither jsdom nor testing-library, and adding either means a
 * lockfile change shared with every other workstream for the sake of one
 * file. `StatusPanel` is a pure function of its props — no state, no effects
 * — so server-rendering it observes exactly what a viewer would see, and the
 * one interactive path is reached by finding the control by its accessible
 * name and invoking the handler it exposes, which is what a click does.
 *
 * The assertions are about what a person perceives: whether a panel appears
 * at all, what a screen reader is told, and which corner it is placed in. The
 * corner is the one thing named by class, because a class is the actual
 * mechanism by which CSS puts the panel top-left rather than bottom-right.
 *
 * The *durability* of that choice across a reload is not testable here — it
 * lives in `localStorage` and a real page load — and is proven in
 * `apps/web/e2e/status-placement.spec.ts` instead.
 */

const CORNERS: PanelCorner[] = [
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
];

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
    corner: "bottom-right",
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

/** The corner picker, as the viewer reaches it: by its label. */
function cornerPicker(
  corner: PanelCorner,
  onCornerChange: (corner: PanelCorner) => void,
): React.ReactElement {
  const tree = StatusPanel({
    resources: [resource(EXACT)],
    corner,
    onCornerChange,
  });
  const select = findByAriaLabel(tree, "Panel position");
  if (!select) {
    throw new Error("the corner picker should be reachable by its name");
  }
  return select;
}

describe("StatusPanel placement (FR-011)", () => {
  it.each(CORNERS)("puts the panel in the %s corner", (corner) => {
    const markup = render({ corner });
    expect(markup).toContain(`status-panel--${corner}`);
    // And in no other, so a leftover class cannot fight the chosen one.
    for (const other of CORNERS.filter((c) => c !== corner)) {
      expect(markup).not.toContain(`status-panel--${other}`);
    }
  });

  it("offers all four corners under a named control", () => {
    const markup = render({ onCornerChange: () => {} });
    expect(markup).toContain('aria-label="Panel position"');
    for (const label of [
      "Top left",
      "Top right",
      "Bottom left",
      "Bottom right",
    ]) {
      expect(markup).toContain(label);
    }
  });

  it("shows the corner the viewer is on as the control's current value", () => {
    expect(
      (cornerPicker("top-right", () => {}).props as { value: string }).value,
    ).toBe("top-right");
  });

  it("reports a new choice, so something above it can remember it", () => {
    const onCornerChange = vi.fn();
    const select = cornerPicker("bottom-right", onCornerChange);
    const onChange = (
      select.props as {
        onChange: (event: { target: { value: string } }) => void;
      }
    ).onChange;

    onChange({ target: { value: "top-left" } });

    expect(onCornerChange).toHaveBeenCalledWith("top-left");
  });

  it("omits the control entirely when the corner cannot be changed", () => {
    expect(render()).not.toContain("Panel position");
  });
});

describe("StatusPanel with nothing selected (FR-012)", () => {
  it("shows no panel at all when no token is selected", () => {
    expect(render({ resources: null })).toBe("");
  });

  it("shows no panel for a token whose system declares no resources", () => {
    // Not an empty frame: a blank panel in the corner reads as "this token
    // has nothing left", which is a different claim from "nothing to show".
    expect(render({ resources: [] })).toBe("");
  });

  it("cannot keep the previous token's figures after deselection", () => {
    expect(render({ resources: [resource(EXACT)] })).toContain("7 / 12");
    expect(render({ resources: null })).not.toContain("7 / 12");
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
