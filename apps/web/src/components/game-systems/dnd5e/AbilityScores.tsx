/**
 * AbilityScores.tsx
 * D&D 5e Character Ability Scores Component
 *
 * Phase 4.8.1: System-Aware React Components (Phase E.1)
 *
 * Displays the 6 core ability scores (Strength, Dexterity, Constitution, Intelligence, Wisdom, Charisma)
 * with their modifiers calculated from base_data.
 *
 * Each ability score:
 * - Shows raw score (1-20)
 * - Calculates modifier: (score - 10) / 2, rounded down
 * - Color-coded: red (1-3), orange (4-7), yellow (8-11), green (12-15), blue (16-19), purple (20)
 */

import type { ReactNode } from "react";
import { Card } from "@/components/ui/card/Card";
import { cn } from "@/lib/utils";

export interface AbilityScoresProps {
  /** Ability id -> raw score, as stored in the actor's `ability_data` JSONB. */
  data?: Record<string, number>;
  editable?: boolean;
  onUpdate?: (abilityId: string, score: number) => void;
}

const ABILITIES = [
  { id: "strength", name: "Strength", short: "STR" },
  { id: "dexterity", name: "Dexterity", short: "DEX" },
  { id: "constitution", name: "Constitution", short: "CON" },
  { id: "intelligence", name: "Intelligence", short: "INT" },
  { id: "wisdom", name: "Wisdom", short: "WIS" },
  { id: "charisma", name: "Charisma", short: "CHA" },
];

function calculateModifier(score: number): number {
  return Math.floor((score - 10) / 2);
}

const SCORE_COLOR_CLASSES: Record<string, string> = {
  critical: "border-red-500 bg-red-500/10",
  poor: "border-orange-500 bg-orange-500/10",
  average: "border-amber-500 bg-amber-500/10",
  good: "border-green-500 bg-green-500/10",
  excellent: "border-blue-500 bg-blue-500/10",
  legendary: "border-purple-500 bg-purple-500/10",
};

function getScoreColor(score: number): string {
  if (score <= 3) return "critical";
  if (score <= 7) return "poor";
  if (score <= 11) return "average";
  if (score <= 15) return "good";
  if (score <= 19) return "excellent";
  return "legendary";
}

function AbilityScore({
  ability,
  score,
  editable,
  onUpdate,
}: {
  ability: (typeof ABILITIES)[0];
  score: number;
  editable?: boolean;
  onUpdate?: (abilityId: string, score: number) => void;
}): ReactNode {
  const modifier = calculateModifier(score);
  const color = getScoreColor(score);
  const modifierText = modifier >= 0 ? `+${modifier}` : `${modifier}`;

  return (
    <div
      className={cn(
        "grid gap-2 rounded-lg border-2 p-3 text-center",
        SCORE_COLOR_CLASSES[color],
      )}
    >
      <div className="grid gap-0.5">
        <span className="text-xs font-semibold tracking-widest uppercase">
          {ability.short}
        </span>
        <span className="text-[0.7rem] text-muted-foreground">
          {ability.name}
        </span>
      </div>

      <div>
        {editable && onUpdate ? (
          <input
            type="number"
            min="1"
            max="20"
            value={score}
            onChange={(e) => onUpdate(ability.id, parseInt(e.target.value, 10))}
            className="w-full rounded-md border border-input bg-transparent py-1 text-center text-2xl font-semibold outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
            aria-label={`${ability.name} score`}
          />
        ) : (
          <div className="text-2xl font-semibold">{score}</div>
        )}
      </div>

      <div>
        <span className="text-sm font-medium text-muted-foreground">
          {modifierText}
        </span>
      </div>
    </div>
  );
}

/**
 * Display all 6 D&D 5e ability scores
 *
 * Usage:
 * ```tsx
 * <AbilityScores
 *   data={{ strength: 15, dexterity: 14, constitution: 13, intelligence: 10, wisdom: 12, charisma: 8 }}
 *   editable={true}
 *   onUpdate={(abilityId, score) => mutateToken(...)}
 * />
 * ```
 */
export function AbilityScores({
  data = {},
  editable = false,
  onUpdate,
}: AbilityScoresProps) {
  return (
    <Card surface="parchment" className="grid gap-4 p-6">
      <div>
        <h3 className="text-lg font-semibold">Ability Scores</h3>
        <p className="text-sm text-muted-foreground">
          Core attributes that define your character
        </p>
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-6">
        {ABILITIES.map((ability) => (
          <AbilityScore
            key={ability.id}
            ability={ability}
            score={data[ability.id] ?? 10}
            editable={editable}
            onUpdate={onUpdate}
          />
        ))}
      </div>

      {editable && (
        <div className="text-center text-xs text-muted-foreground">
          Click any score to modify. Range: 1-20
        </div>
      )}
    </Card>
  );
}
