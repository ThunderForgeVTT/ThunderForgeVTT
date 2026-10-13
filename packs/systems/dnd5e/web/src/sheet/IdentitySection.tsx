import { Button, type ActorSheetProps } from "@thunderforge/host";
import RollFacetsSection from "../components/RollFacetsSection";
import { hintClass, fieldClass } from "../components/styles";
import { toggleFacet, type sheetFacets } from "../derived-data.ts";
import {
  DND5E_ALIGNMENTS,
  DND5E_CHALLENGE_RATINGS,
  DND5E_CLASSES,
  DND5E_SIZES,
  DND5E_XP_THRESHOLDS,
} from "../sheet-regions";
import {
  Fact,
  Field,
  NumberField,
  OptimisticCheckbox,
  TextField,
} from "./fields";
import { signed, str, titleCase, type Json } from "./values";

/**
 * The identity region: class, level and experience, the challenge rating a
 * creature carries instead, and what shapes the character's rolls.
 */
export default function IdentitySection({
  actor,
  canEdit,
  isPending,
  traitData,
  className,
  classDecl,
  level,
  experience,
  proficiencyBonus,
  challenge,
  isCreature,
  playsInspiration,
  inspiration,
  facets,
  luckPointsLeft,
  writeTraits,
  handleClassChange,
  handleLevelChange,
  onUseChallengeRating,
}: {
  actor: ActorSheetProps["actor"];
  canEdit: boolean;
  isPending: boolean;
  traitData: Json;
  className: string;
  classDecl: (typeof DND5E_CLASSES)[number] | undefined;
  level: number;
  experience: number;
  proficiencyBonus: number;
  challenge: string;
  isCreature: boolean;
  playsInspiration: boolean;
  inspiration: boolean;
  facets: ReturnType<typeof sheetFacets>;
  luckPointsLeft: number;
  writeTraits: (patch: Json) => Promise<void>;
  handleClassChange: (nextClass: string) => Promise<void>;
  handleLevelChange: (nextLevel: number) => Promise<void>;
  onUseChallengeRating: () => Promise<void>;
}) {
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
        <span data-testid="dnd5e-proficiency-bonus" className="tabular-nums">
          {signed(proficiencyBonus)}
        </span>
      </Fact>
      {/* A character has a level, not a challenge rating or a creature
                type; a challenge already stored still shows. */}
      {(actor.isNpc && canEdit) || challenge ? (
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
              onClick={() => void onUseChallengeRating()}
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
            onChange={(event) => void writeTraits({ size: event.target.value })}
          >
            {DND5E_SIZES.map((s) => (
              <option key={s} value={s}>
                {titleCase(s)}
              </option>
            ))}
          </select>
        </Field>
      </div>
      {playsInspiration ? (
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
      ) : null}
      <RollFacetsSection
        facets={facets}
        luckPointsLeft={luckPointsLeft}
        canEdit={canEdit}
        disabled={isPending}
        onToggle={(id) => void writeTraits({ facets: toggleFacet(facets, id) })}
        onResetLuck={() => void writeTraits({ luck_points_used: 0 })}
      />
    </div>
  );
}
