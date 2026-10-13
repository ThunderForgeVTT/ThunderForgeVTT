import { Button } from "@thunderforge/host";
import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
  tileClass,
} from "../components/styles";
import { calculateMaxSpellSlots } from "../derived-data.ts";
import { DND5E_ABILITIES, type AbilityId } from "../sheet-regions";
import { Field, NumberInput } from "./fields";
import {
  MAX_SAVE_DC,
  SPELL_LEVELS,
  clamp,
  num,
  signed,
  type Json,
  type Slot,
} from "./values";

/**
 * The casting numbers and the slot tracker. Spells themselves are listed by
 * the host below the sheet.
 */
export default function SpellcastingSection({
  canEdit,
  isPending,
  spellData,
  spellAbility,
  spellSaveDc,
  spellAttack,
  slotMax,
  slotUsed,
  level,
  proficiencyBonus,
  mod,
  write,
  writeSpells,
}: {
  canEdit: boolean;
  isPending: boolean;
  spellData: Json;
  spellAbility: AbilityId | "";
  spellSaveDc: number | null;
  spellAttack: number | null;
  slotMax: Json;
  slotUsed: Json;
  level: number;
  proficiencyBonus: number;
  mod: (ability: AbilityId) => number;
  write: (slot: Slot, payload: Json) => Promise<void>;
  writeSpells: (patch: Json) => Promise<void>;
}) {
  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-3 gap-2">
        <Field
          id="dnd5e-spellcasting-ability"
          label="Ability"
          canEdit={canEdit}
          display={
            spellAbility
              ? DND5E_ABILITIES.find((a) => a.id === spellAbility)?.label
              : "None"
          }
        >
          <select
            id="dnd5e-spellcasting-ability"
            className={fieldClass}
            data-testid="dnd5e-spellcasting-ability"
            value={spellAbility}
            disabled={isPending}
            onChange={(event) => {
              const next = event.target.value as AbilityId | "";
              if (!next) {
                const rest = { ...spellData };
                delete rest.spellcasting_ability;
                delete rest.spell_save_dc;
                delete rest.spell_attack_bonus;
                void write("spell_data", rest);
                return;
              }
              void write("spell_data", {
                ...spellData,
                spellcasting_ability: next,
                spell_save_dc: clamp(
                  8 + proficiencyBonus + mod(next),
                  8,
                  MAX_SAVE_DC,
                ),
                spell_attack_bonus: proficiencyBonus + mod(next),
              });
            }}
          >
            <option value="">None</option>
            {DND5E_ABILITIES.map((a) => (
              <option key={a.id} value={a.id}>
                {a.label}
              </option>
            ))}
          </select>
        </Field>
        <div className={tileClass}>
          <span className={sectionHeadingClass}>Save DC</span>
          <span
            className="text-2xl font-semibold tabular-nums"
            data-testid="dnd5e-spell-save-dc"
          >
            {spellSaveDc ?? "—"}
          </span>
        </div>
        <div className={tileClass}>
          <span className={sectionHeadingClass}>Attack</span>
          <span
            className="text-2xl font-semibold tabular-nums"
            data-testid="dnd5e-spell-attack"
          >
            {spellAttack === null ? "—" : signed(spellAttack)}
          </span>
        </div>
      </div>
      {spellAbility ? (
        <div className="grid gap-2">
          <div className="flex items-baseline justify-between">
            <span className={sectionHeadingClass}>Spell slots</span>
            {canEdit ? (
              <Button
                size="sm"
                variant="secondary"
                data-testid="dnd5e-long-rest"
                disabled={isPending}
                onClick={() =>
                  void writeSpells({
                    spell_slots_used: Object.fromEntries(
                      SPELL_LEVELS.map((n) => [`level_${n}`, 0]),
                    ),
                  })
                }
              >
                Long rest
              </Button>
            ) : null}
          </div>
          <ul className="grid grid-cols-3 gap-2 @md:grid-cols-5 @xl:grid-cols-9">
            {SPELL_LEVELS.map((n) => {
              const key = `level_${n}`;
              const max = Math.max(
                0,
                num(
                  slotMax[key],
                  Math.max(0, calculateMaxSpellSlots(level, n)),
                ),
              );
              const used = clamp(num(slotUsed[key], 0), 0, max);
              if (max === 0 && !canEdit) return null;
              return (
                <li
                  key={key}
                  className={`${tileClass} ${max === 0 ? "border-dashed bg-transparent" : ""}`}
                  data-empty={max === 0 ? "true" : "false"}
                  data-testid={`dnd5e-spell-slot-${n}`}
                >
                  <span className={sectionHeadingClass}>{n}</span>
                  <span className="text-sm font-semibold tabular-nums">
                    {max - used}
                    <span className={hintClass}> / {max}</span>
                  </span>
                  {canEdit ? (
                    <span className="flex gap-1">
                      <button
                        type="button"
                        className={`${fieldClass} h-7 w-7 px-0`}
                        aria-label={`Spend a level ${n} slot`}
                        data-testid={`dnd5e-spell-slot-${n}-spend`}
                        disabled={isPending || used >= max}
                        onClick={() =>
                          void writeSpells({
                            spell_slots_used: {
                              ...slotUsed,
                              [key]: used + 1,
                            },
                          })
                        }
                      >
                        −
                      </button>
                      <button
                        type="button"
                        className={`${fieldClass} h-7 w-7 px-0`}
                        aria-label={`Recover a level ${n} slot`}
                        disabled={isPending || used <= 0}
                        onClick={() =>
                          void writeSpells({
                            spell_slots_used: {
                              ...slotUsed,
                              [key]: used - 1,
                            },
                          })
                        }
                      >
                        +
                      </button>
                    </span>
                  ) : null}
                  {canEdit ? (
                    <NumberInput
                      id={`dnd5e-spell-slot-${n}-max`}
                      testId={`dnd5e-spell-slot-${n}-max`}
                      className={`${fieldClass} h-7 w-12 text-center text-xs`}
                      value={max}
                      min={0}
                      disabled={isPending}
                      ariaLabel={`Level ${n} slots per day`}
                      onCommit={(v) =>
                        writeSpells({
                          spell_slots: {
                            ...slotMax,
                            [key]: Math.max(0, v),
                          },
                        })
                      }
                    />
                  ) : null}
                </li>
              );
            })}
          </ul>
          <p className={hintClass}>
            Slots default to the full-caster table for level {level}. Set a
            level's slots per day to match the class. Spells themselves are
            listed under Abilities below.
          </p>
        </div>
      ) : (
        <p className={hintClass}>
          Not a spellcaster. Choose a casting ability to track a save DC, attack
          bonus and slots.
        </p>
      )}
    </div>
  );
}
