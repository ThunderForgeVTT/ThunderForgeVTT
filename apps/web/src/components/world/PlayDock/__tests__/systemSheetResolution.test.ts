import { describe, expect, it } from "vitest";
import {
  resolveActorSheet,
  SYSTEM_ACTOR_SHEETS,
} from "@/pages/world/actor/systemActorSheets";

/**
 * The spec's edge case: "a game system that defines no character sheet, when a
 * player chooses View" (spec 031). The in-pane view must still open — FR-002
 * puts the player's character inside the play screen — so what the absence has
 * to be is an answer this component can render around, not an exception and
 * not a redirect back out to a tab.
 *
 * Lives beside the in-pane view rather than beside the registry because it is
 * the caller's contract that matters here: `null` means "draw what you can
 * without a sheet".
 */
describe("resolveActorSheet", () => {
  it("gives a registered system its own sheet", () => {
    expect(resolveActorSheet("genie")).toBe(SYSTEM_ACTOR_SHEETS.genie);
  });

  /**
   * A system that ships no sheet, an actor belonging to no system at all, and
   * an id that matches nothing are one answer on screen: there is nothing
   * systemic to draw. Nothing is substituted — a generic sheet would be this
   * app inventing stats for rules it does not own.
   */
  it("answers null for anything it has no sheet for", () => {
    expect(resolveActorSheet("a-system-with-no-sheet")).toBeNull();
    expect(resolveActorSheet(null)).toBeNull();
    expect(resolveActorSheet(undefined)).toBeNull();
    expect(resolveActorSheet("")).toBeNull();
  });
});
