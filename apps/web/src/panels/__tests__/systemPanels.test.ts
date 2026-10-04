import { describe, expect, it } from "vitest";
import {
  panelKey,
  resolvePanel,
  resolvePanelTitle,
  SYSTEM_PANELS,
} from "@/panels/systemPanels";

/**
 * `032/T108`: four pages used to compare a world's game system id against one
 * system's, and mount that system's panel if it matched. A pack declares its
 * panels now, and this app finds them.
 *
 * The registry is not written by hand, so the useful assertions are not that
 * a lookup agrees with the map — they are the same object, it always will be
 * — but that discovery *happened at all*, and that the one property the slot
 * vocabulary had to allow actually holds.
 *
 * These name Genie because a test has to name something to assert it, and
 * Genie is the pack that ships panels today. `check-system-registry` exempts
 * tests for exactly this reason.
 */
describe("systemPanels", () => {
  /**
   * A glob whose pattern stops matching yields an empty registry silently,
   * and every system quietly loses every panel — no error, no warning, just
   * pages that render nothing where a panel used to be. That is the failure
   * this catches.
   */
  it("discovers the panels bundled packs ship, without a hand-written list", () => {
    expect(
      Object.keys(SYSTEM_PANELS).length,
      "the glob found no pack panels at all — check its pattern",
    ).toBeGreaterThan(0);

    for (const slot of [
      "npc-detail",
      "world-staging",
      "world-settings",
      "dock",
    ] as const) {
      expect(
        resolvePanel("genie", slot),
        `genie:${slot} resolved to nothing`,
      ).toBeTypeOf("function");
    }
  });

  /**
   * The case the vocabulary exists to permit.
   *
   * Genie's session loop is one panel reached from two places — the staging
   * page before play, the play dock during it — so `panels/dock.tsx`
   * re-exports `panels/world-staging.tsx`'s default rather than copying it.
   * Asserted as reference identity, not as "both are functions": a refactor
   * that forked them into two near-identical components would pass the weaker
   * check, and noticing that is the point.
   */
  it("lets one component fill two slots", () => {
    const staging = resolvePanel("genie", "world-staging");
    const dock = resolvePanel("genie", "dock");

    expect(staging).not.toBeNull();
    expect(dock).toBe(staging);
  });

  /**
   * A system that ships no panel for a slot, a world with no system at all,
   * and an id that matches nothing are one answer on screen: there is nothing
   * to mount there. So they are one answer here, and the caller renders
   * whatever it renders without a panel — no dock tab, or nothing at all.
   */
  it("answers null for anything it has no panel for", () => {
    expect(resolvePanel("genie", "npc-detail")).not.toBeNull();
    expect(resolvePanel("a-system-with-no-panels", "dock")).toBeNull();
    expect(resolvePanel(null, "dock")).toBeNull();
    expect(resolvePanel(undefined, "dock")).toBeNull();
    expect(resolvePanel("", "dock")).toBeNull();
  });

  it("keys a panel by system and slot together", () => {
    expect(panelKey("genie", "dock")).toBe("genie:dock");
    expect(SYSTEM_PANELS[panelKey("genie", "dock")]).toBe(
      resolvePanel("genie", "dock"),
    );
  });

  /**
   * Spec 067 FR-020: the dock tab reads what the pack calls its panel. The
   * title rides on the slot file, not the component, so Genie's shared
   * session loop is titled in the dock and untitled on the staging page.
   */
  it("reads the title a panel module exports", () => {
    expect(resolvePanelTitle("genie", "dock")).toBe("Clocks");
    expect(resolvePanelTitle("roll_for_shoes", "dock")).toBe("Table");
    expect(resolvePanelTitle("genie", "world-staging")).toBeNull();
    expect(resolvePanelTitle("a-system-with-no-panels", "dock")).toBeNull();
    expect(resolvePanelTitle(null, "dock")).toBeNull();
  });
});
