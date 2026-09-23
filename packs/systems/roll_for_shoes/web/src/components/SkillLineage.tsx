import { Button } from "@thunderforge/host";

import { lineageOrder, type Skill } from "../game.ts";
import { cardTitleClass, hintClass } from "./styles.ts";

export interface SkillLineageProps {
  skills: Skill[];
  /** Disabled while a roll is in flight. Not gated on edit permission — the
   * dock is where a player sits during play, and rolling is what they do. */
  busy: boolean;
  onRoll: (skill: Skill) => void;
}

/**
 * The character's skills, each beneath the one it grew out of.
 *
 * Nested rather than sorted by level, because the nesting *is* the character's
 * history: every skill records the moment it was invented and what was being
 * attempted at the time.
 */
export function SkillLineage({ skills, busy, onRoll }: SkillLineageProps) {
  return (
    <section className="grid gap-2">
      <h3 className={cardTitleClass}>Skills</h3>
      <p className={hintClass}>A skill rolls one die for each of its levels.</p>
      <ul className="grid gap-1.5">
        {lineageOrder(skills).map(({ skill, depth }) => (
          <li
            key={skill.id}
            data-testid={`rfs-skill-${skill.id}`}
            className="flex items-center justify-between gap-3"
            style={{ paddingLeft: `${depth * 1.25}rem` }}
          >
            <span className="flex min-w-0 items-baseline gap-2">
              <span className="truncate text-sm">{skill.name}</span>
              <span className="text-xs tabular-nums text-muted-foreground">
                {skill.level}
              </span>
            </span>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={busy}
              data-testid={`rfs-roll-${skill.id}`}
              onClick={() => onRoll(skill)}
            >
              Roll {skill.level}d6
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export default SkillLineage;
