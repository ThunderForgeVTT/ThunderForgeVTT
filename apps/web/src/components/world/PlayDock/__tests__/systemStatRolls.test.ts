import { describe, expect, it } from "vitest";

import { loadStatRolls, rollsFor } from "../systemStatRolls";

/**
 * A pack that knows its own stat rolls is asked for them; a pack that does
 * not keeps the host's generic reading of `ability_data`.
 *
 * Names systems because a test has to; `check-system-registry` exempts
 * tests for that reason.
 */

describe("rollsFor", () => {
  it("prefers the pack's stat rolls over the generic reading", () => {
    const seen: unknown[] = [];
    const rolls = rollsFor(
      (systemData) => {
        seen.push(systemData);
        return [{ key: "stat-wisdom", label: "Wisdom", formula: "1d20+4" }];
      },
      {
        ability_data: { wisdom: 18, armor_class: 18 },
        trait_data: { level: 3 },
      },
    );
    expect(rolls).toEqual([
      { key: "stat-wisdom", label: "Wisdom", formula: "1d20+4" },
    ]);
    // The pack is handed every slot, not only `ability_data`: a save needs
    // proficiency and level too.
    expect(seen).toEqual([
      {
        ability_data: { wisdom: 18, armor_class: 18 },
        trait_data: { level: 3 },
      },
    ]);
  });

  it("falls back to the generic reading where the pack has none", () => {
    expect(rollsFor(null, { ability_data: { might: 3 } })).toEqual([
      { key: "stat-might", label: "Might", formula: "1d20+3" },
    ]);
    expect(rollsFor(null, null)).toEqual([]);
  });

  it("keeps only well-formed entries from a pack", () => {
    const rolls = rollsFor(
      () => [
        { key: "stat-a", label: "A", formula: "1d20" },
        { key: "stat-b", label: "B" },
        "nonsense",
        null,
      ],
      {},
    );
    expect(rolls).toEqual([{ key: "stat-a", label: "A", formula: "1d20" }]);
    expect(rollsFor(() => "not a list", {})).toEqual([]);
  });

  it("offers no stat rolls when the pack's own throws", () => {
    expect(
      rollsFor(
        () => {
          throw new Error("bad data");
        },
        { ability_data: { wisdom: 18 } },
      ),
    ).toEqual([]);
  });
});

describe("loadStatRolls", () => {
  it("finds the stat rolls a pack exports by discovery", async () => {
    const statRolls = await loadStatRolls("dnd5e");
    expect(statRolls).toBeTypeOf("function");
    expect(
      rollsFor(statRolls, {
        ability_data: { wisdom: 18, armor_class: 18 },
      }),
    ).toEqual([{ key: "stat-wisdom", label: "Wisdom", formula: "1d20+4" }]);
  });

  it("is null for no system, an unknown one, and packs that ship none", async () => {
    expect(await loadStatRolls(null)).toBeNull();
    expect(await loadStatRolls("no-such-system")).toBeNull();
    expect(await loadStatRolls("genie")).toBeNull();
    expect(await loadStatRolls("roll_for_shoes")).toBeNull();
  });
});
