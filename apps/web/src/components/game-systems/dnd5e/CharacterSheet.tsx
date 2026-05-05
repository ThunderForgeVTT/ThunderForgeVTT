/**
 * CharacterSheet.tsx
 * D&D 5e Character Sheet Component
 *
 * Phase 4.8.1: System-Aware React Components (Phase E.1)
 *
 * Main component that displays the full D&D 5e character sheet by composing:
 * - AbilityScores: Core ability scores with modifiers
 * - SkillsList: All 18 skills with proficiency and passive checks
 * - Spellbook: Known spells and spell slots (future)
 * - Resources: HP, hit dice, spell slots, etc. (future)
 *
 * Data is fetched from RxDB world_actor_system_data collection and updated
 * via GraphQL mutations. Manifest-aware rendering allows other game systems
 * (Pathfinder 2e, CoC 7e) to define their own CharacterSheet components.
 */

import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { Container } from "@/components/ui/container/Container";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { Card } from "@/components/ui/card/Card";
import { cn } from "@/utils/cn";
import { AbilityScores } from "./AbilityScores";
import { SkillsList } from "./SkillsList";
import styles from "./CharacterSheet.module.scss";

export interface CharacterSheetProps {
  actorId: string;
  actorName: string;
  actorData?: {
    ability_data?: Record<string, any>;
    resource_data?: Record<string, any>;
    proficiency_data?: Record<string, any>;
    trait_data?: Record<string, any>;
    spell_data?: Record<string, any>;
  };
  editable?: boolean;
  onUpdate?: (dataType: string, data: Record<string, any>) => void;
  onError?: (error: Error) => void;
}

/**
 * Main D&D 5e Character Sheet Component
 *
 * This is a manifest-aware component that could be swapped out for
 * Pathfinder 2e, CoC 7e, etc. by the game system loader.
 *
 * Usage:
 * ```tsx
 * <CharacterSheet
 *   actorId="actor-123"
 *   actorName="Aragorn"
 *   actorData={{
 *     ability_data: { strength: 15, dexterity: 14, ... },
 *     proficiency_data: { acrobatics: true, ... },
 *   }}
 *   editable={true}
 *   onUpdate={(dataType, data) => graphql.mutate(...)}
 * />
 * ```
 */
export function CharacterSheet({
  actorId,
  actorName,
  actorData = {},
  editable = false,
  onUpdate,
  onError,
}: CharacterSheetProps): ReactNode {
  const [activeTab, setActiveTab] = useState("abilities");
  const [isOptimistic, setIsOptimistic] = useState(false);

  const abilityData = actorData.ability_data ?? {};
  const proficiencyData = actorData.proficiency_data ?? {};
  const resourceData = actorData.resource_data ?? {};
  const traitData = actorData.trait_data ?? {};
  const spellData = actorData.spell_data ?? {};

  const handleUpdateAbility = async (abilityId: string, score: number) => {
    try {
      setIsOptimistic(true);
      const updated = { ...abilityData, [abilityId]: score };
      onUpdate?.("ability_data", updated);
    } catch (error) {
      onError?.(error instanceof Error ? error : new Error(String(error)));
      setIsOptimistic(false);
    }
  };

  const handleToggleProficiency = async (skillId: string, proficient: boolean) => {
    try {
      setIsOptimistic(true);
      const updated = { ...proficiencyData, [skillId]: proficient };
      onUpdate?.("proficiency_data", updated);
    } catch (error) {
      onError?.(error instanceof Error ? error : new Error(String(error)));
      setIsOptimistic(false);
    }
  };

  return (
    <Container className={styles.container}>
      {/* Character Header */}
      <div className={cn(styles.header, { [styles.optimistic]: isOptimistic })}>
        <div className={styles.titleSection}>
          <h1 className={styles.characterName}>{actorName}</h1>
          <p className={styles.characterId}>ID: {actorId.substring(0, 8)}...</p>
        </div>

        {isOptimistic && (
          <div className={styles.optimisticBadge}>
            Updating...
          </div>
        )}
      </div>

      {/* Character Sheet Tabs */}
      <Tabs
        value={activeTab}
        onValueChange={setActiveTab}
        className={styles.tabsContainer}
      >
        <div className={styles.tabsList}>
          <button
            onClick={() => setActiveTab("abilities")}
            className={cn(styles.tab, { [styles.active]: activeTab === "abilities" })}
          >
            Abilities
          </button>
          <button
            onClick={() => setActiveTab("skills")}
            className={cn(styles.tab, { [styles.active]: activeTab === "skills" })}
          >
            Skills
          </button>
          <button
            onClick={() => setActiveTab("traits")}
            className={cn(styles.tab, { [styles.active]: activeTab === "traits" })}
          >
            Traits
          </button>
          <button
            onClick={() => setActiveTab("spells")}
            className={cn(styles.tab, { [styles.active]: activeTab === "spells" })}
          >
            Spells
          </button>
        </div>

        {/* Abilities Tab */}
        {activeTab === "abilities" && (
          <div className={styles.tabContent}>
            <AbilityScores
              data={abilityData}
              editable={editable}
              onUpdate={handleUpdateAbility}
            />

            {/* Quick Stats */}
            <Card surface="parchment" className={styles.quickStats}>
              <div className={styles.statsGrid}>
                <div className={styles.stat}>
                  <span className={styles.statLabel}>Hit Points</span>
                  <span className={styles.statValue}>{resourceData.hp ?? 0}</span>
                </div>
                <div className={styles.stat}>
                  <span className={styles.statLabel}>Armor Class</span>
                  <span className={styles.statValue}>{resourceData.ac ?? 10}</span>
                </div>
                <div className={styles.stat}>
                  <span className={styles.statLabel}>Speed</span>
                  <span className={styles.statValue}>{resourceData.speed ?? 30} ft</span>
                </div>
                <div className={styles.stat}>
                  <span className={styles.statLabel}>Initiative</span>
                  <span className={styles.statValue}>
                    {Math.floor(((abilityData.dexterity ?? 10) - 10) / 2) >= 0 ? "+" : ""}
                    {Math.floor(((abilityData.dexterity ?? 10) - 10) / 2)}
                  </span>
                </div>
              </div>
            </Card>
          </div>
        )}

        {/* Skills Tab */}
        {activeTab === "skills" && (
          <div className={styles.tabContent}>
            <SkillsList
              abilityData={abilityData}
              proficiencyData={proficiencyData}
              editable={editable}
              onToggleProficiency={handleToggleProficiency}
            />
          </div>
        )}

        {/* Traits Tab */}
        {activeTab === "traits" && (
          <div className={styles.tabContent}>
            <Card surface="parchment" className={styles.traitsCard}>
              <div className={styles.traitsHeader}>
                <h3>Character Traits</h3>
              </div>

              <div className={styles.traitsList}>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Class</span>
                  <span className={styles.traitValue}>{traitData.class ?? "—"}</span>
                </div>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Level</span>
                  <span className={styles.traitValue}>{traitData.level ?? 1}</span>
                </div>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Race</span>
                  <span className={styles.traitValue}>{traitData.race ?? "—"}</span>
                </div>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Background</span>
                  <span className={styles.traitValue}>{traitData.background ?? "—"}</span>
                </div>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Alignment</span>
                  <span className={styles.traitValue}>{traitData.alignment ?? "—"}</span>
                </div>
                <div className={styles.traitItem}>
                  <span className={styles.traitLabel}>Experience</span>
                  <span className={styles.traitValue}>{traitData.experience ?? 0}</span>
                </div>
              </div>
            </Card>
          </div>
        )}

        {/* Spells Tab */}
        {activeTab === "spells" && (
          <div className={styles.tabContent}>
            <Card surface="parchment" className={styles.spellsCard}>
              <div className={styles.spellsHeader}>
                <h3>Spellbook</h3>
                <p className={styles.spellsSubtitle}>
                  Spellcasting ability: {spellData.spellcasting_ability ?? "—"}
                </p>
              </div>

              {spellData.spell_slots ? (
                <div className={styles.spellSlots}>
                  <div className={styles.spellSlotLabel}>Spell Slots</div>
                  <div className={styles.spellSlotsList}>
                    {Object.entries(spellData.spell_slots).map(([level, slots]: [string, any]) => (
                      <div key={level} className={styles.spellSlot}>
                        <span>{level}:</span>
                        <span>{slots}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <p className={styles.placeholder}>No spell slots recorded</p>
              )}

              {spellData.known_spells && spellData.known_spells.length > 0 ? (
                <div className={styles.knownSpells}>
                  <div className={styles.knownSpellsLabel}>Known Spells</div>
                  <ul className={styles.knownSpellsList}>
                    {spellData.known_spells.map((spell: string, idx: number) => (
                      <li key={idx}>{spell}</li>
                    ))}
                  </ul>
                </div>
              ) : (
                <p className={styles.placeholder}>No spells known</p>
              )}
            </Card>
          </div>
        )}
      </Tabs>
    </Container>
  );
}
