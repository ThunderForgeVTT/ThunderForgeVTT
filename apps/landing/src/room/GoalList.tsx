import { GOALS, type GoalId } from "@thunderforge/encounter";
import { GOAL_LABELS } from "./words.ts";

// Spec 091 FR-013: the four goals in order, done or not said in words, not
// colour alone.

export default function GoalList({ met }: { met: readonly GoalId[] }) {
  return (
    <section className="room-goals" aria-labelledby="room-goals-title">
      <h3 id="room-goals-title" className="room-subhead">
        Goals
      </h3>
      <ol>
        {GOALS.map((goal) => {
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
