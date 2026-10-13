import { hintClass } from "../components/styles";
import {
  DND5E_ABILITIES,
  DND5E_SKILLS,
  type AbilityId,
} from "../sheet-regions";
import { Fact, OptimisticCheckbox } from "./fields";
import { signed, toggleId, type Json } from "./values";

/** Every skill with its bonus, proficiency and expertise, and the passives. */
export default function SkillsSection({
  canEdit,
  isPending,
  skillProficiencies,
  skillExpertise,
  skillBonus,
  passivePerception,
  writeProficiencies,
}: {
  canEdit: boolean;
  isPending: boolean;
  skillProficiencies: string[];
  skillExpertise: string[];
  skillBonus: (skill: { id: string; ability: AbilityId }) => number;
  passivePerception: number;
  writeProficiencies: (patch: Json) => Promise<void>;
}) {
  return (
    <div className="grid gap-3">
      {canEdit ? (
        <p className={hintClass}>
          First box: proficient. Second box: expertise, which doubles the
          proficiency bonus.
        </p>
      ) : null}
      <ul className="grid gap-1 @md:grid-cols-2" data-testid="dnd5e-skill-list">
        {DND5E_SKILLS.map((skill) => {
          const proficient = skillProficiencies.includes(skill.id);
          const expert = skillExpertise.includes(skill.id);
          const bonus = skillBonus(skill);
          const id = `dnd5e-skill-${skill.id}`;
          const abbreviation = DND5E_ABILITIES.find(
            (a) => a.id === skill.ability,
          )?.abbreviation;
          return (
            <li
              key={skill.id}
              className="flex items-center gap-2 rounded-md px-1 py-0.5 text-sm hover:bg-muted/40"
              data-testid={id}
              data-proficient={proficient ? "true" : "false"}
              data-expertise={expert ? "true" : "false"}
            >
              {canEdit ? (
                <>
                  <OptimisticCheckbox
                    id={id}
                    testId={`${id}-proficient`}
                    checked={proficient}
                    disabled={isPending}
                    onToggle={() =>
                      // Dropping the proficiency drops the expertise
                      // that rode on it.
                      writeProficiencies({
                        skill_proficiencies: toggleId(
                          skillProficiencies,
                          skill.id,
                        ),
                        skill_expertise: proficient
                          ? skillExpertise.filter((x) => x !== skill.id)
                          : skillExpertise,
                      })
                    }
                  />
                  <OptimisticCheckbox
                    id={`${id}-expertise`}
                    testId={`${id}-expertise`}
                    checked={expert}
                    disabled={isPending || !proficient}
                    ariaLabel={`Expertise in ${skill.label}`}
                    onToggle={() =>
                      writeProficiencies({
                        skill_expertise: toggleId(skillExpertise, skill.id),
                      })
                    }
                  />
                </>
              ) : (
                <span className="w-3 text-center" aria-hidden="true">
                  {expert ? "◆" : proficient ? "●" : "○"}
                </span>
              )}
              <span
                className="w-10 font-semibold tabular-nums"
                data-testid={`${id}-bonus`}
              >
                {signed(bonus)}
              </span>
              <label htmlFor={canEdit ? id : undefined} className="flex-1">
                {skill.label}
                {!canEdit && proficient ? (
                  <span className="sr-only">
                    {expert ? " (expertise)" : " (proficient)"}
                  </span>
                ) : null}
              </label>
              <span className={`${hintClass} tracking-wider`}>
                {abbreviation}
              </span>
            </li>
          );
        })}
      </ul>
      <div className="grid grid-cols-2 gap-3 border-t border-border pt-3">
        <Fact label="Passive Perception">
          <span className="tabular-nums" data-testid="dnd5e-passive-perception">
            {passivePerception}
          </span>
        </Fact>
        <Fact label="Passive Investigation">
          <span className="tabular-nums">
            {10 +
              skillBonus({
                id: "investigation",
                ability: "intelligence",
              })}
          </span>
        </Fact>
      </div>
    </div>
  );
}
