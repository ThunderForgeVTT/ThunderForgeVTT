import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { createActor, getWorldActors } from "@/api/actors";
import { indexActors, searchActorIds } from "@/search/actorSearch";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { WorldActorRecord } from "@/types/actor";

export interface NpcCompendiumTabProps {
  worldId: string;
  onSelect: (actorId: string) => void;
  selectedActorId: string | null;
  /** DM/GM-only — gates the "Add NPC" control (FR-006, spec 010 precedent). */
  isGm: boolean;
  /** Bump to force a re-fetch (e.g. after creating a new NPC). */
  refreshKey?: number;
  /**
   * Called whenever the roster is (re)fetched, so the parent
   * `WorldCompendiumPage` can resolve `selectedActorId` into a full
   * `WorldActorRecord` for the preview panel without a second network
   * round trip (research.md §3). Not part of the external component
   * contract's minimal signature (contracts/compendium-npcs.md) — an
   * additive convenience prop.
   */
  onRosterLoaded?: (actors: WorldActorRecord[]) => void;
}

/**
 * Spec 011: the Compendium's NPCs tab. Adapted from spec 010's
 * `NpcCatalog` (`@/components/world/NpcCatalog/NpcCatalog`) — same
 * search/FlexSearch/table data flow, but rows are selectable (open the
 * right-side `ActorPreviewPanel`) instead of navigating away directly.
 * The inline View/Edit links remain as direct-navigation shortcuts,
 * independent of selection (contracts/compendium-npcs.md).
 */
export function NpcCompendiumTab({
  worldId,
  onSelect,
  selectedActorId,
  isGm,
  refreshKey,
  onRosterLoaded,
}: NpcCompendiumTabProps) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [matchedIds, setMatchedIds] = useState<string[] | null>(null);
  const [internalRefreshTick, setInternalRefreshTick] = useState(0);
  const [newNpcName, setNewNpcName] = useState("");
  const [newNpcDescription, setNewNpcDescription] = useState("");
  const [isCreatingNpc, setIsCreatingNpc] = useState(false);

  const handleAddNpc = async () => {
    const label = newNpcName.trim();
    if (!label) {
      return;
    }
    setIsCreatingNpc(true);
    try {
      await createActor({
        worldId,
        label,
        isNpc: true,
        description: newNpcDescription.trim() || undefined,
      });
      setNewNpcName("");
      setNewNpcDescription("");
      setInternalRefreshTick((current) => current + 1);
    } finally {
      setIsCreatingNpc(false);
    }
  };

  useEffect(() => {
    let active = true;
    setActors(null);
    setError(null);

    getWorldActors(worldId)
      .then((result) => {
        if (!active) {
          return;
        }
        setActors(result);
        onRosterLoaded?.(result);
        const npcs = result.filter((actor) => actor.isNpc);
        void indexActors(
          worldId,
          npcs.map((npc) => ({
            id: npc.id,
            label: npc.label,
            description: npc.description,
          })),
        );
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId, refreshKey, internalRefreshTick]);

  useEffect(() => {
    let active = true;
    searchActorIds(worldId, query).then((ids) => {
      if (active) {
        setMatchedIds(ids);
      }
    });
    return () => {
      active = false;
    };
  }, [worldId, query]);

  const npcs = useMemo(
    () => (actors ?? []).filter((actor) => actor.isNpc),
    [actors],
  );

  const visibleNpcs = useMemo(() => {
    if (matchedIds === null) {
      return npcs;
    }
    const rank = new Map(matchedIds.map((id, index) => [id, index]));
    return npcs
      .filter((npc) => rank.has(npc.id))
      .sort((a, b) => (rank.get(a.id) ?? 0) - (rank.get(b.id) ?? 0));
  }, [npcs, matchedIds]);

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load NPCs: {error.message}
      </p>
    );
  }

  if (actors === null) {
    return <p className="text-sm text-muted-foreground">Loading NPCs…</p>;
  }

  return (
    <div className="grid gap-3">
      <Input
        type="search"
        placeholder="Search NPCs by name or description…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="npc-catalog-search-input"
        aria-label="Search NPCs"
      />

      {npcs.length === 0 ? (
        <p className="text-sm text-muted-foreground">No NPCs yet.</p>
      ) : visibleNpcs.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No NPCs match "{query}".
        </p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="npc-catalog-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Name</th>
                <th className="p-2 font-semibold">Description</th>
                <th className="p-2 font-semibold">Actions</th>
              </tr>
            </thead>
            <tbody>
              {visibleNpcs.map((npc) => (
                <tr
                  key={npc.id}
                  className={cn(
                    "cursor-pointer border-b border-border last:border-0 hover:bg-muted/40",
                    selectedActorId === npc.id && "bg-muted",
                  )}
                  data-testid={`npc-catalog-row-${npc.id}`}
                  onClick={() => onSelect(npc.id)}
                  aria-selected={selectedActorId === npc.id}
                >
                  <td className="p-2 font-medium">{npc.label}</td>
                  <td className="max-w-xs truncate p-2 text-muted-foreground">
                    {npc.description || (
                      <span className="italic">No description</span>
                    )}
                  </td>
                  <td className="p-2">
                    <div
                      className="flex gap-2"
                      onClick={(event) => event.stopPropagation()}
                    >
                      <Button
                        asChild
                        variant="ghost"
                        size="sm"
                        data-testid={`npc-catalog-view-${npc.id}`}
                      >
                        <Link to={`/world/${worldId}/actor/${npc.id}/view`}>
                          View
                        </Link>
                      </Button>
                      {npc.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          asChild
                          variant="ghost"
                          size="sm"
                          data-testid={`npc-catalog-edit-${npc.id}`}
                        >
                          <Link to={`/world/${worldId}/actor/${npc.id}/edit`}>
                            Edit
                          </Link>
                        </Button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {isGm ? (
        <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
          <Input
            value={newNpcName}
            onChange={(event) => setNewNpcName(event.target.value)}
            placeholder="New NPC name"
            disabled={isCreatingNpc}
            data-testid="new-npc-name-input"
          />
          <Input
            value={newNpcDescription}
            onChange={(event) => setNewNpcDescription(event.target.value)}
            placeholder="Description (optional)"
            disabled={isCreatingNpc}
            data-testid="new-npc-description-input"
          />
          <Button
            type="button"
            size="sm"
            icon="skull"
            onClick={() => void handleAddNpc()}
            disabled={isCreatingNpc || !newNpcName.trim()}
            data-testid="add-npc-button"
          >
            Add NPC
          </Button>
        </div>
      ) : null}
    </div>
  );
}
