import { useEffect, useMemo, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import {
  getWorldActorImages,
  getWorldActors,
  type ActorImageRecord,
} from "@/api/actors";
import { indexActors, searchActorIds } from "@/search/actorSearch";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { portraitOf } from "@/pages/world/actor/actorImagery";
import type { WorldActorRecord } from "@/types/actor";

export interface NpcCompendiumTabProps {
  worldId: string;
  onSelect: (actorId: string) => void;
  selectedActorId: string | null;
  /** DM/GM-only — gates the "New NPC" link (FR-006, spec 010 precedent). */
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
 *
 * Spec 031 (T068, FR-035): the inline "Add NPC" form is gone. Creating an NPC
 * meant two boxes wedged under the table, with no room for a description worth
 * reading and nowhere at all for a portrait — so it moved to `NpcEditorPage`
 * behind an explicit save, and this tab became a list and a link. `refreshKey`
 * survives because the parent may still have reason to refetch; the tab no
 * longer has one of its own.
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
  // Spec 031 (FR-036): the roster's imagery, keyed by actor. Fetched
  // separately from the roster itself because it costs the server a query per
  // actor, and only the screens that show a face should pay for it.
  const [imagesByActor, setImagesByActor] = useState<
    Record<string, ActorImageRecord[]>
  >({});

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(
    `${worldId}|${refreshKey ?? ""}`,
    () => {
      setActors(null);
      setError(null);
    },
  );

  useEffect(() => {
    let active = true;

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

    getWorldActorImages(worldId)
      .then((byActor) => {
        if (active) {
          setImagesByActor(byActor);
        }
      })
      .catch(() => {
        // Imagery that fails to load leaves the roster readable — a missing
        // portrait is not a reason to hide the NPC it belongs to.
        if (active) {
          setImagesByActor({});
        }
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
                <th className="p-2 font-semibold">Portrait</th>
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
                  <td className="p-2">
                    {/* The list shows the portrait, never the token: a token
                        is drawn for map scale and reads as a smudge here. */}
                    {portraitOf(imagesByActor[npc.id]) ? (
                      <img
                        src={portraitOf(imagesByActor[npc.id])!.thumbnailUrl}
                        alt=""
                        className="h-10 w-8 rounded border border-border object-cover"
                        data-testid={`npc-catalog-portrait-${npc.id}`}
                      />
                    ) : (
                      <div
                        className="h-10 w-8 rounded border border-dashed border-border"
                        data-testid={`npc-catalog-portrait-empty-${npc.id}`}
                      />
                    )}
                  </td>
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
        <Button
          asChild
          size="sm"
          icon="skull"
          className="justify-self-start"
          data-testid="new-npc-link"
        >
          <Link to={`/world/${worldId}/compendium/npc/new`}>New NPC</Link>
        </Button>
      ) : null}
    </div>
  );
}
