import {
  Card,
  updateActorSystemData,
  useActorSystemData,
  useUpdateTraitData,
  type ActorSheetProps,
} from "@thunderforge/host";
import GenieCharacterSheet, {
  type GenieAbilityData,
  type GenieProficiencyData,
  type GenieResourceData,
} from "./components/CharacterSheet";
import { GENIE_CONDITIONS } from "./conditions";
import { calculateMaxWishPoints } from "./derived-data.ts";

const DEFAULT_GENIE_ABILITIES: GenieAbilityData = {
  might: 0,
  cunning: 0,
  spirit: 0,
};
const DEFAULT_GENIE_PROFICIENCIES: GenieProficiencyData = {
  trained_skills: [],
};

/**
 * Spec 018 (US1/US4/US6): the Genie system's character sheet — abilities,
 * skills, and the conditions track (as CharacterSheet's own "Conditions"
 * tab) — plus a GM/owner condition-editing control.
 *
 * # Why this file is in the pack
 *
 * It used to live in `apps/web/src/pages/world/actor/`, and a registry there
 * mapped `"genie"` to it. That one entry was the last place shared web code
 * named a game system, and the reason it was there was real: this is a
 * *data-connected container*, not the plain presentational component the
 * manifest carries. It fetches, it mutates, it recomputes max Wish Points
 * from `trait_data.level`. There was nowhere in a pack to put that.
 *
 * ADR-029 settled that a bundled pack may contribute behaviour, and
 * `@thunderforge/host` says what such a pack may reach for. Between them
 * there is now somewhere, so this is there. The host discovers it by looking
 * for `packs/systems/<id>/web/src/ActorSheet.tsx` at build time — a convention,
 * not a list — so a pack that ships one is mounted and a pack that does not
 * is simply a system without a sheet.
 *
 * What is deliberately unchanged: the *internals* are as system-specific as
 * they ever were, and should stay that way. Only the mounting decision is
 * generic.
 */
export default function GenieActorSheet({ actor, canEdit }: ActorSheetProps) {
  const { data, refetch } = useActorSystemData(actor.id, "genie");
  const { updateTraits, isPending } = useUpdateTraitData(actor.id, "genie");

  const abilityData =
    (data?.ability_data as GenieAbilityData | undefined) ??
    DEFAULT_GENIE_ABILITIES;
  const proficiencyData =
    (data?.proficiency_data as GenieProficiencyData | undefined) ??
    DEFAULT_GENIE_PROFICIENCIES;
  const activeConditions: string[] = Array.isArray(
    data?.trait_data?.active_conditions,
  )
    ? (data!.trait_data!.active_conditions as string[])
    : [];
  const level: number =
    typeof data?.trait_data?.level === "number" ? data.trait_data.level : 1;
  const resourceData: GenieResourceData = (data?.resource_data as
    | GenieResourceData
    | undefined) ?? {
    current_wish_points: 0,
    max_wish_points: calculateMaxWishPoints(level),
    current_health: 1,
    max_health: 1,
  };

  const handleAbilityChange = async (
    ability: keyof GenieAbilityData,
    value: number,
  ) => {
    await updateActorSystemData(actor.id, "genie", "ability_data", {
      ...abilityData,
      [ability]: value,
    });
    await refetch();
  };

  const toggleCondition = async (key: string) => {
    const next = activeConditions.includes(key)
      ? activeConditions.filter((c) => c !== key)
      : [...activeConditions, key];
    await updateTraits({
      ...(data?.trait_data ?? {}),
      active_conditions: next,
    });
    await refetch();
  };

  const handleLevelChange = async (newLevel: number) => {
    await updateTraits({ ...(data?.trait_data ?? {}), level: newLevel });
    const newMaxWishPoints = calculateMaxWishPoints(newLevel);
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      max_wish_points: newMaxWishPoints,
      current_wish_points: Math.min(
        resourceData.current_wish_points,
        newMaxWishPoints,
      ),
    });
    await refetch();
  };

  const handleResourceChange = async (
    field: keyof GenieResourceData,
    value: number,
  ) => {
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      [field]: value,
    });
    await refetch();
  };

  return (
    <Card className="grid gap-4 p-4" data-testid="genie-actor-sheet">
      <GenieCharacterSheet
        character={{
          id: actor.id,
          name: actor.label,
          abilityData,
          proficiencyData,
          activeConditions,
          level,
          resourceData,
        }}
        isEditable={canEdit}
        onAbilityChange={(ability, value) =>
          void handleAbilityChange(ability, value)
        }
        onLevelChange={(newLevel) => void handleLevelChange(newLevel)}
        onResourceChange={(field, value) =>
          void handleResourceChange(field, value)
        }
      />
      {canEdit ? (
        <div
          className="grid gap-2 border-t pt-4"
          data-testid="genie-condition-editor"
        >
          <h3 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
            Conditions (edit)
          </h3>
          {GENIE_CONDITIONS.map((condition) => (
            <label
              key={condition.key}
              className="flex items-center gap-2 text-sm"
            >
              <input
                type="checkbox"
                checked={activeConditions.includes(condition.key)}
                disabled={isPending}
                onChange={() => void toggleCondition(condition.key)}
              />
              {condition.label}
            </label>
          ))}
        </div>
      ) : null}
    </Card>
  );
}
