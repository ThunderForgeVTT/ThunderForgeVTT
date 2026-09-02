import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { BASE_PACK_TITLE, interfacePackLabel } from "../interface-pack-label";
import type { InterfacePackSummary } from "@/api/interfacePacks";

const pack = (id: string, title: string): InterfacePackSummary => ({
  id,
  title,
  version: "1.0.0",
  description: "",
  targets: [],
});

const INSTALLED = [
  pack("forge", "Forge"),
  pack("forged-steel", "Forged Steel"),
];

describe("interfacePackLabel", () => {
  it("names the base pack when a world has stored no binding", () => {
    // FR-023: not "Unbound placeholder", not "Not yet assigned". The base pack
    // is in force and it is what the participant is looking at.
    expect(interfacePackLabel(null, INSTALLED)).toBe("Forge");
  });

  it("names the bound pack by its title, not its id", () => {
    expect(interfacePackLabel("forged-steel", INSTALLED)).toBe("Forged Steel");
  });

  it("gives the same answer for unset on every surface", () => {
    // SC-008 is "zero distinct strings for the unset state". Both surfaces
    // call this function, so the property is the function being a function.
    // Asserted anyway because SC-008 is what the requirement is measured by,
    // and a future overload taking a surface would break it silently.
    const hub = interfacePackLabel(null, INSTALLED);
    const dashboard = interfacePackLabel(null, INSTALLED);
    expect(hub).toBe(dashboard);
  });

  it("falls back to the base pack's title before the listing arrives", () => {
    expect(interfacePackLabel(null, [])).toBe(BASE_PACK_TITLE);
  });

  it("shows a missing pack's id rather than inventing a title for it", () => {
    // FR-018's case. The world has already opened under the base pack and
    // MissingPackNotice has named the pack; a two-word field repeating the
    // explanation would be a third wording of one fact.
    expect(interfacePackLabel("forged-obsidian", INSTALLED)).toBe(
      "forged-obsidian",
    );
  });
});

describe("BASE_PACK_TITLE", () => {
  it("is what the base pack's manifest actually says", () => {
    // The constant exists to avoid rendering "forge" for a frame before the
    // listing arrives, which means it is a transcribed value — the class of
    // thing this repository has watched drift before. Bound to its source
    // here so the drift is a failing test rather than a wrong label.
    const manifest = JSON.parse(
      readFileSync(
        path.resolve(
          import.meta.dirname,
          "../../../../../packs/interface/forge/interface.json",
        ),
        "utf8",
      ),
    ) as { id: string; title: string };

    expect(manifest.id).toBe("forge");
    expect(BASE_PACK_TITLE).toBe(manifest.title);
  });
});
