import { Button } from "@thunderforge/host";

import {
  remainingNonSixes,
  sixesShown,
  type Skill,
  type Verdict,
} from "../game.ts";
import {
  boughtDieClass,
  cardTitleClass,
  dieClass,
  hintClass,
} from "./styles.ts";

export interface RollResultProps {
  skill: Skill;
  faces: number[];
  total: number;
  /** What the statuses came to, already included in `total`. */
  modifier: number;
  opposition: number | null;
  result: Verdict;
  /** How many dice experience has been spent on, this roll. */
  bought: number;
  xp: number;
  busy: boolean;
  onSpendXp: () => void;
}

const VERDICT_WORD: Record<Verdict, string> = {
  success: "Success",
  failure: "Failure",
  unjudged: "Unjudged",
};

/**
 * What the dice said.
 *
 * The dice are drawn as faces rather than summarised, because the game is read
 * off the individual dice: the total decides the outcome, but it is every die
 * showing a six that earns a new skill.
 *
 * A bought die is marked as bought and keeps its rolled face. It is not
 * redrawn as a six, because it did not come up a six — it counts towards the
 * advancement and changes nothing else.
 */
export function RollResult({
  skill,
  faces,
  total,
  modifier,
  opposition,
  result,
  bought,
  xp,
  busy,
  onSpendXp,
}: RollResultProps) {
  const sixes = sixesShown(faces, bought);
  const left = remainingNonSixes(faces, bought);

  // Which dice a spend has been applied to. Experience buys dice that are not
  // already sixes, and buys them left to right, so which ones are marked is a
  // presentation decision rather than a rule.
  const boughtIndices = new Set<number>(
    faces
      .map((face, index) => ({ face, index }))
      .filter(({ face }) => face !== 6)
      .slice(0, bought)
      .map(({ index }) => index),
  );

  return (
    <section className="grid gap-2">
      <h3 className={cardTitleClass}>{skill.name}</h3>

      <div className="flex flex-wrap items-center gap-1.5">
        {faces.map((face, index) => (
          <span
            key={index}
            data-testid={`rfs-die-${index}`}
            className={boughtIndices.has(index) ? boughtDieClass : dieClass}
            title={boughtIndices.has(index) ? "Bought with XP" : undefined}
          >
            {face}
          </span>
        ))}
      </div>

      <p className="text-sm">
        <span data-testid="rfs-total" className="font-semibold tabular-nums">
          {total}
        </span>{" "}
        {modifier === 0 ? null : (
          // Shown beside the total rather than folded into it silently: a
          // player who cannot see the adjustment cannot check the arithmetic,
          // and the dice above deliberately still read as they fell.
          <span className={hintClass}>
            (
            <span data-testid="rfs-modifier" className="tabular-nums">
              {modifier > 0 ? `+${modifier}` : `\u2212${Math.abs(modifier)}`}
            </span>{" "}
            from statuses){" "}
          </span>
        )}
        {opposition === null ? (
          <span className={hintClass}>with nothing to beat</span>
        ) : (
          <span className={hintClass}>against {opposition}</span>
        )}
      </p>

      <p data-testid="rfs-result" className="text-sm font-semibold">
        {VERDICT_WORD[result]}
        {result === "failure" ? (
          <span className={hintClass}> — 1 XP</span>
        ) : null}
      </p>

      <p className={hintClass}>
        {sixes} of {faces.length} showing six
      </p>

      {left > 0 ? (
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy || xp < 1}
            data-testid="rfs-spend-xp"
            onClick={onSpendXp}
          >
            Spend 1 XP for a six
          </Button>
          <span className={hintClass}>
            Only towards a new skill. The roll itself stands as it fell.
          </span>
        </div>
      ) : null}
    </section>
  );
}

export default RollResult;
