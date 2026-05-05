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
import { cn } from "@/utils/cn";
import styles from "./AbilityScores.module.scss";

export interface AbilityScoresProps {
  data?: Record<string, any>;
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
    <div className={cn(styles.abilityScore, styles[color])}>
      <div className={styles.header}>
        <span className={styles.short}>{ability.short}</span>
        <span className={styles.full}>{ability.name}</span>
      </div>

      <div className={styles.scoreDisplay}>
        {editable && onUpdate ? (
          <input
            type="number"
            min="1"
            max="20"
            value={score}
            onChange={(e) => onUpdate(ability.id, parseInt(e.target.value, 10))}
            className={styles.scoreInput}
            aria-label={`${ability.name} score`}
          />
        ) : (
          <div className={styles.score}>{score}</div>
        )}
      </div>

      <div className={styles.modifier}>
        <span className={styles.modifierText}>{modifierText}</span>
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
    <Card surface="parchment" className={styles.container}>
      <div className={styles.header}>
        <h3>Ability Scores</h3>
        <p className={styles.subtitle}>
          Core attributes that define your character
        </p>
      </div>

      <div className={styles.grid}>
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
        <div className={styles.hint}>
          Click any score to modify. Range: 1-20
        </div>
      )}
    </Card>
  );
}
