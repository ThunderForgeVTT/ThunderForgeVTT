import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
  tileClass,
} from "../components/styles";
import type { AbilityId, DND5E_CLASSES } from "../sheet-regions";
import { NumberField, NumberInput, TextField } from "./fields";
import {
  OTHER_SENSES,
  OTHER_SPEEDS,
  clamp,
  num,
  signed,
  str,
  type Json,
} from "./values";

/**
 * The combat region: hit points, armour class, initiative and speed, the
 * other speeds and senses, hit dice and death saves.
 */
export default function CombatSection({
  canEdit,
  isPending,
  resourceData,
  traitData,
  classDecl,
  mod,
  level,
  currentHp,
  maxHp,
  tempHp,
  armorClass,
  speed,
  darkvision,
  hitDiceUsed,
  deathSuccesses,
  deathFailures,
  writeAbilities,
  writeResources,
  writeTraits,
}: {
  canEdit: boolean;
  isPending: boolean;
  resourceData: Json;
  traitData: Json;
  classDecl: (typeof DND5E_CLASSES)[number] | undefined;
  mod: (ability: AbilityId) => number;
  level: number;
  currentHp: number;
  maxHp: number;
  tempHp: number;
  armorClass: number;
  speed: number;
  darkvision: number;
  hitDiceUsed: number;
  deathSuccesses: number;
  deathFailures: number;
  writeAbilities: (patch: Json) => Promise<void>;
  writeResources: (patch: Json) => Promise<void>;
  writeTraits: (patch: Json) => Promise<void>;
}) {
  const hitDie = classDecl?.hitDie;
  return (
    <div className="grid gap-4">
      <div className="grid gap-2" data-testid="dnd5e-hit-points">
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-sm font-semibold">Hit points</span>
          <span className="flex items-baseline gap-1 tabular-nums">
            {canEdit ? (
              <NumberInput
                id="dnd5e-current-hp"
                testId="dnd5e-current-hp-input"
                className={`${fieldClass} w-16 text-center`}
                value={currentHp}
                min={0}
                max={maxHp}
                disabled={isPending}
                ariaLabel="Current hit points"
                onCommit={(v) =>
                  writeResources({ current_hp: clamp(v, 0, maxHp) })
                }
              />
            ) : (
              <span
                className="text-lg font-semibold"
                data-testid="dnd5e-current-hp"
              >
                {currentHp}
              </span>
            )}
            <span className={hintClass}>/</span>
            {canEdit ? (
              <NumberInput
                id="dnd5e-max-hp"
                testId="dnd5e-max-hp-input"
                className={`${fieldClass} w-16 text-center`}
                value={maxHp}
                min={1}
                disabled={isPending}
                ariaLabel="Maximum hit points"
                onCommit={(v) => {
                  const nextMax = Math.max(1, v);
                  return writeResources({
                    max_hp: nextMax,
                    current_hp: Math.min(currentHp, nextMax),
                  });
                }}
              />
            ) : (
              <span className={hintClass}>{maxHp} max</span>
            )}
          </span>
        </div>
        <meter
          min={0}
          max={maxHp}
          low={Math.floor(maxHp / 4)}
          high={Math.floor(maxHp / 2)}
          optimum={maxHp}
          value={currentHp}
          aria-label={`Hit points: ${currentHp} of ${maxHp}`}
          aria-valuemin={0}
          aria-valuemax={maxHp}
          aria-valuenow={currentHp}
          data-testid="dnd5e-hit-point-meter"
          className="h-2 w-full"
        />
        <div className="flex items-center justify-between gap-2 text-sm">
          <label htmlFor={canEdit ? "dnd5e-temp-hp" : undefined}>
            Temporary hit points
          </label>
          {canEdit ? (
            <NumberInput
              id="dnd5e-temp-hp"
              testId="dnd5e-temp-hp-input"
              className={`${fieldClass} w-16 text-center`}
              value={tempHp}
              min={0}
              disabled={isPending}
              onCommit={(v) => writeResources({ temporary_hp: Math.max(0, v) })}
            />
          ) : (
            <span className="font-semibold tabular-nums">{tempHp}</span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-3 gap-2">
        <div className={tileClass}>
          <label
            htmlFor={canEdit ? "dnd5e-armor-class" : undefined}
            className={sectionHeadingClass}
          >
            AC
          </label>
          {canEdit ? (
            <NumberInput
              id="dnd5e-armor-class"
              testId="dnd5e-armor-class-input"
              className={`${fieldClass} w-14 text-center`}
              value={armorClass}
              min={0}
              disabled={isPending}
              onCommit={(v) => writeAbilities({ armor_class: Math.max(0, v) })}
            />
          ) : (
            <span
              className="text-2xl font-semibold tabular-nums"
              data-testid="dnd5e-armor-class"
            >
              {armorClass}
            </span>
          )}
        </div>
        <div className={tileClass}>
          <span className={sectionHeadingClass}>Initiative</span>
          <span
            className="text-2xl font-semibold tabular-nums"
            data-testid="dnd5e-initiative"
          >
            {signed(mod("dexterity"))}
          </span>
        </div>
        <div className={tileClass}>
          <label
            htmlFor={canEdit ? "dnd5e-speed" : undefined}
            className={sectionHeadingClass}
          >
            Speed
          </label>
          {canEdit ? (
            <NumberInput
              id="dnd5e-speed"
              testId="dnd5e-speed-input"
              className={`${fieldClass} w-14 text-center`}
              value={speed}
              min={0}
              step={5}
              disabled={isPending}
              onCommit={(v) => writeTraits({ speed_walk: Math.max(0, v) })}
            />
          ) : (
            <span className="text-2xl font-semibold tabular-nums">{speed}</span>
          )}
          <span className={hintClass}>ft.</span>
        </div>
      </div>

      <div className="grid gap-3">
        <NumberField
          id="dnd5e-darkvision"
          label="Darkvision (ft.)"
          canEdit={canEdit}
          value={darkvision}
          min={0}
          step={30}
          onCommit={(v) => writeTraits({ darkvision: Math.max(0, v) })}
        />
        <div
          className="grid grid-cols-2 gap-3"
          data-testid="dnd5e-other-movement"
        >
          {[...OTHER_SPEEDS, ...OTHER_SENSES]
            // Read-only, a creature that cannot fly has no fly speed
            // to show; editing, every one is offered.
            .filter(({ key }) => canEdit || num(traitData[key], 0) > 0)
            .map(({ key, label }) => (
              <NumberField
                key={key}
                id={`dnd5e-${key.replace("_", "-")}`}
                label={`${label} (ft.)`}
                canEdit={canEdit}
                value={num(traitData[key], 0)}
                min={0}
                step={5}
                onCommit={(v) => writeTraits({ [key]: Math.max(0, v) })}
              />
            ))}
        </div>
        {canEdit || str(resourceData.hit_dice) ? (
          <TextField
            id="dnd5e-hit-dice-formula"
            label="Hit point dice"
            canEdit={canEdit}
            value={str(resourceData.hit_dice)}
            onCommit={(v) => writeResources({ hit_dice: v.trim() || null })}
          />
        ) : null}
        <div className="grid gap-1">
          <div>
            <label
              htmlFor={canEdit ? "dnd5e-hit-dice-used" : undefined}
              className={sectionHeadingClass}
            >
              Hit dice
            </label>
          </div>
          <div className="flex items-center gap-2 tabular-nums">
            {canEdit ? (
              <NumberInput
                id="dnd5e-hit-dice-used"
                testId="dnd5e-hit-dice-used-input"
                className={`${fieldClass} w-16 text-center`}
                value={level - hitDiceUsed}
                min={0}
                max={level}
                disabled={isPending}
                ariaLabel="Hit dice remaining"
                onCommit={(v) =>
                  writeResources({
                    hit_dice_used: level - clamp(v, 0, level),
                  })
                }
              />
            ) : (
              <span className="font-semibold">{level - hitDiceUsed}</span>
            )}
            <span className={hintClass}>
              of {level}
              {hitDie ? ` d${hitDie}` : ""} remaining
            </span>
          </div>
        </div>
        <div className="grid gap-1">
          <div className={sectionHeadingClass}>Death saves</div>
          <div className="grid gap-1 text-sm">
            <DeathSaveRow
              label="Successes"
              id="dnd5e-death-successes"
              count={deathSuccesses}
              canEdit={canEdit}
              disabled={isPending}
              onChange={(v) => writeResources({ death_save_successes: v })}
            />
            <DeathSaveRow
              label="Failures"
              id="dnd5e-death-failures"
              count={deathFailures}
              canEdit={canEdit}
              disabled={isPending}
              onChange={(v) => writeResources({ death_save_failures: v })}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

/** Three boxes, as the printed sheet draws them. */
function DeathSaveRow({
  label,
  id,
  count,
  canEdit,
  disabled,
  onChange,
}: {
  label: string;
  id: string;
  count: number;
  canEdit: boolean;
  disabled: boolean;
  onChange: (count: number) => Promise<void> | void;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span id={`${id}-label`}>{label}</span>
      <span
        className="flex gap-1"
        role={canEdit ? "group" : undefined}
        aria-labelledby={`${id}-label`}
        data-testid={id}
        data-count={count}
      >
        {[1, 2, 3].map((n) =>
          canEdit ? (
            <input
              key={n}
              type="checkbox"
              className="h-6 w-6"
              aria-label={`${label}: ${n}`}
              checked={count >= n}
              disabled={disabled}
              onChange={() => void onChange(count >= n ? n - 1 : n)}
            />
          ) : (
            <span key={n} aria-hidden="true">
              {count >= n ? "●" : "○"}
            </span>
          ),
        )}
        {!canEdit ? <span className="sr-only">{count} of 3</span> : null}
      </span>
    </div>
  );
}
