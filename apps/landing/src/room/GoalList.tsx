import type { GoalId } from "@thunderforge/encounter";
import { GOAL_LABELS } from "./words.ts";

// Spec 091 FR-013: the room's goals in order, done or not said in words, not
// colour alone.

interface Props {
  goals: readonly GoalId[];
  met: readonly GoalId[];
  /** The list's name, unique on the page: each room has its own. */
  label: string;
  /** h2 in the hero, where there is no section heading above it. */
  Heading?: "h2" | "h3";
}

export default function GoalList({ goals, met, label, Heading = "h3" }: Props) {
  return (
    <section className="room-goals" aria-label={label}>
      <Heading className="room-subhead">
        Goals
      </Heading>
      <ol>
        {goals.map((goal) => {
          const done = met.includes(goal);
          return (
            <li key={goal} className={done ? "room-goal room-goal--done" : "room-goal"} data-goal={goal}>
              <span className="room-goal-mark" aria-hidden="true">
                {done ? "✓" : ""}
              </span>
              {GOAL_LABELS[goal]}
              <span className="room-goal-state">{done ? " (done)" : " (not yet)"}</span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
