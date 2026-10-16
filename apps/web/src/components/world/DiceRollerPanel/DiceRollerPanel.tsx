import { useState } from "react";
import { rollDice } from "@/api/roll";
import { RollResult } from "@/components/world/RollResult";
import { RollVisibilityPicker } from "@/components/world/RollVisibility/RollVisibilityPicker";
import { useRollVisibility } from "@/components/world/RollVisibility/useRollVisibility";
import {
  engineDiceTimings,
  prefersReducedMotion,
} from "@/engine/bevy/diceThrow";
import type { RollResolutionRecord } from "@/types/roll";

import { D20Icon } from "./D20Icon";
import { DiceFormulaHelp } from "./DiceFormulaHelp";
import { revealDelayMs } from "./revealDelay";

export interface DiceRollerPanelProps {
  worldId: string;
  /**
   * Whether the viewer is the Game Master (spec 028 FR-067). Only they see a
   * note where the server determined a result differently; everyone else sees
   * an ordinary roll, which is the point of the rule.
   */
  isGameMaster?: boolean;
  /** Whether the Bevy canvas is mounted — when it isn't, the result is
   * shown immediately rather than waiting on an animation that will
   * never play (FR-016). */
  engineReady: boolean;
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
  worldId,
  engineReady,
  isGameMaster = false,
}: DiceRollerPanelProps) {
  const [formula, setFormula] = useState("1d20");
  const [isRolling, setIsRolling] = useState(false);
  const [result, setResult] = useState<RollResolutionRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Spec 081: who sees the roll, remembered per browser.
  const [visibility, setVisibility] = useRollVisibility(isGameMaster);

  const handleRoll = async (rolled: string = formula) => {
    setFormula(rolled);
    setIsRolling(true);
    setError(null);
    setResult(null);
    try {
      const resolution = await rollDice(worldId, rolled, { visibility });

      // Spec 081 (FR-013): the board animates this roll from its world
      // event, like every other client's roll; the panel only waits for the
      // dice to settle before showing the total.
      const delay = revealDelayMs({
        engineReady,
        timings: engineDiceTimings(),
        reducedMotion: prefersReducedMotion(),
      });
      if (delay > 0) {
        await new Promise((resolve) => setTimeout(resolve, delay));
      }

      setResult(resolution);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to roll dice");
    } finally {
      setIsRolling(false);
    }
  };

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
        value={visibility}
        onChange={setVisibility}
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
