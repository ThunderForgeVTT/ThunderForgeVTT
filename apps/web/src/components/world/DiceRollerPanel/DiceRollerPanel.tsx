import { useState } from "react";
import { rollDice } from "@/api/roll";
import { RollResult } from "@/components/world/RollResult";
import { RollVisibilityPicker } from "@/components/world/RollVisibility/RollVisibilityPicker";
import { useRollVisibility } from "@/components/world/RollVisibility/useRollVisibility";
import type { RollResolutionRecord } from "@/types/roll";

import { D20Icon } from "./D20Icon";
import { DiceFormulaHelp } from "./DiceFormulaHelp";

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
 * engine for the bouncing-dice reveal, and only then shows the total —
 * gated on a fixed reveal delay matching the engine's own settle
 * animation duration when the canvas is mounted, or shown immediately
 * when it isn't (quickstart.md US4 step 3: a missing animation surface
 * never blocks or hides a resolved roll).
 */
// Mirrors `SETTLE_DURATION_SECS` in
// `crates/thunderforge-engine/src/plugins/dice_roll.rs` — kept in sync manually since
// the two live in separate build targets with no shared config.
const ANIMATION_REVEAL_MS = 1200;

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

  const handleRoll = async () => {
    setIsRolling(true);
    setError(null);
    setResult(null);
    try {
      const resolution = await rollDice(worldId, formula, { visibility });

      // Spec 081 (FR-013): the board animates this roll from its world
      // event, like every other client's roll; the panel only waits for the
      // dice to settle before showing the total.
      if (engineReady) {
        await new Promise((resolve) =>
          setTimeout(resolve, ANIMATION_REVEAL_MS),
        );
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
