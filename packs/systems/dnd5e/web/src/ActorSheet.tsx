import { useState, type ReactNode } from "react";
import {
  updateActorSystemData,
  useActorSystemData,
  useWorldSystemSettings,
  type ActorSheetProps,
} from "@thunderforge/host";
import {
  calculateAbilityModifier,
  calculateLuckPointsLeft,
  calculateProficiencyBonus,
  calculateProficiencyBonusForChallenge,
  sheetFacets,
} from "./derived-data.ts";
import AbilitiesSection from "./sheet/AbilitiesSection";
import ClassesSection from "./sheet/ClassesSection";
import CoinsSection from "./sheet/CoinsSection";
import CombatSection from "./sheet/CombatSection";
import DefencesSection from "./sheet/DefencesSection";
import FeaturesSection from "./sheet/FeaturesSection";
import IdentitySection from "./sheet/IdentitySection";
import NotesRegion from "./sheet/NotesRegion";
import PersonaSection from "./sheet/PersonaSection";
import ProficienciesSection from "./sheet/ProficienciesSection";
import SkillsSection from "./sheet/SkillsSection";
import SpellcastingSection from "./sheet/SpellcastingSection";
import StagedLinks from "./sheet/StagedLinks";
import {
  classesPatch,
  readClasses,
  readCoins,
  readDefences,
  readPersona,
} from "./sheet/data";
import {
  MAX_SAVE_DC,
  MAX_SCORE,
  clamp,
  idList,
  num,
  str,
  type Json,
  type Slot,
} from "./sheet/values";
import {
  DND5E_ABILITIES,
  DND5E_CLASSES,
  DND5E_SHEET_REGIONS,
  type AbilityId,
  type SheetRegion,
} from "./sheet-regions";
import { cardClass, hintClass, sectionHeadingClass } from "./components/styles";

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
  // Spec 084: what shapes this character's rolls, and the Luck Points the
  // Lucky feat has left until a long rest.
  const facets = sheetFacets(traitData.facets);
  const luckPointsLeft = calculateLuckPointsLeft(
    proficiencyBonus,
    num(traitData.luck_points_used, 0),
  );
  // A table that does not award Inspiration turns it off in the world's
  // settings. Shown until the read lands and when it fails: the declared
  // default is on, and a control that flickers in is worse than one that
  // leaves.
  const playsInspiration =
    useWorldSystemSettings(actor.worldId).valueOf("inspiration") !== false;

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
          <IdentitySection
            actor={actor}
            canEdit={canEdit}
            isPending={isPending}
            traitData={traitData}
            className={className}
            classDecl={classDecl}
            level={level}
            experience={experience}
            proficiencyBonus={proficiencyBonus}
            challenge={challenge}
            isCreature={isCreature}
            playsInspiration={playsInspiration}
            inspiration={inspiration}
            facets={facets}
            luckPointsLeft={luckPointsLeft}
            writeTraits={writeTraits}
            handleClassChange={handleClassChange}
            handleLevelChange={handleLevelChange}
            onUseChallengeRating={useChallengeRating}
          />
        );

      case "abilities":
        return (
          <AbilitiesSection
            canEdit={canEdit}
            isPending={isPending}
            scores={scores}
            mod={mod}
            proficiencyBonus={proficiencyBonus}
            saveProficiencies={saveProficiencies}
            handleScoreChange={handleScoreChange}
            writeProficiencies={writeProficiencies}
          />
        );

      case "combat":
        return (
          <CombatSection
            canEdit={canEdit}
            isPending={isPending}
            resourceData={resourceData}
            traitData={traitData}
            classDecl={classDecl}
            mod={mod}
            level={level}
            currentHp={currentHp}
            maxHp={maxHp}
            tempHp={tempHp}
            armorClass={armorClass}
            speed={speed}
            darkvision={darkvision}
            hitDiceUsed={hitDiceUsed}
            deathSuccesses={deathSuccesses}
            deathFailures={deathFailures}
            writeAbilities={writeAbilities}
            writeResources={writeResources}
            writeTraits={writeTraits}
          />
        );

      case "skills":
        return (
          <SkillsSection
            canEdit={canEdit}
            isPending={isPending}
            skillProficiencies={skillProficiencies}
            skillExpertise={skillExpertise}
            skillBonus={skillBonus}
            passivePerception={passivePerception}
            writeProficiencies={writeProficiencies}
          />
        );

      case "spellcasting":
        return (
          <SpellcastingSection
            canEdit={canEdit}
            isPending={isPending}
            spellData={spellData}
            spellAbility={spellAbility}
            spellSaveDc={spellSaveDc}
            spellAttack={spellAttack}
            slotMax={slotMax}
            slotUsed={slotUsed}
            level={level}
            proficiencyBonus={proficiencyBonus}
            mod={mod}
            write={write}
            writeSpells={writeSpells}
          />
        );

      case "proficiencies":
        return (
          <ProficienciesSection
            canEdit={canEdit}
            isPending={isPending}
            proficiencyData={proficiencyData}
            writeProficiencies={writeProficiencies}
          />
        );

      case "features":
        return (
          <FeaturesSection
            canEdit={canEdit}
            isPending={isPending}
            traitData={traitData}
            writeTraits={writeTraits}
          />
        );

      // Spec 048: sections in their own files under `sheet/`; this only
      // mounts them and says where their writes go.
      case "classes":
        return (
          <ClassesSection
            classes={readClasses(traitData)}
            canEdit={canEdit}
            disabled={isPending}
            onSave={(classes) => void writeTraits(classesPatch(classes))}
          />
        );

      case "defences":
        return (
          <DefencesSection
            defences={readDefences(traitData)}
            canEdit={canEdit}
            disabled={isPending}
            onSave={(key, ids) => void writeTraits({ [key]: ids })}
          />
        );

      case "coins":
        return (
          <CoinsSection
            coins={readCoins(resourceData)}
            canEdit={canEdit}
            disabled={isPending}
            onSave={(coins) => void writeResources({ coins })}
          />
        );

      case "persona":
        return (
          <PersonaSection
            persona={readPersona(traitData)}
            canEdit={canEdit}
            disabled={isPending}
            onSave={(key, value) => void writeTraits({ [key]: value })}
          />
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
      <StagedLinks actorId={actor.id} worldId={actor.worldId} />
    </div>
  );
}
