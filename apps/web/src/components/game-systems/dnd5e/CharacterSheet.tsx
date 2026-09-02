/**
 * CharacterSheet.tsx
 * D&D 5e Character Sheet Component
 *
 * Phase 4.8.1: System-Aware React Components (Phase E.1-E.2)
 *
 * Main component that displays the full D&D 5e character sheet by composing:
 * - AbilityScores: Core ability scores with modifiers
 * - SkillsList: All 18 skills with proficiency and passive checks
 * - Spellbook: Known spells and spell slots (future)
 * - Resources: HP, hit dice, spell slots, etc. (future)
 *
 * Data is fetched and updated directly via GraphQL (RxDB removed — see
 * useActorSystemData.ts / useUpdateActorData.ts).
 *
 * Phase E.2 Integration:
 * ✅ useActorSystemData - direct GraphQL fetch
 * ✅ useUpdateActorData - GraphQL mutation
 * ✅ useGameSystemManifest - System calculators
 */

import type { ReactNode } from "react";
import { useEffect } from "react";
import { Loader2 } from "lucide-react";
import { Container } from "@/components/ui/container/Container";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { Card } from "@/components/ui/card/Card";
import { cn } from "@/lib/utils";
import {
  booleanEntries,
  numberEntries,
  readNumber,
  readString,
} from "@/lib/systemData";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import { useUpdateActorData } from "@/hooks/useUpdateActorData";
import { useGameSystemManifest } from "@/contexts/game-system-context";
import { AbilityScores } from "./AbilityScores";
import { SkillsList } from "./SkillsList";

/**
 * The slice of the `spell_data` JSONB column this sheet renders. That column
 * arrives open from the server (`Record<string, unknown>` — its shape is the
 * loaded game system's business), so the shape the sheet actually depends on
 * is written down once here instead of being re-assumed at every use site.
 */
interface Dnd5eSpellData {
  spellcasting_ability?: string;
  spell_slots?: Record<string, number>;
  known_spells?: string[];
}

/**
 * Narrow the open `spell_data` column to the three fields this sheet reads.
 *
 * Each field is checked on its own: a row holding a readable spellcasting
 * ability but a malformed `spell_slots` still renders the ability. Anything
 * unreadable is dropped, so the sheet falls through to its "no spell slots
 * recorded" copy rather than rendering a stringified object.
 */
function toDnd5eSpellData(
  raw: Record<string, unknown> | undefined,
): Dnd5eSpellData {
  const slots = raw?.spell_slots;
  const known = raw?.known_spells;
  return {
    spellcasting_ability: readString(raw, "spellcasting_ability"),
    spell_slots:
      typeof slots === "object" && slots !== null && !Array.isArray(slots)
        ? numberEntries(slots as Record<string, unknown>)
        : undefined,
    known_spells: Array.isArray(known)
      ? known.filter((entry): entry is string => typeof entry === "string")
      : undefined,
  };
}

export interface CharacterSheetProps {
  actorId: string;
  actorName: string;
  gameSystemId?: string;
  editable?: boolean;
  onError?: (error: Error) => void;
}

/**
 * Main D&D 5e Character Sheet Component
 *
 * Usage:
 * ```tsx
 * <CharacterSheet
 *   actorId="actor-123"
 *   actorName="Aragorn"
 *   gameSystemId="dnd5e"
 *   editable={true}
 * />
 * ```
 *
 * ✅ E2.1: useActorSystemData loads data via direct GraphQL fetch
 * ✅ E2.2: useUpdateActorData sends the GraphQL mutation directly
 * ✅ E2.3: useGameSystemManifest provides system calculators (no prop drilling)
 */
export function CharacterSheet({
  actorId,
  actorName,
  gameSystemId = "dnd5e",
  editable = false,
  onError,
}: CharacterSheetProps): ReactNode {
  // E2.1: Load actor system data via direct GraphQL fetch
  const {
    data: actorData,
    loading: dataLoading,
    error: dataError,
  } = useActorSystemData(actorId, gameSystemId);

  // E2.2: Setup mutation handler with optimistic updates
  const {
    mutate: updateActorData,
    isPending: isMutating,
    error: mutationError,
  } = useUpdateActorData(actorId, gameSystemId);

  // E2.3: Load system manifest for calculators
  const { loading: manifestLoading, error: manifestError } =
    useGameSystemManifest(gameSystemId);

  const isLoading = dataLoading || manifestLoading;
  const isOptimistic = isMutating;
  const currentError = dataError || manifestError || mutationError;

  // Notify parent of errors
  useEffect(() => {
    if (currentError && onError) {
      onError(currentError);
    }
  }, [currentError, onError]);

  if (isLoading) {
    return (
      <Container className="grid place-items-center py-16">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Loader2 className="size-5 animate-spin" aria-hidden="true" />
          <p>Loading character sheet...</p>
        </div>
      </Container>
    );
  }

  if (currentError) {
    return (
      <Container className="grid gap-3 py-16 text-center">
        <h2 className="text-xl font-semibold">Failed to Load Character</h2>
        <p className="text-muted-foreground">{currentError.message}</p>
        <button
          onClick={() => window.location.reload()}
          className="justify-self-center text-sm font-medium text-primary underline-offset-4 hover:underline"
        >
          Reload
        </button>
      </Container>
    );
  }

  if (!actorData) {
    return (
      <Container className="grid place-items-center py-16">
        <p className="text-muted-foreground">No character data found</p>
      </Container>
    );
  }

  // The raw columns are what gets written back: an update spreads the column
  // it edits, and spreading a narrowed copy instead would silently delete
  // every key this sheet cannot read (another system's fields, a newer
  // field this build predates).
  const abilityData = actorData.ability_data ?? {};
  const proficiencyData = actorData.proficiency_data ?? {};

  // The narrowed views are what gets rendered.
  const abilityScores = numberEntries(abilityData);
  const proficiencies = booleanEntries(proficiencyData);
  const resourceData = actorData.resource_data;
  const traitData = actorData.trait_data;
  const spellData = toDnd5eSpellData(actorData.spell_data);

  const handleUpdateAbility = async (abilityId: string, score: number) => {
    try {
      const updated = { ...abilityData, [abilityId]: score };
      await updateActorData("ability_data", updated);
    } catch (error) {
      onError?.(error instanceof Error ? error : new Error(String(error)));
    }
  };

  const handleToggleProficiency = async (
    skillId: string,
    proficient: boolean,
  ) => {
    try {
      const updated = { ...proficiencyData, [skillId]: proficient };
      await updateActorData("proficiency_data", updated);
    } catch (error) {
      onError?.(error instanceof Error ? error : new Error(String(error)));
    }
  };

  const dexModifier = Math.floor(((abilityScores.dexterity ?? 10) - 10) / 2);

  return (
    <Container className="grid gap-6 py-6">
      {/* Character Header */}
      <div
        className={cn(
          "flex items-center justify-between gap-4",
          isOptimistic && "opacity-80",
        )}
      >
        <div>
          <h1 className="text-2xl font-semibold">{actorName}</h1>
          <p className="text-sm text-muted-foreground">
            ID: {actorId.substring(0, 8)}...
          </p>
        </div>

        {isOptimistic && (
          <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
            Updating...
          </span>
        )}
      </div>

      {/* Character Sheet Tabs */}
      <Tabs
        defaultValue="abilities"
        items={[
          {
            value: "abilities",
            label: "Abilities",
            content: (
              <div className="grid gap-4">
                <AbilityScores
                  data={abilityScores}
                  editable={editable}
                  onUpdate={handleUpdateAbility}
                />

                {/* Quick Stats */}
                <Card surface="parchment" className="p-6">
                  <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
                    <div className="grid gap-1">
                      <span className="text-xs text-muted-foreground">
                        Hit Points
                      </span>
                      <span className="text-xl font-semibold">
                        {readNumber(resourceData, "hp") ?? 0}
                      </span>
                    </div>
                    <div className="grid gap-1">
                      <span className="text-xs text-muted-foreground">
                        Armor Class
                      </span>
                      <span className="text-xl font-semibold">
                        {readNumber(resourceData, "ac") ?? 10}
                      </span>
                    </div>
                    <div className="grid gap-1">
                      <span className="text-xs text-muted-foreground">
                        Speed
                      </span>
                      <span className="text-xl font-semibold">
                        {readNumber(resourceData, "speed") ?? 30} ft
                      </span>
                    </div>
                    <div className="grid gap-1">
                      <span className="text-xs text-muted-foreground">
                        Initiative
                      </span>
                      <span className="text-xl font-semibold">
                        {dexModifier >= 0 ? "+" : ""}
                        {dexModifier}
                      </span>
                    </div>
                  </div>
                </Card>
              </div>
            ),
          },
          {
            value: "skills",
            label: "Skills",
            content: (
              <SkillsList
                abilityData={abilityScores}
                proficiencyData={proficiencies}
                editable={editable}
                onToggleProficiency={handleToggleProficiency}
              />
            ),
          },
          {
            value: "traits",
            label: "Traits",
            content: (
              <Card surface="parchment" className="grid gap-4 p-6">
                <h3 className="text-lg font-semibold">Character Traits</h3>

                <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">Class</span>
                    <span className="font-medium">
                      {readString(traitData, "class") ?? "—"}
                    </span>
                  </div>
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">Level</span>
                    <span className="font-medium">
                      {readNumber(traitData, "level") ?? 1}
                    </span>
                  </div>
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">Race</span>
                    <span className="font-medium">
                      {readString(traitData, "race") ?? "—"}
                    </span>
                  </div>
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">
                      Background
                    </span>
                    <span className="font-medium">
                      {readString(traitData, "background") ?? "—"}
                    </span>
                  </div>
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">
                      Alignment
                    </span>
                    <span className="font-medium">
                      {readString(traitData, "alignment") ?? "—"}
                    </span>
                  </div>
                  <div className="grid gap-1">
                    <span className="text-xs text-muted-foreground">
                      Experience
                    </span>
                    <span className="font-medium">
                      {readNumber(traitData, "experience") ?? 0}
                    </span>
                  </div>
                </div>
              </Card>
            ),
          },
          {
            value: "spells",
            label: "Spells",
            content: (
              <Card surface="parchment" className="grid gap-4 p-6">
                <div>
                  <h3 className="text-lg font-semibold">Spellbook</h3>
                  <p className="text-sm text-muted-foreground">
                    Spellcasting ability:{" "}
                    {spellData.spellcasting_ability ?? "—"}
                  </p>
                </div>

                {spellData.spell_slots ? (
                  <div className="grid gap-2">
                    <div className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      Spell Slots
                    </div>
                    <div className="flex flex-wrap gap-3">
                      {Object.entries(spellData.spell_slots).map(
                        ([level, slots]) => (
                          <div
                            key={level}
                            className="flex items-center gap-1 rounded-md bg-muted px-3 py-1 text-sm"
                          >
                            <span>{level}:</span>
                            <span className="font-medium">{slots}</span>
                          </div>
                        ),
                      )}
                    </div>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    No spell slots recorded
                  </p>
                )}

                {spellData.known_spells && spellData.known_spells.length > 0 ? (
                  <div className="grid gap-2">
                    <div className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      Known Spells
                    </div>
                    <ul className="grid list-disc gap-1 pl-4 text-sm">
                      {spellData.known_spells.map((spell, idx) => (
                        <li key={idx}>{spell}</li>
                      ))}
                    </ul>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    No spells known
                  </p>
                )}
              </Card>
            ),
          },
        ]}
      />
    </Container>
  );
}
