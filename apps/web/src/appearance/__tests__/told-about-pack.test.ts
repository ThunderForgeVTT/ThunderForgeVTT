import { beforeEach, describe, expect, it } from "vitest";

import {
  resetToldAboutPacks,
  shouldTellAboutMissingPack,
} from "../told-about-pack";

describe("shouldTellAboutMissingPack (FR-018: inform each participant once)", () => {
  beforeEach(() => {
    resetToldAboutPacks();
  });

  it("tells about a missing pack the first time and not the second", () => {
    expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(true);
    expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(false);
  });

  it("survives the remount that a world navigation causes", () => {
    // The regression this file exists for. `MissingPackNotice` held the record
    // in a ref, which is correct in a component that stays mounted — and every
    // world route renders its own `WorldSectionShell`, so it does not. Walking
    // Session Setup → Compendium → Scenes unmounted and remounted the notice
    // twice and told the participant three times.
    //
    // Module state is what a remount cannot reach, so navigation is modelled
    // here as exactly that: repeated calls with no reset between them.
    expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(true);
    for (const _navigation of ["compendium", "scenes", "settings", "staging"]) {
      expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(false);
    }
  });

  it("tells about a different missing pack separately", () => {
    // A second world bound to a different absent pack is a different fact.
    expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(true);
    expect(shouldTellAboutMissingPack("forged-jade")).toBe(true);
    expect(shouldTellAboutMissingPack("forged-obsidian")).toBe(false);
  });

  it("says nothing when no pack is missing", () => {
    // The overwhelmingly common case: a world on the base pack, or on a pack
    // that loaded. Silence here is the whole point of FR-018's "block
    // nothing".
    expect(shouldTellAboutMissingPack(null)).toBe(false);
    expect(shouldTellAboutMissingPack(null)).toBe(false);
  });

  it("does not treat an empty id as a pack worth reporting", () => {
    expect(shouldTellAboutMissingPack("")).toBe(false);
  });
});
