import { useState } from "react";
import { Button } from "@thunderforge/host";
import { HIT_DICE, classesProblem, type ClassEntry, type HitDie } from "./data";
import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
} from "../components/styles";

/**
 * Spec 048: the classes a character has levels in. With classes listed, the
 * character's level is their sum and its class is the first one, so a
 * multiclass character reads as one level and many classes.
 *
 * Edited as a list and saved as one, because the level follows the whole
 * list: saving one row at a time would pass through levels the validator
 * refuses.
 */
export default function ClassesSection({
  classes,
  canEdit,
  disabled,
  onSave,
}: {
  classes: ClassEntry[];
  canEdit: boolean;
  disabled: boolean;
  onSave: (classes: ClassEntry[]) => void;
}) {
  if (!canEdit) {
    return (
      <div className="grid gap-1" data-testid="dnd5e-classes">
        <div className={sectionHeadingClass}>Classes</div>
        {classes.length === 0 ? (
          <span className={hintClass}>No classes recorded.</span>
        ) : (
          <ul className="grid gap-1">
            {classes.map((entry, index) => (
              <li
                key={index}
                className="text-sm"
                data-testid={`dnd5e-class-${index}`}
              >
                <span className="font-medium">
                  {entry.name} {entry.level}
                </span>
                {entry.subclass ? ` (${entry.subclass})` : ""}
                <span className={hintClass}> · {entry.hitDie}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    );
  }
  return (
    <ClassesDraft
      key={JSON.stringify(classes)}
      classes={classes}
      disabled={disabled}
      onSave={onSave}
    />
  );
}

function ClassesDraft({
  classes,
  disabled,
  onSave,
}: {
  classes: ClassEntry[];
  disabled: boolean;
  onSave: (classes: ClassEntry[]) => void;
}) {
  const [draft, setDraft] = useState(classes);
  const problem = classesProblem(draft);
  const changed = JSON.stringify(draft) !== JSON.stringify(classes);
  const edit = (index: number, patch: Partial<ClassEntry>) =>
    setDraft(draft.map((row, i) => (i === index ? { ...row, ...patch } : row)));

  return (
    <div className="grid gap-2" data-testid="dnd5e-classes">
      <div className={sectionHeadingClass}>Classes</div>
      {draft.map((entry, index) => (
        <fieldset
          key={index}
          className="grid grid-cols-2 gap-2 @md:grid-cols-[2fr_2fr_1fr_1fr_auto]"
          data-testid={`dnd5e-class-${index}`}
        >
          <legend className="sr-only">Class {index + 1}</legend>
          <input
            aria-label="Class"
            className={fieldClass}
            value={entry.name}
            disabled={disabled}
            onChange={(event) => edit(index, { name: event.target.value })}
          />
          <input
            aria-label="Subclass"
            className={fieldClass}
            value={entry.subclass}
            disabled={disabled}
            onChange={(event) => edit(index, { subclass: event.target.value })}
          />
          <input
            aria-label="Level"
            type="number"
            min={1}
            max={20}
            className={fieldClass}
            value={entry.level}
            disabled={disabled}
            onChange={(event) =>
              edit(index, {
                level: Math.min(
                  20,
                  Math.max(1, Number(event.target.value) || 1),
                ),
              })
            }
          />
          <select
            aria-label="Hit die"
            className={fieldClass}
            value={entry.hitDie}
            disabled={disabled}
            onChange={(event) =>
              edit(index, { hitDie: event.target.value as HitDie })
            }
          >
            {HIT_DICE.map((die) => (
              <option key={die} value={die}>
                {die}
              </option>
            ))}
          </select>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={disabled}
            onClick={() => setDraft(draft.filter((_, i) => i !== index))}
          >
            Remove
          </Button>
        </fieldset>
      ))}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="secondary"
          size="sm"
          disabled={disabled}
          data-testid="dnd5e-class-add"
          onClick={() =>
            setDraft([
              ...draft,
              { name: "", subclass: "", level: 1, hitDie: "d8" },
            ])
          }
        >
          Add a class
        </Button>
        <Button
          type="button"
          size="sm"
          disabled={disabled || !changed || problem !== null}
          data-testid="dnd5e-classes-save"
          onClick={() => onSave(draft)}
        >
          Save classes
        </Button>
        {problem ? (
          <span role="status" className={hintClass}>
            {problem}
          </span>
        ) : null}
      </div>
    </div>
  );
}
