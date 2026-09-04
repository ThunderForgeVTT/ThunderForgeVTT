import { describe, expect, it } from "vitest";
import { abilityRolls, statRolls } from "../characterRolls";
import type { AbilityEffectRecord, WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";

/**
 * What the in-pane character view offers a player to roll (spec 031 FR-003).
 *
 * Every test here is about a *formula*, never about a result. That is the
 * point of the split: `rollDice` is the only thing in this application allowed
 * to decide a number, so the whole of this module's job is choosing what to
 * ask it. A test asserting a total would mean this code had started computing
 * one, which is the failure the design exists to prevent.
 *
 * This repo's vitest runs in a `node` environment and has no component tests;
 * that the button reaches the table is e2e's to prove.
 */

function effect(over: Partial<AbilityEffectRecord> = {}): AbilityEffectRecord {
  return {
    id: "effect-1",
    abilityId: "ability-1",
    effectType: "DAMAGE",
    formula: "2d6+3",
    target: "SELF",
    triggerKind: "ON_USE",
    sortOrder: 0,
    ...over,
  };
}

function ability(over: Partial<WorldAbilityRecord> = {}): WorldAbilityRecord {
  return {
    id: "ability-1",
    worldId: "world-1",
    name: "Firebolt",
    description: null,
    classification: "SPELL",
    grade: null,
    gmOnly: false,
    effects: [effect()],
    myPermissionLevel: "VIEWER",
    moderated: false,
    moderationCaseId: null,
    createdAt: "",
    updatedAt: "",
    linkedFromLore: [],
    ...over,
  };
}

function entry(
  over: Partial<ActorAbilityEntryRecord> = {},
): ActorAbilityEntryRecord {
  return {
    id: "entry-1",
    actorId: "actor-1",
    abilityId: "ability-1",
    abilityName: "Firebolt",
    classification: "SPELL",
    gmOnly: false,
    ...over,
  };
}

describe("statRolls", () => {
  it("offers a d20 check per numeric score, whatever the system named them", () => {
    expect(statRolls({ might: 3, cunning: -1 })).toEqual([
      { key: "stat-might", label: "Might", formula: "1d20+3" },
      { key: "stat-cunning", label: "Cunning", formula: "1d20-1" },
    ]);
  });

  /** `1d20+0` is a correct formula that reads like a bug to the player. */
  it("writes a zero modifier as a bare d20", () => {
    expect(statRolls({ spirit: 0 })[0].formula).toBe("1d20");
  });

  /**
   * `ability_data` is a system-owned blob. Genie keeps `trained_skills` in the
   * neighbouring column, but nothing stops a pack putting a string or a list
   * here, and an unrollable value must produce no button rather than a
   * formula built out of `undefined`.
   */
  it("ignores members that are not finite numbers", () => {
    expect(
      statRolls({
        might: 2,
        title: "the Bold",
        skills: ["stealth"],
        broken: Number.NaN,
        missing: null,
      }),
    ).toEqual([{ key: "stat-might", label: "Might", formula: "1d20+2" }]);
  });

  it("has nothing to offer an actor with no system data yet", () => {
    expect(statRolls(null)).toEqual([]);
    expect(statRolls(undefined)).toEqual([]);
    expect(statRolls({})).toEqual([]);
  });

  it("reads a multi-word key as words", () => {
    expect(statRolls({ wisdom_save: 1 })[0].label).toBe("Wisdom save");
  });
});

describe("abilityRolls", () => {
  it("offers one roll per effect the ability actually carries a formula for", () => {
    const rolls = abilityRolls(
      [entry()],
      [
        ability({
          effects: [
            effect({ id: "e1", effectType: "ATTACK_ROLL", formula: "1d20+5" }),
            effect({ id: "e2", effectType: "DAMAGE", formula: "2d6" }),
            effect({ id: "e3", effectType: "MODIFIER", formula: "  " }),
          ],
        }),
      ],
    );
    expect(rolls).toEqual([
      {
        key: "ability-entry-1-e1",
        label: "Firebolt (attack)",
        formula: "1d20+5",
      },
      {
        key: "ability-entry-1-e2",
        label: "Firebolt (damage)",
        formula: "2d6",
      },
    ]);
  });

  /**
   * Spec 025 FR-023: the entry survives its ability's deletion as a tombstone.
   * It can still be *named* on the full sheet, but there is no formula left,
   * so there is nothing to press.
   */
  it("offers nothing for an ability that has been deleted", () => {
    expect(
      abilityRolls(
        [entry({ abilityId: null, classification: null })],
        [ability()],
      ),
    ).toEqual([]);
  });

  /**
   * Spec 025 FR-024b filters GM-only abilities out server-side for a non-DM,
   * so they simply are not in either list this receives. The check that
   * matters is that an unmatched entry contributes nothing — this module must
   * never grow visibility filtering of its own.
   */
  it("offers nothing for an entry with no matching catalogue record", () => {
    expect(abilityRolls([entry({ abilityId: "gone" })], [ability()])).toEqual(
      [],
    );
  });

  it("has nothing to offer before either read has landed", () => {
    expect(abilityRolls(null, [ability()])).toEqual([]);
    expect(abilityRolls([entry()], null)).toEqual([]);
  });
});
