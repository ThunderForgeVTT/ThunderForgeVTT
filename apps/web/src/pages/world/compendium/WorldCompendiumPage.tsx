import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { ActorPreviewPanel } from "@/pages/world/compendium/ActorPreviewPanel";
import { ComingSoonTab } from "@/pages/world/compendium/ComingSoonTab";
import { ItemCompendiumTab } from "@/pages/world/compendium/ItemCompendiumTab";
import { ItemPreviewPanel } from "@/pages/world/compendium/ItemPreviewPanel";
import { LoreCompendiumTab } from "@/pages/world/compendium/LoreCompendiumTab";
import { NpcCompendiumTab } from "@/pages/world/compendium/NpcCompendiumTab";
import { useWorldRole } from "@/hooks/useWorldRole";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldItemRecord } from "@/types/item";
import type { WorldRecord } from "@/types/world";

export interface WorldCompendiumPageProps {
  worldId: string;
  world: WorldRecord | null;
}

/**
 * Spec 011: `/world/:id/compendium` — the DM/player-shared portal for
 * curating world artifacts (NPCs real in this pass; Items/Abilities
 * placeholder), reached via a link from Session Setup. Renders inside the
 * normal app chrome, never the full-screen canvas.
 *
 * The tabbed shell (`CompendiumTabDef[]`, data-model.md) is deliberately
 * a plain array so a future tab is a one-line addition, not a
 * restructuring (research.md §4).
 */
const COMPENDIUM_TAB_VALUES = ["npcs", "lore", "items", "abilities"];

export function WorldCompendiumPage({ worldId, world }: WorldCompendiumPageProps) {
  // Spec 021 (world sidebar nav): the tab is URL-driven so sidebar links
  // (e.g. `/compendium?tab=lore`) land directly on that tab instead of
  // always opening to NPCs.
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedTab = searchParams.get("tab");
  const activeTab = COMPENDIUM_TAB_VALUES.includes(requestedTab ?? "") ? requestedTab! : "npcs";
  const [selectedActorId, setSelectedActorId] = useState<string | null>(null);
  const [roster, setRoster] = useState<WorldActorRecord[]>([]);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [itemCatalog, setItemCatalog] = useState<WorldItemRecord[]>([]);
  const { isGm } = useWorldRole(worldId, world);

  const selectedActor = useMemo(
    () => roster.find((actor) => actor.id === selectedActorId) ?? null,
    [roster, selectedActorId],
  );

  const selectedItem = useMemo(
    () => itemCatalog.find((item) => item.id === selectedItemId) ?? null,
    [itemCatalog, selectedItemId],
  );

  return (
    <main className="grid min-h-screen gap-4 bg-background p-4" data-testid="world-compendium-page">
      <header className="rounded-xl border border-border bg-card p-5">
        <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          World compendium
        </p>
        <h1 className="text-2xl font-semibold">{world?.name ?? "World"} artifacts</h1>
        <p className="mt-2 max-w-3xl text-muted-foreground">
          Browse and curate this world's NPCs, lore, items, and abilities without entering play.
        </p>
      </header>

      <Tabs
        defaultValue="npcs"
        value={activeTab}
        onValueChange={(tab) => setSearchParams((current) => {
          const next = new URLSearchParams(current);
          next.set("tab", tab);
          return next;
        }, { replace: true })}
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
            content: <ComingSoonTab label="Abilities" />,
          },
        ]}
      />
    </main>
  );
}
