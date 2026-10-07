import { Dialog } from "@/components/ui/dialog/Dialog";

/** One line of the guide: what to type, and what it rolls. */
const EXAMPLES: Array<{ formula: string; means: string }> = [
  { formula: "1d20", means: "One twenty-sided die." },
  { formula: "2d6 + 3", means: "Two six-sided dice, added, plus 3." },
  { formula: "1d20 + 5 - 1", means: "Add and subtract numbers freely." },
  { formula: "1d100", means: "A percentile roll, 1 to 100." },
  { formula: "4dF", means: "Four Fate dice: each -1, 0 or +1." },
  {
    formula: "2d20kh1",
    means: "Advantage: roll two, keep the highest one.",
  },
  {
    formula: "2d20kl1",
    means: "Disadvantage: roll two, keep the lowest one.",
  },
  { formula: "4d6dl1", means: "Roll four, drop the lowest one." },
  { formula: "1d20r1", means: "Reroll a 1, once." },
  { formula: "2d6r<3", means: "Reroll any 1 or 2, once." },
  { formula: "1d6rr1", means: "Keep rerolling while it shows a 1." },
  { formula: "1d6x", means: "Exploding: a 6 rolls another die, added." },
  { formula: "4d6min2", means: "Count any die under 2 as 2." },
  { formula: "6d10cs>=8", means: "Count successes: dice showing 8 or more." },
  {
    formula: "{1d20, 1d20}kh1 + 4",
    means: "A group: keep the best total of the terms inside.",
  },
];

/**
 * A guide to what the dice roller understands, behind the `?` beside it.
 */
export function DiceFormulaHelp() {
  return (
    <Dialog
      title="Writing a dice formula"
      description="Type a formula and press the die to roll it."
      className="max-h-[85vh] overflow-y-auto"
      trigger={
        <button
          type="button"
          data-testid="dice-formula-help-button"
          aria-label="How to write a dice formula"
          title="How to write a dice formula"
          style={{
            width: "1.75rem",
            borderRadius: "9999px",
            border: "1px solid rgba(255,255,255,0.4)",
            background: "transparent",
            color: "inherit",
            fontWeight: 600,
          }}
        >
          ?
        </button>
      }
    >
      <div className="grid gap-3 text-sm" data-testid="dice-formula-help">
        <p>
          <code>NdS</code> rolls N dice with S sides each. Leave out N for one
          die. After a dice term, add modifiers in any order. A condition is{" "}
          <code>=n</code>, <code>&gt;n</code>, <code>&gt;=n</code>,{" "}
          <code>&lt;n</code> or <code>&lt;=n</code>; a bare number means{" "}
          <code>=n</code>.
        </p>
        <table className="w-full border-collapse">
          <thead>
            <tr className="text-left text-muted-foreground">
              <th className="py-1 pr-3 font-medium">Type</th>
              <th className="py-1 font-medium">To roll</th>
            </tr>
          </thead>
          <tbody>
            {EXAMPLES.map(({ formula, means }) => (
              <tr key={formula} className="border-t border-border">
                <td className="py-1 pr-3 whitespace-nowrap">
                  <code>{formula}</code>
                </td>
                <td className="py-1">{means}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="text-muted-foreground">
          Who sees the roll is set by <strong>Roll for</strong>, under the
          formula.
        </p>
      </div>
    </Dialog>
  );
}
