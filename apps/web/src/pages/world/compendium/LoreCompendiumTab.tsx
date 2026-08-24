import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { createLoreEntry, getWorldLoreEntries } from "@/api/lore";
import { COMPENDIUM_OVERVIEW_SLUG } from "@/api/compendiumOverview";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { LoreEntryRecord } from "@/types/lore";

export interface LoreCompendiumTabProps {
  worldId: string;
  /** DM/GM-only — gates the "New entry" control (FR-002). */
  isGm: boolean;
}

/**
 * Spec 012 (T025, FR-001): the Compendium's Lore tab — a world-scoped
 * index of lore entries, mirroring `NpcCompendiumTab.tsx`'s structure.
 * Unlike the NPC tab, selecting a row navigates directly to the entry's
 * `/world/:id/lore/:slug/view` route rather than opening a side preview
 * panel, since a lore entry's rendered Markdown body is the primary
 * content (no separate detail panel makes sense here).
 */
export function LoreCompendiumTab({ worldId, isGm }: LoreCompendiumTabProps) {
  const [entries, setEntries] = useState<LoreEntryRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [refreshTick, setRefreshTick] = useState(0);
  const [newTitle, setNewTitle] = useState("");
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    let active = true;
    setEntries(null);
    setError(null);

    getWorldLoreEntries(worldId)
      .then((result) => {
        if (active) {
          setEntries(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, refreshTick]);

  const handleCreate = async () => {
    const title = newTitle.trim();
    if (!title) {
      return;
    }
    setIsCreating(true);
    try {
      await createLoreEntry({ worldId, title });
      setNewTitle("");
      setRefreshTick((current) => current + 1);
    } finally {
      setIsCreating(false);
    }
  };

  // Spec 021: the reserved Compendium-overview entry is authored from
  // System settings, not this catalog — exclude it here so it doesn't show
  // up as an ordinary, editable/deletable-looking lore row.
  const catalogEntries = useMemo(
    () => (entries ?? []).filter((entry) => entry.slug !== COMPENDIUM_OVERVIEW_SLUG),
    [entries],
  );

  const visibleEntries = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return catalogEntries;
    }
    return catalogEntries.filter((entry) => entry.title.toLowerCase().includes(q));
  }, [catalogEntries, query]);

  if (error) {
    return <p className="text-sm text-destructive">Failed to load lore: {error.message}</p>;
  }

  if (entries === null) {
    return <p className="text-sm text-muted-foreground">Loading lore…</p>;
  }

  return (
    <div className="grid gap-3">
      <Input
        type="search"
        placeholder="Search lore entries by title…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="lore-catalog-search-input"
        aria-label="Search lore entries"
      />

      {catalogEntries.length === 0 ? (
        <p className="text-sm text-muted-foreground">No lore entries yet.</p>
      ) : visibleEntries.length === 0 ? (
        <p className="text-sm text-muted-foreground">No lore entries match "{query}".</p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="lore-catalog-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Title</th>
                <th className="p-2 font-semibold">Actions</th>
              </tr>
            </thead>
            <tbody>
              {visibleEntries.map((entry) => (
                <tr
                  key={entry.id}
                  className={cn(
                    "border-b border-border last:border-0 hover:bg-muted/40",
                  )}
                  data-testid={`lore-catalog-row-${entry.id}`}
                >
                  <td className="p-2 font-medium">
                    <Link
                      to={`/world/${worldId}/lore/${entry.slug}/view`}
                      className="hover:underline"
                    >
                      {entry.title}
                    </Link>
                  </td>
                  <td className="p-2">
                    <div className="flex gap-2">
                      <Button
                        asChild
                        variant="ghost"
                        size="sm"
                        data-testid={`lore-catalog-view-${entry.id}`}
                      >
                        <Link to={`/world/${worldId}/lore/${entry.slug}/view`}>View</Link>
                      </Button>
                      {entry.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          asChild
                          variant="ghost"
                          size="sm"
                          data-testid={`lore-catalog-edit-${entry.id}`}
                        >
                          <Link to={`/world/${worldId}/lore/${entry.slug}/edit`}>Edit</Link>
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
        <div className="grid gap-2 sm:grid-cols-[1fr_auto]">
          <Input
            value={newTitle}
            onChange={(event) => setNewTitle(event.target.value)}
            placeholder="New lore entry title"
            disabled={isCreating}
            data-testid="new-lore-entry-title-input"
          />
          <Button
            type="button"
            size="sm"
            icon="quill"
            onClick={() => void handleCreate()}
            disabled={isCreating || !newTitle.trim()}
            data-testid="add-lore-entry-button"
          >
            New entry
          </Button>
        </div>
      ) : null}
    </div>
  );
}
