import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { RerollButtonsView } from "../RerollButtons";
import { msUntilClosed } from "../rerollWindow";

/**
 * Spec 084 T044: the Reroll buttons on the maker's own roll. Read from
 * server-rendered markup, as `apps/web`'s unit tests have no DOM; the timer
 * that hides them is `msUntilClosed`, tested as a function.
 */
const INSPIRATION = { id: "inspiration", label: "Heroic Inspiration" };
const LUCK = { id: "luck_point", label: "Luck Point" };

function markup(props: Partial<Parameters<typeof RerollButtonsView>[0]>) {
  return renderToStaticMarkup(
    <RerollButtonsView
      offers={[INSPIRATION, LUCK]}
      open
      busy={null}
      error={null}
      onReroll={() => {}}
      {...props}
    />,
  );
}

describe("RerollButtons", () => {
  it("shows one button per offer, named by its label", () => {
    const html = markup({});
    expect(html).toContain('data-testid="roll-reroll-inspiration"');
    expect(html).toContain('data-testid="roll-reroll-luck_point"');
    expect(html).toContain("Reroll (Heroic Inspiration)");
    expect(html).toContain("Reroll (Luck Point)");
  });

  it("shows nothing once the window has closed or nothing is offered", () => {
    expect(markup({ open: false })).toBe("");
    expect(markup({ offers: [] })).toBe("");
  });

  it("disables every button while one is in flight", () => {
    const html = markup({ busy: "inspiration" });
    expect(html.match(/disabled=""/g)).toHaveLength(2);
    expect(html).toContain("Rerolling…");
  });

  it("shows the server's refusal as it was said", () => {
    const html = markup({ error: "This roll has already been rerolled." });
    expect(html).toContain('data-testid="roll-reroll-error"');
    expect(html).toContain("This roll has already been rerolled.");
  });

  it("counts down to the window's end, and to nothing without one", () => {
    const now = Date.parse("2026-10-08T12:00:00Z");
    expect(msUntilClosed("2026-10-08T12:01:30Z", now)).toBe(90_000);
    expect(msUntilClosed("2026-10-08T11:59:00Z", now)).toBe(0);
    expect(msUntilClosed(null, now)).toBe(0);
  });
});
