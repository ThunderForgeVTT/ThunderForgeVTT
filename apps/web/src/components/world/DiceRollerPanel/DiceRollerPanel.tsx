import { RollResult } from "@/components/world/RollResult";
import { RollVisibilityPicker } from "@/components/world/RollVisibility/RollVisibilityPicker";

import { D20Icon } from "./D20Icon";
import { DiceFormulaHelp } from "./DiceFormulaHelp";
import type { DiceRoller } from "./useDiceRoller";

export interface DiceRollerPanelProps {
  /** The roller's state, held by the board (`useDiceRoller`). */
  roller: DiceRoller;
  /**
   * Whether the viewer is the Game Master (spec 028 FR-067). Only they see a
   * note where the server determined a result differently; everyone else sees
   * an ordinary roll, which is the point of the rule.
   */
  isGameMaster?: boolean;
}

/**
 * Spec 014 (US4): triggers `rollDice` (the sole source of an
 * authoritative result), forwards the response's per-die detail into the
 * engine for the reveal, and only then shows the total — gated on the
 * engine's own tumble time (spec 083, `revealDelayMs`) when the canvas is
 * mounted, or shown immediately
 * when it isn't (quickstart.md US4 step 3: a missing animation surface
 * never blocks or hides a resolved roll).
 */

/**
 * One tap, one die (owner's T017 ask): the common single rolls, so nobody
 * types `1d6` to roll a d6. `dc` is the dice parser's coin.
 */
const QUICK_DICE = [
  ["d20", "1d20"],
  ["d12", "1d12"],
  ["d10", "1d10"],
  ["d8", "1d8"],
  ["d6", "1d6"],
  ["d4", "1d4"],
  ["Coin", "1dc"],
] as const;

export function DiceRollerPanel({
  roller,
  isGameMaster = false,
}: DiceRollerPanelProps) {
  const { formula, setFormula, isRolling, result, error } = roller;
  const handleRoll = roller.roll;

  return (
    <div
      data-testid="dice-roller-panel"
      style={{
        background: "rgba(20, 20, 20, 0.85)",
        color: "white",
        padding: "0.75rem",
        borderRadius: "0.5rem",
        display: "flex",
        flexDirection: "column",
        gap: "0.5rem",
        minWidth: "12rem",
      }}
    >
      <div
        role="group"
        aria-label="Quick roll"
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(2.75rem, 1fr))",
          gap: "0.25rem",
        }}
      >
        {QUICK_DICE.map(([name, die]) => (
          <button
            key={die}
            type="button"
            data-testid={`dice-quick-${name.toLowerCase()}`}
            onClick={() => void handleRoll(die)}
            disabled={isRolling}
            aria-label={name === "Coin" ? "Flip a coin" : `Roll a ${name}`}
            style={{
              minHeight: "2.75rem",
              borderRadius: "0.5rem",
              border: "1px solid rgba(255, 255, 255, 0.2)",
              background: "transparent",
              color: "inherit",
              fontSize: "0.875rem",
              fontWeight: 600,
              fontVariantNumeric: "tabular-nums",
              touchAction: "manipulation",
              cursor: isRolling ? "wait" : "pointer",
            }}
          >
            {name}
          </button>
        ))}
      </div>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input
          data-testid="dice-formula-input"
          value={formula}
          onChange={(e) => setFormula(e.target.value)}
          placeholder="1d20+5"
          style={{ flex: 1, minWidth: 0 }}
        />
        <button
          data-testid="dice-roll-button"
          type="button"
          onClick={() => void handleRoll()}
          disabled={isRolling}
          aria-label={isRolling ? "Rolling…" : "Roll"}
          title="Roll"
          style={{
            display: "grid",
            placeItems: "center",
            background: "transparent",
            border: "none",
            padding: 0,
            cursor: isRolling ? "wait" : "pointer",
            opacity: isRolling ? 0.6 : 1,
          }}
        >
          <D20Icon size={28} />
        </button>
        <DiceFormulaHelp />
      </div>
      <RollVisibilityPicker
        isGm={isGameMaster}
        value={roller.visibility}
        onChange={roller.setVisibility}
      />
      {error ? <p data-testid="dice-roll-error">{error}</p> : null}
      {result ? (
        <div data-testid="dice-roll-result">
          {/*
            Spec 028 T102a: the result renders through `RollResult`, which is
            also where a value the server determined differently gets its quiet
            note — for the Game Master only (FR-067).
          */}
          <RollResult resolution={result} isGameMaster={isGameMaster} />
        </div>
      ) : null}
    </div>
  );
}
