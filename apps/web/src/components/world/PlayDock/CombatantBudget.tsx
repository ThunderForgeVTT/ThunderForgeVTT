import { cn } from "@/lib/utils";
import type { BudgetLineRecord, TurnBudgetRecord } from "@/types/combat";
import { budgetLines, type BudgetLineView } from "./budgetText";

/**
 * Spec 046 US5: what a combatant has left of its turn, on every seat.
 *
 * Four lines — action, bonus action, reaction, movement — each as what is
 * left of what the turn affords. Nothing here refuses anything, and nothing
 * here decides anything: the server records every spend (C9), and an
 * overspend is shown as the negative number it is, marked, so a table that
 * calls it a debt can see the debt.
 *
 * Rendered for every combatant, including one whose name a player reads as
 * "Unknown": the budget is numbers only, and names nobody.
 */
export function CombatantBudget({
  label,
  budget,
}: {
  label: string;
  budget: TurnBudgetRecord | null;
}) {
  if (!budget) return null;
  return (
    <ul
      className="flex basis-full flex-wrap gap-1 pl-10"
      data-testid="combatant-budget"
      aria-label={`What ${label} has left this turn`}
    >
      {budgetLines(budget).map((line) => (
        <BudgetPip key={line.key} line={line} />
      ))}
    </ul>
  );
}

function BudgetPip({ line }: { line: BudgetLineView }) {
  return (
    <li
      data-testid={`budget-${line.key}`}
      data-allowed={line.record.allowed}
      data-spent={line.record.spent}
      data-remaining={line.record.remaining}
      data-overspent={isOverspent(line.record) ? "true" : "false"}
      aria-label={line.spoken}
      title={line.spoken}
      className={cn(
        "rounded border px-1.5 py-0.5 text-[11px] leading-none tabular-nums",
        isOverspent(line.record)
          ? "border-destructive/60 bg-destructive/10 font-semibold text-destructive"
          : line.record.remaining <= 0
            ? "border-border text-muted-foreground line-through decoration-muted-foreground/60"
            : "border-border text-foreground",
      )}
    >
      {line.shown}
    </li>
  );
}

function isOverspent(record: BudgetLineRecord): boolean {
  return record.spent > record.allowed;
}
