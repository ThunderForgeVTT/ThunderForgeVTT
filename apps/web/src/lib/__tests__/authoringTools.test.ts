import { describe, expect, it } from "vitest";
import { permittedTools, reconcileOpenTool } from "@/lib/authoringTools";
import type { GmToolId } from "@/components/world/GmToolRail/GmToolRail";

const rail: { id: GmToolId }[] = [
  { id: "select" },
  { id: "walls" },
  { id: "lights" },
  { id: "shapes" },
  { id: "tokens" },
  { id: "interactions" },
];

describe("permittedTools", () => {
  it("offers a tool only when the viewer holds it", () => {
    expect(permittedTools(rail, ["select", "walls"]).map((t) => t.id)).toEqual([
      "select",
      "walls",
    ]);
  });

  it("offers nothing when the viewer holds nothing", () => {
    // FR-045 seen from the rail: a player in a world whose Game Master has
    // granted nothing gets an empty answer, and an empty answer must not be
    // read as "no restriction" — the mistake that would hand out every tool.
    expect(permittedTools(rail, [])).toEqual([]);
  });

  it("leaves the rail alone until the answer arrives", () => {
    // Not a permission decision: the server and the engine both refuse
    // independently, so showing the pre-permission rail for the moment before
    // the query resolves costs nothing and avoids a Game Master's rail
    // flickering on every load.
    expect(permittedTools(rail, null)).toHaveLength(rail.length);
  });

  it("ignores a granted tool this build does not have", () => {
    expect(
      permittedTools(rail, ["walls", "wombat" as GmToolId]).map((t) => t.id),
    ).toEqual(["walls"]);
  });
});

describe("reconcileOpenTool", () => {
  it("keeps a tool the viewer still holds", () => {
    expect(reconcileOpenTool("walls", ["select", "walls"])).toBe("walls");
  });

  it("moves off a tool that was just taken away", () => {
    expect(reconcileOpenTool("walls", ["select"])).toBe("select");
  });

  it("closes the rail entirely when nothing is left", () => {
    expect(reconcileOpenTool("walls", [])).toBeNull();
  });

  it("waits rather than guessing while unresolved", () => {
    expect(reconcileOpenTool("walls", null)).toBe("walls");
  });
});
