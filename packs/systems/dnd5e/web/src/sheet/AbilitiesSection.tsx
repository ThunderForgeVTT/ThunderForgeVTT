import { fieldClass, hintClass, tileClass } from "../components/styles";
import { DND5E_ABILITIES, type AbilityId } from "../sheet-regions";
import { NumberInput, OptimisticCheckbox } from "./fields";
import { MAX_SCORE, signed, toggleId, type Json } from "./values";

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
}: {
  canEdit: boolean;
  isPending: boolean;
  scores: Record<AbilityId, number>;
  mod: (ability: AbilityId) => number;
  proficiencyBonus: number;
  saveProficiencies: string[];
  handleScoreChange: (ability: AbilityId, value: number) => Promise<void>;
  writeProficiencies: (patch: Json) => Promise<void>;
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
            <span
              className="text-2xl leading-none font-semibold tabular-nums"
              data-testid={`dnd5e-mod-${ability.id}`}
            >
              {signed(mod(ability.id))}
            </span>
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
          </div>
        );
      })}
    </div>
  );
}
