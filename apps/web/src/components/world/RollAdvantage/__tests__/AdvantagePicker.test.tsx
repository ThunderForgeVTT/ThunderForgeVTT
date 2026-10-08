import { describe, expect, it, vi } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { AdvantagePicker } from "../AdvantagePicker";
import { ADVANTAGE_CHOICES } from "../advantage";
import type { Advantage } from "../advantage";

/**
 * Spec 084 FR-018: the picker beside a character's rolls. `apps/web` has no
 * DOM in its unit tests (see `effectHelperRow.test.tsx`), so what a viewer
 * sees is read from the server-rendered markup, and a click is the handler
 * the button carries.
 */
function markup(value: Advantage) {
  return renderToStaticMarkup(
    <AdvantagePicker value={value} onChange={() => {}} />,
  );
}

/** The buttons the picker draws, as React elements. */
function buttons(onChange: (choice: Advantage) => void) {
  const group = AdvantagePicker({
    value: "NORMAL",
    onChange,
  }) as React.ReactElement<{
    children: React.ReactElement<{ onClick: () => void }>[];
  }>;
  return group.props.children;
}

describe("AdvantagePicker", () => {
  it("offers normal, advantage and disadvantage, under a name", () => {
    const html = markup("NORMAL");
    expect(html).toContain('data-testid="roll-advantage-picker"');
    expect(html).toContain('role="radiogroup"');
    expect(html).toContain('aria-label="Roll with advantage"');
    expect(ADVANTAGE_CHOICES).toEqual(["NORMAL", "ADVANTAGE", "DISADVANTAGE"]);
    for (const label of ["Normal", "Advantage", "Disadvantage"]) {
      expect(html).toContain(`>${label}<`);
    }
  });

  it("marks the current choice, and only that one", () => {
    const html = markup("DISADVANTAGE");
    expect(html.match(/aria-checked="true"/g)).toHaveLength(1);
    expect(html).toMatch(
      /data-testid="roll-advantage-DISADVANTAGE"[^>]*aria-checked="true"|aria-checked="true"[^>]*data-testid="roll-advantage-DISADVANTAGE"/,
    );
  });

  it("reports the choice a click picks", () => {
    const onChange = vi.fn();
    const [, advantage, disadvantage] = buttons(onChange);
    advantage.props.onClick();
    disadvantage.props.onClick();
    expect(onChange.mock.calls).toEqual([["ADVANTAGE"], ["DISADVANTAGE"]]);
  });
});
