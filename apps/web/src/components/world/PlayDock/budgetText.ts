import type { BudgetLineRecord, TurnBudgetRecord } from "@/types/combat";

/** One line of a combatant's budget, ready to show and to say. */
export interface BudgetLineView {
  key: "action" | "bonus-action" | "reaction" | "movement" | "legendary";
  record: BudgetLineRecord;
  /** On the pip: what is left of what the turn affords, "1/1", "−5/30 ft". */
  shown: string;
  /** To a screen reader and on hover: the same, in a sentence. */
  spoken: string;
}

/** A minus sign, not a hyphen, for a debt. */
function signed(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return rounded < 0 ? `−${Math.abs(rounded)}` : `${rounded}`;
}

function view(
  key: BudgetLineView["key"],
  names: [shown: string, spoken: string],
  record: BudgetLineRecord,
  unit = "",
): BudgetLineView {
  const [short, name] = names;
  const suffix = unit ? ` ${unit}` : "";
  const shown = `${short} ${signed(record.remaining)}/${signed(record.allowed)}${suffix}`;
  const over = record.spent - record.allowed;
  const spoken =
    over > 0
      ? `${name}: overspent by ${signed(over)}${suffix} (${signed(record.spent)}${suffix} spent of ${signed(record.allowed)}${suffix})`
      : `${name}: ${signed(record.remaining)}${suffix} of ${signed(record.allowed)}${suffix} left`;
  return { key, record, shown, spoken };
}

/**
 * The lines a budget is shown as, in the order a turn is spoken of: action,
 * bonus action, reaction, movement, then legendary actions when the creature
 * has them.
 */
export function budgetLines(budget: TurnBudgetRecord): BudgetLineView[] {
  const lines = [
    view("action", ["Action", "Action"], budget.action),
    view("bonus-action", ["Bonus", "Bonus action"], budget.bonusAction),
    view("reaction", ["Reaction", "Reaction"], budget.reaction),
    view("movement", ["Move", "Movement"], budget.movement, budget.unit),
  ];
  if (budget.legendary) {
    lines.push(
      view("legendary", ["Legendary", "Legendary actions"], budget.legendary),
    );
  }
  return lines;
}
