import { describe, expect, it } from "vitest";
import type { TurnBudgetRecord } from "@/types/combat";
import { budgetLines } from "../budgetText";

const line = (allowed: number, spent: number) => ({
  allowed,
  spent,
  remaining: allowed - spent,
});

const fresh: TurnBudgetRecord = {
  action: line(1, 0),
  bonusAction: line(1, 0),
  reaction: line(1, 0),
  movement: line(30, 0),
  legendary: null,
  unit: "ft",
};

describe("budgetLines", () => {
  it("shows what is left of what a turn affords, in turn order", () => {
    expect(budgetLines(fresh).map((l) => l.shown)).toEqual([
      "Action 1/1",
      "Bonus 1/1",
      "Reaction 1/1",
      "Move 30/30 ft",
    ]);
    expect(budgetLines(fresh)[3].spoken).toBe("Movement: 30 ft of 30 ft left");
  });

  it("shows an overspend as the debt it is, never hides it", () => {
    const spent = budgetLines({
      ...fresh,
      action: line(1, 2),
      movement: line(30, 35),
    });
    expect(spent[0].shown).toBe("Action −1/1");
    expect(spent[0].spoken).toBe("Action: overspent by 1 (2 spent of 1)");
    expect(spent[3].shown).toBe("Move −5/30 ft");
    expect(spent[3].spoken).toBe(
      "Movement: overspent by 5 ft (35 ft spent of 30 ft)",
    );
  });

  it("adds legendary actions only for a creature that has them", () => {
    const lines = budgetLines({ ...fresh, legendary: line(3, 1) });
    expect(lines.map((l) => l.key)).toContain("legendary");
    expect(lines[4].shown).toBe("Legendary 2/3");
  });
});
