import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { getWorldActors } from "@/api/actors";
import { indexActors, searchActorIds } from "@/search/actorSearch";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import type { WorldActorRecord } from "@/types/actor";

export interface NpcCatalogProps {
  worldId: string;
  /** Bump this to force a re-fetch (e.g. after creating a new NPC). */
  refreshKey?: number;
}

/**
 * Spec 010 follow-up: the staging page's NPC catalog — a searchable mini
 * table (name, description, view/edit) over the world's NPC roster.
 * Search is instant-as-you-type, powered by a client-side FlexSearch
 * index (`@/search/actorSearch`) built from the already-fetched roster;
 * there's no per-keystroke network round trip.
 */
export function NpcCatalog({ worldId, refreshKey }: NpcCatalogProps) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [matchedIds, setMatchedIds] = useState<string[] | null>(null);

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
  }, [worldId, refreshKey]);

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
                  className="border-b border-border last:border-0"
                  data-testid={`npc-catalog-row-${npc.id}`}
                >
                  <td className="p-2 font-medium">{npc.label}</td>
                  <td className="max-w-xs truncate p-2 text-muted-foreground">
                    {npc.description || (
                      <span className="italic">No description</span>
                    )}
                  </td>
                  <td className="p-2">
                    <div className="flex gap-2">
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
    </div>
  );
}
