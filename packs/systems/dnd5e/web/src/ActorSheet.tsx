import { useState, type ReactNode } from "react";
import {
  Button,
  updateActorSystemData,
  useActorSystemData,
  type ActorSheetProps,
} from "@thunderforge/host";
import {
  calculateAbilityModifier,
  calculateMaxSpellSlots,
  calculateProficiencyBonus,
  calculateProficiencyBonusForChallenge,
} from "./derived-data.ts";
import {
  DND5E_ABILITIES,
  DND5E_ALIGNMENTS,
  DND5E_CHALLENGE_RATINGS,
  DND5E_CLASSES,
  DND5E_SHEET_REGIONS,
  DND5E_SIZES,
  DND5E_SKILLS,
  DND5E_XP_THRESHOLDS,
  type AbilityId,
  type SheetRegion,
} from "./sheet-regions";
import {
  cardClass,
  fieldClass,
  hintClass,
  sectionHeadingClass,
  textareaClass,
  tileClass,
} from "./components/styles";

type Json = Record<string, unknown>;
type Slot =
  | "ability_data"
  | "resource_data"
  | "proficiency_data"
  | "trait_data"
  | "spell_data";

/**
 * Columns a region takes, by the span it declares. One column in a narrow
 * sheet whatever a region asks for, so a 375px phone and the play dock read
 * it top to bottom in the declared order. Full literal strings: Tailwind only
 * builds classes it finds verbatim.
 *
 * **Container queries, not viewport ones** (`@md:`, not `md:`). The sheet is
 * mounted in two places: the actor page, and the play dock, which is about
 * 22rem wide on a monitor of any size. Asked about the viewport, the sheet
 * laid itself out in three columns inside that dock, each a few characters
 * wide. What it has to fit is the box it was given, so that is what it asks.
 */
const SPAN_CLASS: Record<SheetRegion["span"], string> = {
  1: "",
  2: "@md:col-span-2",
  3: "@md:col-span-2 @xl:col-span-3",
};

const SPELL_LEVELS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/**
 * The game's ceiling for an ability score, and the validator's. A character's
 * own progression stops at 20, but a belt can carry one past it and a giant
 * starts there, so the sheet does not refuse what the server accepts.
 */
const MAX_SCORE = 30;
/** Likewise for a spell save DC: 8 is the formula's floor. */
const MAX_SAVE_DC = 30;

/** The speeds and senses beside the walk speed and darkvision, in feet. */
const OTHER_SPEEDS = [
  { key: "speed_fly", label: "Fly" },
  { key: "speed_swim", label: "Swim" },
  { key: "speed_climb", label: "Climb" },
  { key: "speed_burrow", label: "Burrow" },
] as const;
const OTHER_SENSES = [
  { key: "blindsight", label: "Blindsight" },
  { key: "tremorsense", label: "Tremorsense" },
  { key: "truesight", label: "Truesight" },
] as const;

function num(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * A proficiency set as the server's rules read it: a list of ids. The
 * validator also accepts the older map of booleans, so a sheet that finds one
 * reads the true keys and writes a list back.
 */
function idList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }
  if (value && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .filter(([, on]) => on === true)
      .map(([id]) => id);
  }
  return [];
}

function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function signed(n: number): string {
  return n >= 0 ? `+${n}` : `${n}`;
}

function titleCase(id: string): string {
  return id.charAt(0).toUpperCase() + id.slice(1);
}

/**
 * The D&D 5e character sheet the host mounts on the actor's edit and view
 * routes and in the play dock (`apps/web/src/pages/world/actor/systemActorSheets.ts`).
 *
 * # What it writes, and the shapes the server checks
 *
 * Every field lands in one of the five per-system slots through
 * `updateActorSystemData`, which runs this pack's validator
 * (`packs/systems/dnd5e/server/src/validators.rs`) before anything is stored.
 * The sheet therefore sends whole slots, never a lone field: a write of
 * `trait_data` always carries `class` and `level`, a write of `ability_data`
 * always carries all six scores, and a proficiency set is always a list of
 * ids, because that is the shape `rules.rs` reads when it derives a save or
 * a skill bonus for the roll buttons the host draws under this sheet.
 *
 * # What it does not draw
 *
 * The host page draws the actor's portrait and token above the sheet, and
 * inventory, spells and feats (the world's ability vocabulary) below it. So
 * there is no spell list here, only the casting numbers and the slot tracker.
 *
 * # Who may edit
 *
 * `canEdit` comes from the host: the actor's own permission, and only on the
 * edit route. Without it every value is text, and no input, checkbox or
 * button is drawn. The server refuses a write from anyone else regardless.
 */
export default function DnD5eActorSheet({ actor, canEdit }: ActorSheetProps) {
  const { data, refetch } = useActorSystemData(actor.id, "dnd5e");
  const [error, setError] = useState<string | null>(null);
  const [isPending, setPending] = useState(false);

  const abilityData = (data?.ability_data ?? {}) as Json;
  const resourceData = (data?.resource_data ?? {}) as Json;
  const proficiencyData = (data?.proficiency_data ?? {}) as Json;
  const traitData = (data?.trait_data ?? {}) as Json;
  const spellData = (data?.spell_data ?? {}) as Json;

  // Base values, with the defaults a blank sheet reads as.
  const scores = Object.fromEntries(
    DND5E_ABILITIES.map((a) => [
      a.id,
      clamp(num(abilityData[a.id], 10), 1, MAX_SCORE),
    ]),
  ) as Record<AbilityId, number>;
  const mod = (ability: AbilityId) => calculateAbilityModifier(scores[ability]);
  const level = clamp(num(traitData.level, 1), 1, 20);
  const className = str(traitData.class);
  const classDecl = DND5E_CLASSES.find((c) => c.name === className);
  // A creature has a challenge rating where a character has a level, and the
  // rules read the rating only when no level is stored (`rules.rs`). The
  // sheet follows the same order so the bonus it shows is the one a roll
  // will use.
  const challenge = str(traitData.challenge);
  const hasLevel = typeof traitData.level === "number";
  const challengeBonus = hasLevel
    ? null
    : calculateProficiencyBonusForChallenge(challenge);
  const isCreature = challengeBonus !== null;
  const proficiencyBonus = challengeBonus ?? calculateProficiencyBonus(level);
  const saveProficiencies = idList(proficiencyData.saving_throw_proficiencies);
  const skillProficiencies = idList(proficiencyData.skill_proficiencies);
  // Expertise rides on proficiency, as it does in `rules.rs`: an id stored
  // here without the proficiency beside it doubles nothing.
  const skillExpertise = idList(proficiencyData.skill_expertise).filter((id) =>
    skillProficiencies.includes(id),
  );
  const skillBonus = (skill: { id: string; ability: AbilityId }) =>
    mod(skill.ability) +
    (skillProficiencies.includes(skill.id)
      ? proficiencyBonus * (skillExpertise.includes(skill.id) ? 2 : 1)
      : 0);

  const maxHp = Math.max(1, num(resourceData.max_hp, 10));
  const currentHp = clamp(num(resourceData.current_hp, maxHp), 0, maxHp);
  const tempHp = Math.max(0, num(resourceData.temporary_hp, 0));
  const hitDiceUsed = clamp(num(resourceData.hit_dice_used, 0), 0, level);
  const deathSuccesses = clamp(num(resourceData.death_save_successes, 0), 0, 3);
  const deathFailures = clamp(num(resourceData.death_save_failures, 0), 0, 3);
  const armorClass = num(abilityData.armor_class, 10 + mod("dexterity"));
  const speed = num(traitData.speed_walk, 30);
  const darkvision = num(traitData.darkvision, 0);
  const experience = Math.max(0, num(traitData.experience, 0));
  const inspiration = traitData.inspiration === true;

  const spellAbility = (str(spellData.spellcasting_ability) ||
    classDecl?.spellcasting ||
    "") as AbilityId | "";
  const spellSaveDc = spellAbility
    ? 8 + proficiencyBonus + mod(spellAbility)
    : null;
  const spellAttack = spellAbility
    ? proficiencyBonus + mod(spellAbility)
    : null;
  const slotMax = (spellData.spell_slots ?? {}) as Json;
  const slotUsed = (spellData.spell_slots_used ?? {}) as Json;

  const passivePerception =
    10 + skillBonus({ id: "perception", ability: "wisdom" });

  const write = async (slot: Slot, payload: Json) => {
    setPending(true);
    setError(null);
    try {
      await updateActorSystemData(actor.id, "dnd5e", slot, payload);
      await refetch();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setPending(false);
    }
  };

  // Each writer sends the whole slot with the fields the validator requires.
  const writeAbilities = (patch: Json) =>
    write("ability_data", { ...abilityData, ...scores, ...patch });
  const writeResources = (patch: Json) =>
    write("resource_data", { ...resourceData, max_hp: maxHp, ...patch });
  // A creature is written without the class and level a character's slot
  // always carries: storing level 1 on a goblin would take its proficiency
  // bonus away from its challenge rating. Judged on what the write leaves
  // behind, so giving a blank sheet a challenge rating makes a creature of it.
  const writeTraits = (patch: Json) => {
    const nextChallenge =
      "challenge" in patch ? str(patch.challenge) : challenge;
    const staysCreature =
      !hasLevel &&
      !("level" in patch) &&
      calculateProficiencyBonusForChallenge(nextChallenge) !== null;
    return write("trait_data", {
      ...traitData,
      ...(staysCreature ? {} : { class: className, level }),
      ...patch,
    });
  };
  /** Drop the level so the challenge rating sets the proficiency bonus. */
  const useChallengeRating = () => {
    const { level: _level, class: _class, ...rest } = traitData;
    return write("trait_data", rest);
  };

  const writeProficiencies = (patch: Json) =>
    write("proficiency_data", {
      ...proficiencyData,
      skill_proficiencies: skillProficiencies,
      skill_expertise: skillExpertise,
      saving_throw_proficiencies: saveProficiencies,
      proficiency_bonus: proficiencyBonus,
      ...patch,
    });
  const writeSpells = (patch: Json) =>
    write("spell_data", {
      ...spellData,
      ...(spellAbility ? { spellcasting_ability: spellAbility } : {}),
      ...(spellSaveDc !== null
        ? {
            spell_save_dc: clamp(spellSaveDc, 8, MAX_SAVE_DC),
            spell_attack_bonus: spellAttack,
          }
        : {}),
      ...patch,
    });

  const toggleId = (set: string[], id: string) =>
    set.includes(id) ? set.filter((x) => x !== id) : [...set, id];

  const handleLevelChange = async (nextLevel: number) => {
    const next = clamp(nextLevel, 1, 20);
    await writeTraits({ level: next });
    // Derived casting numbers live in spell_data for the engine and the
    // rules; a level change moves them, so they are rewritten here.
    if (spellAbility) {
      const pb = calculateProficiencyBonus(next);
      await write("spell_data", {
        ...spellData,
        spellcasting_ability: spellAbility,
        spell_save_dc: clamp(8 + pb + mod(spellAbility), 8, MAX_SAVE_DC),
        spell_attack_bonus: pb + mod(spellAbility),
      });
    }
  };

  const handleClassChange = async (nextClass: string) => {
    await writeTraits({ class: nextClass });
    const decl = DND5E_CLASSES.find((c) => c.name === nextClass);
    if (decl?.spellcasting && !str(spellData.spellcasting_ability)) {
      await write("spell_data", {
        ...spellData,
        spellcasting_ability: decl.spellcasting,
        spell_save_dc: clamp(
          8 + proficiencyBonus + mod(decl.spellcasting),
          8,
          MAX_SAVE_DC,
        ),
        spell_attack_bonus: proficiencyBonus + mod(decl.spellcasting),
      });
    }
  };

  const handleScoreChange = async (ability: AbilityId, value: number) => {
    const next = clamp(value, 1, MAX_SCORE);
    await writeAbilities({ [ability]: next });
    if (spellAbility === ability) {
      const m = calculateAbilityModifier(next);
      await write("spell_data", {
        ...spellData,
        spellcasting_ability: spellAbility,
        spell_save_dc: clamp(8 + proficiencyBonus + m, 8, MAX_SAVE_DC),
        spell_attack_bonus: proficiencyBonus + m,
      });
    }
  };

  const regionBody = (region: SheetRegion): ReactNode => {
    switch (region.kind) {
      case "identity":
        return (
          <div className="grid gap-3">
            <Fact label="Name">{actor.label}</Fact>
            <Field
              id="dnd5e-class"
              label="Class"
              canEdit={canEdit}
              display={className || "—"}
            >
              <select
                id="dnd5e-class"
                className={fieldClass}
                data-testid="dnd5e-class-select"
                value={classDecl ? className : className ? "__custom" : ""}
                disabled={isPending}
                onChange={(event) => {
                  const chosen = event.target.value;
                  if (chosen === "__custom") return;
                  void handleClassChange(chosen);
                }}
              >
                <option value="">Choose a class</option>
                {DND5E_CLASSES.map((c) => (
                  <option key={c.name} value={c.name}>
                    {c.name} (d{c.hitDie})
                  </option>
                ))}
                {className && !classDecl ? (
                  <option value="__custom">{className}</option>
                ) : null}
              </select>
            </Field>
            <TextField
              id="dnd5e-subclass"
              label="Subclass"
              canEdit={canEdit}
              value={str(traitData.subclass)}
              onCommit={(v) => writeTraits({ subclass: v || null })}
            />
            <div className="grid grid-cols-2 gap-3">
              <NumberField
                id="dnd5e-level"
                label="Level"
                canEdit={canEdit}
                value={level}
                min={1}
                max={20}
                onCommit={handleLevelChange}
              />
              <NumberField
                id="dnd5e-experience"
                label="Experience"
                canEdit={canEdit}
                value={experience}
                min={0}
                onCommit={(v) => writeTraits({ experience: v })}
                hint={
                  level < 20
                    ? `${DND5E_XP_THRESHOLDS[level + 1].toLocaleString()} for level ${level + 1}`
                    : undefined
                }
              />
            </div>
            <Fact label="Proficiency bonus">
              <span
                data-testid="dnd5e-proficiency-bonus"
                className="tabular-nums"
              >
                {signed(proficiencyBonus)}
              </span>
            </Fact>
            {canEdit || challenge ? (
              <div className="grid grid-cols-2 gap-3">
                <Field
                  id="dnd5e-challenge"
                  label="Challenge"
                  canEdit={canEdit}
                  display={challenge || "—"}
                >
                  <select
                    id="dnd5e-challenge"
                    className={fieldClass}
                    data-testid="dnd5e-challenge-select"
                    value={challenge}
                    disabled={isPending}
                    onChange={(event) =>
                      void writeTraits({
                        challenge: event.target.value || null,
                      })
                    }
                  >
                    <option value="">—</option>
                    {DND5E_CHALLENGE_RATINGS.map((rating) => (
                      <option key={rating} value={rating}>
                        {rating}
                      </option>
                    ))}
                  </select>
                </Field>
                <TextField
                  id="dnd5e-creature-type"
                  label="Creature type"
                  canEdit={canEdit}
                  value={str(traitData.creature_type)}
                  onCommit={(v) => writeTraits({ creature_type: v || null })}
                />
              </div>
            ) : null}
            {canEdit && challenge ? (
              <div className="grid gap-1" data-testid="dnd5e-challenge-hint">
                <p className={hintClass}>
                  {isCreature
                    ? "A creature: the proficiency bonus comes from the challenge rating. Setting a level makes it follow the level instead."
                    : "A level is set, so the proficiency bonus follows the level, not the challenge rating."}
                </p>
                {isCreature ? null : (
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    className="justify-self-start"
                    data-testid="dnd5e-use-challenge"
                    disabled={isPending}
                    onClick={() => void useChallengeRating()}
                  >
                    Use the challenge rating
                  </Button>
                )}
              </div>
            ) : null}
            <TextField
              id="dnd5e-race"
              label="Race"
              canEdit={canEdit}
              value={str(traitData.race)}
              onCommit={(v) => writeTraits({ race: v || null })}
            />
            <TextField
              id="dnd5e-background"
              label="Background"
              canEdit={canEdit}
              value={str(traitData.background)}
              onCommit={(v) => writeTraits({ background: v || null })}
            />
            <div className="grid grid-cols-2 gap-3">
              <Field
                id="dnd5e-alignment"
                label="Alignment"
                canEdit={canEdit}
                display={str(traitData.alignment) || "—"}
              >
                <select
                  id="dnd5e-alignment"
                  className={fieldClass}
                  value={str(traitData.alignment)}
                  disabled={isPending}
                  onChange={(event) =>
                    void writeTraits({ alignment: event.target.value || null })
                  }
                >
                  <option value="">—</option>
                  {DND5E_ALIGNMENTS.map((a) => (
                    <option key={a} value={a}>
                      {a}
                    </option>
                  ))}
                </select>
              </Field>
              <Field
                id="dnd5e-size"
                label="Size"
                canEdit={canEdit}
                display={titleCase(str(traitData.size, "medium"))}
              >
                <select
                  id="dnd5e-size"
                  className={fieldClass}
                  data-testid="dnd5e-size-select"
                  value={str(traitData.size, "medium")}
                  disabled={isPending}
                  onChange={(event) =>
                    void writeTraits({ size: event.target.value })
                  }
                >
                  {DND5E_SIZES.map((s) => (
                    <option key={s} value={s}>
                      {titleCase(s)}
                    </option>
                  ))}
                </select>
              </Field>
            </div>
            <div className="flex items-center gap-2">
              {canEdit ? (
                <OptimisticCheckbox
                  id="dnd5e-inspiration"
                  testId="dnd5e-inspiration"
                  checked={inspiration}
                  disabled={isPending}
                  onToggle={() => writeTraits({ inspiration: !inspiration })}
                />
              ) : (
                <span aria-hidden="true">{inspiration ? "★" : "☆"}</span>
              )}
              <label
                htmlFor={canEdit ? "dnd5e-inspiration" : undefined}
                className="text-sm"
              >
                Inspiration{canEdit ? "" : inspiration ? ": yes" : ": no"}
              </label>
            </div>
          </div>
        );

      case "abilities":
        return (
          <div className="grid grid-cols-2 gap-3 @md:grid-cols-3 @xl:grid-cols-6">
            {DND5E_ABILITIES.map((ability) => {
              const id = `dnd5e-score-${ability.id}`;
              const proficient = saveProficiencies.includes(ability.id);
              const save =
                mod(ability.id) + (proficient ? proficiencyBonus : 0);
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

      case "combat": {
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
                    onCommit={(v) =>
                      writeResources({ temporary_hp: Math.max(0, v) })
                    }
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
                    onCommit={(v) =>
                      writeAbilities({ armor_class: Math.max(0, v) })
                    }
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
                    onCommit={(v) =>
                      writeTraits({ speed_walk: Math.max(0, v) })
                    }
                  />
                ) : (
                  <span className="text-2xl font-semibold tabular-nums">
                    {speed}
                  </span>
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
                  onCommit={(v) =>
                    writeResources({ hit_dice: v.trim() || null })
                  }
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
                    onChange={(v) =>
                      writeResources({ death_save_successes: v })
                    }
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

      case "skills":
        return (
          <div className="grid gap-3">
            {canEdit ? (
              <p className={hintClass}>
                First box: proficient. Second box: expertise, which doubles the
                proficiency bonus.
              </p>
            ) : null}
            <ul
              className="grid gap-1 @md:grid-cols-2"
              data-testid="dnd5e-skill-list"
            >
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
                              skill_expertise: toggleId(
                                skillExpertise,
                                skill.id,
                              ),
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
                    <label
                      htmlFor={canEdit ? id : undefined}
                      className="flex-1"
                    >
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
                <span
                  className="tabular-nums"
                  data-testid="dnd5e-passive-perception"
                >
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

      case "spellcasting":
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
                  Slots default to the full-caster table for level {level}. Set
                  a level's slots per day to match the class. Spells themselves
                  are listed under Abilities below.
                </p>
              </div>
            ) : (
              <p className={hintClass}>
                Not a spellcaster. Choose a casting ability to track a save DC,
                attack bonus and slots.
              </p>
            )}
          </div>
        );

      case "proficiencies":
        return (
          <div className="grid gap-3">
            <ListField
              id="dnd5e-armor-proficiencies"
              label="Armour"
              canEdit={canEdit}
              items={stringList(proficiencyData.armor)}
              isSaving={isPending}
              onSave={(items) => writeProficiencies({ armor: items })}
            />
            <ListField
              id="dnd5e-weapon-proficiencies"
              label="Weapons"
              canEdit={canEdit}
              items={stringList(proficiencyData.weapons)}
              isSaving={isPending}
              onSave={(items) => writeProficiencies({ weapons: items })}
            />
            <ListField
              id="dnd5e-tool-proficiencies"
              label="Tools"
              canEdit={canEdit}
              items={stringList(proficiencyData.tools)}
              isSaving={isPending}
              onSave={(items) => writeProficiencies({ tools: items })}
            />
            <ListField
              id="dnd5e-languages"
              label="Languages"
              canEdit={canEdit}
              items={stringList(proficiencyData.languages)}
              isSaving={isPending}
              onSave={(items) => writeProficiencies({ languages: items })}
            />
          </div>
        );

      case "features":
        return (
          <div className="grid gap-3 @md:grid-cols-2">
            <ListField
              id="dnd5e-traits"
              label="Features & traits"
              canEdit={canEdit}
              items={stringList(traitData.traits)}
              rows={8}
              isSaving={isPending}
              onSave={(items) => writeTraits({ traits: items })}
            />
            <ListField
              id="dnd5e-feats"
              label="Feats"
              canEdit={canEdit}
              items={stringList(traitData.feats)}
              rows={8}
              isSaving={isPending}
              onSave={(items) => writeTraits({ feats: items })}
            />
          </div>
        );

      case "notes":
        return (
          <NotesRegion
            key={str(traitData.notes)}
            notes={str(traitData.notes)}
            canEdit={canEdit}
            isSaving={isPending}
            onSave={(notes) => writeTraits({ notes })}
          />
        );
    }
  };

  return (
    <div className="@container">
      <div
        className="grid grid-cols-1 gap-4 @md:grid-cols-2 @xl:grid-cols-3"
        data-testid="dnd5e-actor-sheet"
        data-editable={canEdit ? "true" : "false"}
        aria-busy={isPending}
      >
        {error ? (
          <p
            role="alert"
            className="rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive @md:col-span-2 @xl:col-span-3"
            data-testid="dnd5e-sheet-error"
          >
            {error}
          </p>
        ) : null}
        {DND5E_SHEET_REGIONS.map((region) => (
          <section
            key={region.id}
            aria-labelledby={`dnd5e-region-${region.id}`}
            className={`${cardClass} grid content-start gap-3 ${SPAN_CLASS[region.span]}`}
            data-testid={`dnd5e-region-${region.id}`}
            data-region={region.id}
            data-span={region.span}
          >
            <header className="grid gap-0.5">
              <h2
                id={`dnd5e-region-${region.id}`}
                className={sectionHeadingClass}
              >
                {region.title}
              </h2>
              <p className={hintClass}>{region.blurb}</p>
            </header>
            {regionBody(region)}
          </section>
        ))}
      </div>
    </div>
  );
}

/**
 * A checkbox that shows the click at once and settles on the server's answer.
 *
 * Its `checked` prop is the stored value. A plain controlled checkbox would
 * snap back to that value on the very next render and only move once the
 * refetch landed, which reads as a dead control. The draft flips immediately;
 * the `key` on the stored value resets the draft when the server replies, so
 * a rejected write puts the box back where it was.
 */
function OptimisticCheckbox(props: {
  id: string;
  testId: string;
  checked: boolean;
  disabled: boolean;
  ariaLabel?: string;
  onToggle: () => Promise<void> | void;
}) {
  return <OptimisticCheckboxDraft key={String(props.checked)} {...props} />;
}

function OptimisticCheckboxDraft({
  id,
  testId,
  checked,
  disabled,
  ariaLabel,
  onToggle,
}: {
  id: string;
  testId: string;
  checked: boolean;
  disabled: boolean;
  ariaLabel?: string;
  onToggle: () => Promise<void> | void;
}) {
  const [draft, setDraft] = useState(checked);
  return (
    <input
      id={id}
      type="checkbox"
      data-testid={testId}
      className="h-6 w-6"
      checked={draft}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={() => {
        setDraft(!draft);
        void onToggle();
      }}
    />
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-0.5">
      <div className={sectionHeadingClass}>{label}</div>
      <div className="font-medium break-words">{children}</div>
    </div>
  );
}

/** A labelled row: the control when editable, the value as text otherwise. */
function Field({
  id,
  label,
  canEdit,
  display,
  children,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  display: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid">
        {canEdit ? (
          children
        ) : (
          <span className="font-medium break-words">{display}</span>
        )}
      </div>
    </div>
  );
}

/**
 * A number that commits on blur or Enter rather than on every keystroke.
 *
 * Typing "18" passes through "1"; typing a new value passes through "". A
 * write per keystroke would send both to a validator that rejects them, and
 * would race itself besides. Keyed by the stored value in the parent, so a
 * save from elsewhere replaces an untouched draft.
 */
function NumberInput({
  id,
  testId,
  className,
  value,
  min,
  max,
  step,
  disabled,
  ariaLabel,
  onCommit,
}: {
  id: string;
  testId?: string;
  className: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (value: number) => Promise<void> | void;
}) {
  return (
    <NumberDraft key={value} initial={value}>
      {(draft, setDraft) => {
        const commit = () => {
          const parsed = Number(draft);
          if (draft.trim() === "" || !Number.isFinite(parsed)) {
            setDraft(String(value));
            return;
          }
          const rounded = Math.round(parsed);
          if (rounded !== value) void onCommit(rounded);
          else setDraft(String(value));
        };
        return (
          <input
            id={id}
            type="number"
            inputMode="numeric"
            className={className}
            data-testid={testId}
            value={draft}
            min={min}
            max={max}
            step={step}
            disabled={disabled}
            aria-label={ariaLabel}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                commit();
              }
            }}
          />
        );
      }}
    </NumberDraft>
  );
}

function NumberDraft({
  initial,
  children,
}: {
  initial: number;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(String(initial));
  return <>{children(draft, setDraft)}</>;
}

function NumberField({
  id,
  label,
  canEdit,
  value,
  min,
  max,
  step,
  hint,
  onCommit,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  hint?: string;
  onCommit: (value: number) => Promise<void> | void;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid gap-0.5">
        {canEdit ? (
          <NumberInput
            id={id}
            testId={`${id}-input`}
            className={`${fieldClass} w-24 tabular-nums`}
            value={value}
            min={min}
            max={max}
            step={step}
            onCommit={onCommit}
          />
        ) : (
          <span className="text-lg font-semibold tabular-nums" data-testid={id}>
            {value}
          </span>
        )}
        {hint ? <span className={hintClass}>{hint}</span> : null}
      </div>
    </div>
  );
}

/** Short free text, committed on blur or Enter. */
function TextField({
  id,
  label,
  canEdit,
  value,
  onCommit,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  value: string;
  onCommit: (value: string) => Promise<void> | void;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid">
        {canEdit ? (
          <TextDraft key={value} initial={value}>
            {(draft, setDraft) => {
              const commit = () => {
                if (draft.trim() !== value) void onCommit(draft.trim());
              };
              return (
                <input
                  id={id}
                  type="text"
                  className={fieldClass}
                  data-testid={`${id}-input`}
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  onBlur={commit}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      commit();
                    }
                  }}
                />
              );
            }}
          </TextDraft>
        ) : (
          <span className="font-medium break-words" data-testid={id}>
            {value || "—"}
          </span>
        )}
      </div>
    </div>
  );
}

function TextDraft({
  initial,
  children,
}: {
  initial: string;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(initial);
  return <>{children(draft, setDraft)}</>;
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

/**
 * A list of strings edited as lines. A draft and a Save rather than a write
 * per keystroke, for the same reason as the notes. Keyed by the stored list
 * in the parent.
 */
function ListField({
  id,
  label,
  canEdit,
  items,
  rows = 3,
  isSaving,
  onSave,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  items: string[];
  rows?: number;
  isSaving: boolean;
  onSave: (items: string[]) => Promise<void> | void;
}) {
  const stored = items.join("\n");
  return (
    <ListDraft key={stored} initial={stored}>
      {(draft, setDraft) => (
        <div className="grid gap-1" data-testid={id}>
          <label
            htmlFor={canEdit ? `${id}-input` : undefined}
            className={sectionHeadingClass}
          >
            {label}
          </label>
          {canEdit ? (
            <>
              <textarea
                id={`${id}-input`}
                rows={rows}
                className={textareaClass}
                data-testid={`${id}-input`}
                value={draft}
                placeholder="One per line"
                onChange={(event) => setDraft(event.target.value)}
              />
              <Button
                size="sm"
                variant="secondary"
                className="justify-self-start"
                disabled={isSaving || draft === stored}
                data-testid={`${id}-save`}
                onClick={() =>
                  void onSave(
                    draft
                      .split("\n")
                      .map((line) => line.trim())
                      .filter(Boolean),
                  )
                }
              >
                Update {label.toLowerCase()}
              </Button>
            </>
          ) : items.length === 0 ? (
            <p className={hintClass}>None.</p>
          ) : (
            <ul className="list-disc pl-5 text-sm">
              {items.map((item, index) => (
                <li key={`${item}-${index}`}>{item}</li>
              ))}
            </ul>
          )}
        </div>
      )}
    </ListDraft>
  );
}

function ListDraft({
  initial,
  children,
}: {
  initial: string;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(initial);
  return <>{children(draft, setDraft)}</>;
}

/**
 * Notes, written through `trait_data.notes` — the field this pack's `sheet`
 * block declares. A draft and a Save rather than a write per keystroke; keyed
 * by the stored text in the parent so a save from elsewhere replaces an
 * untouched draft.
 */
function NotesRegion({
  notes,
  canEdit,
  isSaving,
  onSave,
}: {
  notes: string;
  canEdit: boolean;
  isSaving: boolean;
  onSave: (notes: string) => Promise<void> | void;
}) {
  const [draft, setDraft] = useState(notes);

  if (!canEdit) {
    return notes ? (
      <p className="text-sm whitespace-pre-wrap" data-testid="dnd5e-notes">
        {notes}
      </p>
    ) : (
      <p className={hintClass} data-testid="dnd5e-notes">
        No notes.
      </p>
    );
  }

  return (
    <div className="grid gap-2">
      <label htmlFor="dnd5e-notes" className="sr-only">
        Notes
      </label>
      <textarea
        id="dnd5e-notes"
        rows={6}
        className={textareaClass}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        data-testid="dnd5e-notes-input"
      />
      <Button
        size="sm"
        className="justify-self-start"
        disabled={isSaving || draft === notes}
        onClick={() => void onSave(draft)}
        data-testid="dnd5e-notes-save"
      >
        Update notes
      </Button>
    </div>
  );
}
