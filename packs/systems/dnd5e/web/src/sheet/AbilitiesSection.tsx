import { fieldClass, hintClass, tileClass } from "../components/styles";
import { d20 } from "../statRolls";
import { DND5E_ABILITIES, type AbilityId } from "../sheet-regions";
import { NumberInput, OptimisticCheckbox } from "./fields";
import { MAX_SCORE, signed, toggleId, type Json } from "./values";

/** A number that rolls when tapped: bordered, so it reads as a button. */
export const rollableClass =
  "rounded-lg border border-border transition-colors hover:bg-muted active:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none";

/** The six scores, each with its modifier and saving throw. */
export default function AbilitiesSection({
  canEdit,
  isPending,
  scores,
  mod,
  proficiencyBonus,
  saveProficiencies,
  handleScoreChange,
  writeProficiencies,
  onRoll,
}: {
  canEdit: boolean;
  isPending: boolean;
  scores: Record<AbilityId, number>;
  mod: (ability: AbilityId) => number;
  proficiencyBonus: number;
  saveProficiencies: string[];
  handleScoreChange: (ability: AbilityId, value: number) => Promise<void>;
  writeProficiencies: (patch: Json) => Promise<void>;
  /** Given while the sheet is read: a tap on a modifier or save rolls it. */
  onRoll?: (roll: { label: string; formula: string }) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-3 @md:grid-cols-3 @xl:grid-cols-6">
      {DND5E_ABILITIES.map((ability) => {
        const id = `dnd5e-score-${ability.id}`;
        const proficient = saveProficiencies.includes(ability.id);
        const save = mod(ability.id) + (proficient ? proficiencyBonus : 0);
        return (
          <div key={ability.id} className={tileClass} data-testid={id}>
            <label
              htmlFor={canEdit ? `${id}-input` : undefined}
              className="text-sm font-semibold"
            >
              <abbr title={ability.label} className="no-underline">
                {ability.abbreviation}
              </abbr>
            </label>
            {onRoll ? (
              <button
                type="button"
                className={`${rollableClass} min-h-11 min-w-14 px-2`}
                data-testid={`dnd5e-roll-check-${ability.id}`}
                aria-label={`Roll ${ability.label} check, ${signed(mod(ability.id))}`}
                onClick={() =>
                  onRoll({
                    label: `${ability.label} check`,
                    formula: d20(mod(ability.id)),
                  })
                }
              >
                <span
                  className="text-2xl leading-none font-semibold tabular-nums"
                  data-testid={`dnd5e-mod-${ability.id}`}
                >
                  {signed(mod(ability.id))}
                </span>
              </button>
            ) : (
              <span
                className="text-2xl leading-none font-semibold tabular-nums"
                data-testid={`dnd5e-mod-${ability.id}`}
              >
                {signed(mod(ability.id))}
              </span>
            )}
            {canEdit ? (
              <NumberInput
                id={`${id}-input`}
                testId={`${id}-input`}
                className={`${fieldClass} w-16 text-center tabular-nums`}
                value={scores[ability.id]}
                min={1}
                max={MAX_SCORE}
                disabled={isPending}
                ariaLabel={`${ability.label} score`}
                onCommit={(v) => handleScoreChange(ability.id, v)}
              />
            ) : (
              <span className={`${hintClass} tabular-nums`}>
                score {scores[ability.id]}
              </span>
            )}
            {onRoll ? (
              <button
                type="button"
                className={`${rollableClass} mt-1 flex min-h-11 items-center gap-1.5 px-2 text-xs`}
                data-testid={`dnd5e-roll-save-${ability.id}`}
                aria-label={`Roll ${ability.label} save, ${signed(save)}${proficient ? ", proficient" : ""}`}
                onClick={() =>
                  onRoll({
                    label: `${ability.label} save`,
                    formula: d20(save),
                  })
                }
              >
                {proficient ? <span aria-hidden="true">●</span> : null}
                <span className="text-muted-foreground">Save</span>
                <span
                  className="font-semibold text-foreground tabular-nums"
                  data-testid={`dnd5e-save-${ability.id}-bonus`}
                >
                  {signed(save)}
                </span>
              </button>
            ) : (
              <div className="mt-1 flex items-center gap-1.5 text-xs">
                {canEdit ? (
                  <OptimisticCheckbox
                    id={`dnd5e-save-${ability.id}`}
                    testId={`dnd5e-save-${ability.id}`}
                    checked={proficient}
                    disabled={isPending}
                    ariaLabel={`${ability.label} saving throw proficiency`}
                    onToggle={() =>
                      writeProficiencies({
                        saving_throw_proficiencies: toggleId(
                          saveProficiencies,
                          ability.id,
                        ),
                      })
                    }
                  />
                ) : proficient ? (
                  <span aria-hidden="true">●</span>
                ) : null}
                <label
                  htmlFor={canEdit ? `dnd5e-save-${ability.id}` : undefined}
                  className="text-muted-foreground"
                >
                  Save{" "}
                  <span
                    className="font-semibold text-foreground tabular-nums"
                    data-testid={`dnd5e-save-${ability.id}-bonus`}
                  >
                    {signed(save)}
                  </span>
                </label>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
