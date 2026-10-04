import { useState } from "react";
import { Button, Input } from "@thunderforge/host";

import {
  DEEPEST_INDENT,
  FIND_SKILLS_FROM,
  findSkills,
  lineageOrder,
  type Skill,
} from "../game.ts";
import { cardTitleClass, fieldClass, hintClass } from "./styles.ts";

export interface SkillLineageProps {
  skills: Skill[];
  /** Disabled while a roll is in flight. Not gated on edit permission — the
   * dock is where a player sits during play, and rolling is what they do. */
  busy: boolean;
  /**
   * Why nothing can be rolled right now, when the reason is one the player
   * can act on — an advancement waiting for its answer. Shown beside the
   * skills, because a row of dead buttons with no reason reads as broken.
   */
  held?: string | null;
  onRoll: (skill: Skill) => void;
  /** Whether the person looking may change the skills by hand. */
  canEdit?: boolean;
  onRename?: (skill: Skill, name: string) => void;
  onRelevel?: (skill: Skill, level: number) => void;
  onRemove?: (skill: Skill) => void;
  onAdd?: (name: string, parentId: string | null, level: number) => void;
}

/**
 * One skill, while the skills are being changed by hand.
 *
 * The name and the level are committed when the field is left or Enter is
 * pressed, not on every keystroke: each commit is a write of the whole
 * lineage, and a level typed as "12" must not pass through "1" on its way.
 * Removing asks once more in place, because it cannot be undone and the
 * button sits a thumb's width from the level.
 */
function EditableSkill({
  skill,
  busy,
  only,
  onRename,
  onRelevel,
  onRemove,
}: {
  skill: Skill;
  busy: boolean;
  /** The last skill on the sheet, which cannot be removed. */
  only: boolean;
  onRename: (skill: Skill, name: string) => void;
  onRelevel: (skill: Skill, level: number) => void;
  onRemove: (skill: Skill) => void;
}) {
  const [name, setName] = useState(skill.name);
  const [level, setLevel] = useState(String(skill.level));
  const [removing, setRemoving] = useState(false);

  const commitName = (): void => {
    if (name.trim() === "") {
      // An emptied field is a change of mind, not a request for no name.
      setName(skill.name);
    } else if (name.trim() !== skill.name) {
      onRename(skill, name);
    }
  };
  const commitLevel = (): void => {
    const wanted = Number(level);
    if (!Number.isInteger(wanted) || wanted < 1) {
      setLevel(String(skill.level));
    } else if (wanted !== skill.level) {
      onRelevel(skill, wanted);
    }
  };

  return (
    <span className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
      <Input
        aria-label={`Name of ${skill.name}`}
        className={`${fieldClass} min-w-0 flex-1`}
        data-testid={`rfs-skillname-${skill.id}`}
        value={name}
        disabled={busy}
        onChange={(event) => setName(event.target.value)}
        onBlur={commitName}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
      />
      <Input
        type="number"
        inputMode="numeric"
        min={1}
        aria-label={`Level of ${skill.name}`}
        className={`${fieldClass} w-16`}
        data-testid={`rfs-skilllevel-${skill.id}`}
        value={level}
        disabled={busy}
        onChange={(event) => setLevel(event.target.value)}
        onBlur={commitLevel}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
      />
      {removing ? (
        <>
          <Button
            type="button"
            size="sm"
            variant="danger"
            disabled={busy}
            data-testid={`rfs-skillremove-confirm-${skill.id}`}
            onClick={() => onRemove(skill)}
          >
            Remove it
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => setRemoving(false)}
          >
            Keep
          </Button>
        </>
      ) : (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          disabled={busy || only}
          title={only ? "A character keeps at least one skill." : undefined}
          data-testid={`rfs-skillremove-${skill.id}`}
          onClick={() => setRemoving(true)}
        >
          Remove
        </Button>
      )}
    </span>
  );
}

/**
 * Writing a skill onto the sheet by hand.
 *
 * Beneath another skill it takes that skill's level plus one, so the level
 * is only asked for when the new skill stands on its own.
 */
function AddSkill({
  skills,
  busy,
  onAdd,
}: {
  skills: Skill[];
  busy: boolean;
  onAdd: (name: string, parentId: string | null, level: number) => void;
}) {
  const [name, setName] = useState("");
  const [parentId, setParentId] = useState("");
  const [level, setLevel] = useState("1");
  const parent = skills.find((skill) => skill.id === parentId);

  return (
    <form
      className="grid gap-2 border-t border-border pt-2"
      data-testid="rfs-skills-add-form"
      onSubmit={(event) => {
        event.preventDefault();
        onAdd(name, parent ? parent.id : null, Number(level));
        setName("");
      }}
    >
      <p className={hintClass}>
        Add a skill the dice did not give — one agreed at the table, or one the
        character arrived with.
      </p>
      <div className="flex flex-wrap items-end gap-2">
        <label className="grid min-w-0 flex-1 gap-1">
          <span className={hintClass}>Name</span>
          <Input
            className={fieldClass}
            data-testid="rfs-skills-add-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="grid gap-1">
          <span className={hintClass}>Grows out of</span>
          <select
            className={fieldClass}
            data-testid="rfs-skills-add-parent"
            value={parentId}
            onChange={(event) => setParentId(event.target.value)}
          >
            <option value="">Nothing — it stands on its own</option>
            {lineageOrder(skills).map(({ skill }) => (
              <option key={skill.id} value={skill.id}>
                {skill.name} ({skill.level})
              </option>
            ))}
          </select>
        </label>
        {parent ? (
          <p className={hintClass} data-testid="rfs-skills-add-level-fixed">
            Level {parent.level + 1}
          </p>
        ) : (
          <label className="grid gap-1">
            <span className={hintClass}>Level</span>
            <Input
              type="number"
              inputMode="numeric"
              min={1}
              className={`${fieldClass} w-16`}
              data-testid="rfs-skills-add-level"
              value={level}
              onChange={(event) => setLevel(event.target.value)}
            />
          </label>
        )}
        <Button
          type="submit"
          size="sm"
          disabled={busy || name.trim() === ""}
          data-testid="rfs-skills-add"
        >
          Add skill
        </Button>
      </div>
    </form>
  );
}

/**
 * The character's skills, each beneath the one it grew out of.
 *
 * Nested rather than sorted by level, because the nesting *is* the character's
 * history: every skill records the moment it was invented and what was being
 * attempted at the time.
 *
 * # Staying readable as it grows
 *
 * A character a few sessions old has a dozen skills and a lineage six deep,
 * and is read in a dock twenty-odd rem wide. Three things keep that legible.
 * Names wrap instead of being cut off — a skill's name is usually a phrase,
 * and "Talk My Way Past the…" is not a skill anyone can pick. The indent
 * stops deepening after a few levels, with a rule down the left to carry the
 * nesting, so a deep skill is not squeezed into the last third of the row.
 * And once there are enough skills to scroll, a box to find one by name sits
 * above them.
 *
 * # Changing them by hand
 *
 * Skills are discovered at the table, and not only by the dice: a name is
 * misspelt, a better one is found later, the Game Master grants something
 * outright, a pre-made character arrives with three. Anyone who may edit the
 * character can open the by-hand mode and rename, re-level, remove or add.
 * It is a mode rather than always-on fields so that the sheet at rest is a
 * list of things to roll, which is what it is for nearly all of the time.
 */
export function SkillLineage({
  skills,
  busy,
  held = null,
  onRoll,
  canEdit = false,
  onRename,
  onRelevel,
  onRemove,
  onAdd,
}: SkillLineageProps) {
  const [search, setSearch] = useState("");
  const [byHand, setByHand] = useState(false);

  const editing =
    byHand && canEdit && !!onRename && !!onRelevel && !!onRemove && !!onAdd;
  const rows = lineageOrder(skills);
  const findable = skills.length >= FIND_SKILLS_FROM;
  const shown = findable ? findSkills(rows, search) : rows;

  return (
    <section className="grid gap-2">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className={cardTitleClass}>
          Skills{" "}
          <span
            className={`${hintClass} font-normal tabular-nums`}
            data-testid="rfs-skills-count"
          >
            {skills.length}
          </span>
        </h3>
        {canEdit && onAdd ? (
          <button
            type="button"
            className="text-xs underline"
            aria-pressed={byHand}
            data-testid="rfs-skills-edit"
            onClick={() => setByHand((open) => !open)}
          >
            {byHand ? "Done changing skills" : "Change skills by hand"}
          </button>
        ) : null}
      </div>
      <p className={hintClass}>A skill rolls one die for each of its levels.</p>

      {held ? (
        <p
          role="status"
          className="rounded-lg border border-primary bg-primary/10 px-2.5 py-1.5 text-xs font-medium"
          data-testid="rfs-advancement-owed"
        >
          {held}
        </p>
      ) : null}

      {findable ? (
        <Input
          type="search"
          aria-label="Find a skill"
          placeholder="Find a skill"
          className={fieldClass}
          data-testid="rfs-skills-find"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
      ) : null}

      {shown.length === 0 ? (
        <p className={hintClass} data-testid="rfs-skills-none-found">
          No skill is called that.
        </p>
      ) : null}

      <ul className="grid gap-1.5">
        {shown.map(({ skill, depth }) => (
          <li
            key={skill.id}
            data-testid={`rfs-skill-${skill.id}`}
            data-depth={depth}
            className={
              depth > 0
                ? "flex items-center justify-between gap-3 border-l border-border pl-2"
                : "flex items-center justify-between gap-3"
            }
            style={{
              marginLeft: `${Math.min(depth, DEEPEST_INDENT) * 0.875}rem`,
            }}
          >
            {editing ? (
              <EditableSkill
                // Remounted when the stored value moves, so the fields show
                // what the server took rather than what was last typed.
                key={`${skill.name}:${skill.level}`}
                skill={skill}
                busy={busy}
                only={skills.length <= 1}
                onRename={onRename}
                onRelevel={onRelevel}
                onRemove={onRemove}
              />
            ) : (
              <>
                <span className="flex min-w-0 items-baseline gap-2">
                  <span className="min-w-0 text-sm [overflow-wrap:anywhere]">
                    {skill.name}
                  </span>
                  <span className="text-xs tabular-nums text-muted-foreground">
                    {skill.level}
                  </span>
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  className="shrink-0"
                  disabled={busy || !!held}
                  data-testid={`rfs-roll-${skill.id}`}
                  onClick={() => onRoll(skill)}
                >
                  Roll {skill.level}d6
                </Button>
              </>
            )}
          </li>
        ))}
      </ul>

      {editing ? <AddSkill skills={skills} busy={busy} onAdd={onAdd} /> : null}
    </section>
  );
}

export default SkillLineage;
