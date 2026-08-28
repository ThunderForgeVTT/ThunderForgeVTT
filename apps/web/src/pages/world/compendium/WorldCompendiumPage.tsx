import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { getGameSystemManifest } from "@/api/gameSystems";
import { getLoreEntry } from "@/api/lore";
import { COMPENDIUM_OVERVIEW_SLUG } from "@/api/compendiumOverview";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { ActorPreviewPanel } from "@/pages/world/compendium/ActorPreviewPanel";
import { AbilityCompendiumTab } from "@/pages/world/compendium/AbilityCompendiumTab";
import { AbilityPreviewPanel } from "@/pages/world/compendium/AbilityPreviewPanel";
import { ItemCompendiumTab } from "@/pages/world/compendium/ItemCompendiumTab";
import { ItemPreviewPanel } from "@/pages/world/compendium/ItemPreviewPanel";
import { LoreCompendiumTab } from "@/pages/world/compendium/LoreCompendiumTab";
import { NpcCompendiumTab } from "@/pages/world/compendium/NpcCompendiumTab";
import { LoreMarkdownRenderer } from "@/pages/world/lore/LoreMarkdownRenderer";
import { useWorldRole } from "@/hooks/useWorldRole";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldAbilityRecord } from "@/types/ability";
import type { WorldItemRecord } from "@/types/item";
import type { AbilityFacetsLookup } from "@/utils/abilityFacets";
import type { WorldRecord } from "@/types/world";

export interface WorldCompendiumPageProps {
  worldId: string;
  world: WorldRecord | null;
}

/**
 * Spec 011: `/world/:id/compendium` — the DM/player-shared portal for
 * curating world artifacts. All four tabs are real as of spec 025 —
 * the Abilities placeholder was the last one (SC-001). Reached via a link
 * from Session Setup. Renders inside the
 * normal app chrome, never the full-screen canvas.
 *
 * The tabbed shell (`CompendiumTabDef[]`, data-model.md) is deliberately
 * a plain array so a future tab is a one-line addition, not a
 * restructuring (research.md §4).
 */
const COMPENDIUM_TAB_VALUES = ["npcs", "lore", "items", "abilities"];

export function WorldCompendiumPage({
  worldId,
  world,
}: WorldCompendiumPageProps) {
  // Spec 021 (world sidebar nav): the tab is URL-driven so sidebar links
  // (e.g. `/compendium?tab=lore`) land directly on that tab instead of
  // always opening to NPCs.
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedTab = searchParams.get("tab");
  const activeTab = COMPENDIUM_TAB_VALUES.includes(requestedTab ?? "")
    ? requestedTab!
    : "npcs";
  const [selectedActorId, setSelectedActorId] = useState<string | null>(null);
  const [roster, setRoster] = useState<WorldActorRecord[]>([]);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [itemCatalog, setItemCatalog] = useState<WorldItemRecord[]>([]);
  const [selectedAbilityId, setSelectedAbilityId] = useState<string | null>(
    null,
  );
  const [abilityCatalog, setAbilityCatalog] = useState<WorldAbilityRecord[]>(
    [],
  );
  // Spec 025 (T028, FR-010/FR-012): the active system's optional ability
  // presentation facets. `undefined` (no system, no facets block, or a failed
  // manifest fetch) means every classification renders its built-in default
  // label — the resolver is total, so this never needs a loading state.
  const [abilityFacets, setAbilityFacets] = useState<
    AbilityFacetsLookup | undefined
  >(undefined);
  const { isGm } = useWorldRole(worldId, world);

  // Spec 021: the header blurb is GM-authored Markdown (a reserved lore
  // entry, edited from System settings), not a hardcoded sentence — `null`
  // (not yet created for this world) just means the header shows no blurb.
  const [overviewHtml, setOverviewHtml] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getLoreEntry(worldId, COMPENDIUM_OVERVIEW_SLUG)
      .then((entry) => {
        if (active) {
          setOverviewHtml(
            entry && !entry.moderated ? entry.renderedHtml : null,
          );
        }
      })
      .catch(() => {
        if (active) {
          setOverviewHtml(null);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  useEffect(() => {
    const gameSystemId = world?.gameSystemId;
    if (!gameSystemId) {
      setAbilityFacets(undefined);
      return;
    }
    let active = true;
    getGameSystemManifest(gameSystemId)
      .then((manifest) => {
        if (active) {
          setAbilityFacets(
            manifest.abilityFacets as AbilityFacetsLookup | undefined,
          );
        }
      })
      .catch(() => {
        // A manifest fetch failure degrades to default labels rather than
        // breaking the tab.
        if (active) {
          setAbilityFacets(undefined);
        }
      });
    return () => {
      active = false;
    };
  }, [world?.gameSystemId]);

  const selectedActor = useMemo(
    () => roster.find((actor) => actor.id === selectedActorId) ?? null,
    [roster, selectedActorId],
  );

  const selectedItem = useMemo(
    () => itemCatalog.find((item) => item.id === selectedItemId) ?? null,
    [itemCatalog, selectedItemId],
  );

  const selectedAbility = useMemo(
    () =>
      abilityCatalog.find((ability) => ability.id === selectedAbilityId) ??
      null,
    [abilityCatalog, selectedAbilityId],
  );

  return (
    <main className="grid w-full gap-4" data-testid="world-compendium-page">
      <header className="grid gap-1 rounded-lg border border-border bg-card px-4 py-3">
        <h1 className="text-xl font-semibold">
          {world?.name ?? "World"} artifacts
        </h1>
        {overviewHtml ? (
          <LoreMarkdownRenderer
            html={overviewHtml}
            className="text-sm text-muted-foreground"
          />
        ) : null}
      </header>

      <Tabs
        defaultValue="npcs"
        value={activeTab}
        onValueChange={(tab) =>
          setSearchParams(
            (current) => {
              const next = new URLSearchParams(current);
              next.set("tab", tab);
              return next;
            },
            { replace: true },
          )
        }
        items={[
          {
            value: "npcs",
            label: "NPCs",
            icon: "skull",
            content: (
              <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
                <NpcCompendiumTab
                  worldId={worldId}
                  onSelect={setSelectedActorId}
                  selectedActorId={selectedActorId}
                  isGm={isGm}
                  onRosterLoaded={setRoster}
                />
                <ActorPreviewPanel
                  worldId={worldId}
                  actor={selectedActor}
                  onClose={() => setSelectedActorId(null)}
                />
              </div>
            ),
          },
          {
            value: "lore",
            label: "Lore",
            icon: "quill",
            content: <LoreCompendiumTab worldId={worldId} isGm={isGm} />,
          },
          {
            value: "items",
            label: "Items",
            icon: "inventory",
            content: (
              <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
                <ItemCompendiumTab
                  worldId={worldId}
                  onSelect={setSelectedItemId}
                  selectedItemId={selectedItemId}
                  isGm={isGm}
                  onCatalogLoaded={setItemCatalog}
                />
                <ItemPreviewPanel
                  worldId={worldId}
                  item={selectedItem}
                  onClose={() => setSelectedItemId(null)}
                />
              </div>
            ),
          },
          {
            value: "abilities",
            label: "Abilities",
            icon: "spells",
            content: (
              <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
                <AbilityCompendiumTab
                  worldId={worldId}
                  onSelect={setSelectedAbilityId}
                  selectedAbilityId={selectedAbilityId}
                  isGm={isGm}
                  facets={abilityFacets}
                  onCatalogLoaded={setAbilityCatalog}
                />
                <AbilityPreviewPanel
                  worldId={worldId}
                  ability={selectedAbility}
                  facets={abilityFacets}
                  onClose={() => setSelectedAbilityId(null)}
                />
              </div>
            ),
          },
        ]}
      />
    </main>
  );
}
