import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import { createItem, getWorldItems, suggestItemName } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { WorldItemRecord } from "@/types/item";

export interface ItemCompendiumTabProps {
  worldId: string;
  onSelect: (itemId: string) => void;
  selectedItemId: string | null;
  /** DM/GM-only — gates the "Add Item" control (FR-002, spec 010 precedent). */
  isGm: boolean;
  /** Bump to force a re-fetch (e.g. after creating a new Item). */
  refreshKey?: number;
  /** Called whenever the catalog is (re)fetched, mirrors NpcCompendiumTab's onRosterLoaded. */
  onCatalogLoaded?: (items: WorldItemRecord[]) => void;
}

/**
 * Spec 013 (T024/T026): the Compendium's Items tab, replacing spec 011's
 * placeholder. Mirrors NpcCompendiumTab's search/table/add-control shape,
 * adapted to `api/items.ts` (server-side `search` param instead of
 * client-side FlexSearch, since there's no existing item search index).
 */
export function ItemCompendiumTab({
  worldId,
  onSelect,
  selectedItemId,
  isGm,
  refreshKey,
  onCatalogLoaded,
}: ItemCompendiumTabProps) {
  const [items, setItems] = useState<WorldItemRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [internalRefreshTick, setInternalRefreshTick] = useState(0);
  const [newItemName, setNewItemName] = useState("");
  const [newItemDescription, setNewItemDescription] = useState("");
  const [isCreatingItem, setIsCreatingItem] = useState(false);
  // Stored against the name it was looked up for. Whether a suggestion is
  // showing right now is then a render-time question ("is this still the
  // name we asked about, and is it still long enough to ask?"), so the
  // effect below never has to null it back out.
  const [loadedSuggestion, setLoadedSuggestion] = useState<{
    worldId: string;
    query: string;
    match: WorldItemRecord | null;
  } | null>(null);

  const handleAddItem = async () => {
    const name = newItemName.trim();
    if (!name) {
      return;
    }
    setIsCreatingItem(true);
    try {
      await createItem({
        worldId,
        name,
        description: newItemDescription.trim() || undefined,
      });
      setNewItemName("");
      setNewItemDescription("");
      setLoadedSuggestion(null);
      setInternalRefreshTick((current) => current + 1);
    } finally {
      setIsCreatingItem(false);
    }
  };

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(
    `${worldId}|${query}|${refreshKey ?? ""}|${internalRefreshTick}`,
    () => {
      setItems(null);
      setError(null);
    },
  );

  useEffect(() => {
    let active = true;

    getWorldItems(worldId, query || undefined)
      .then((result) => {
        if (!active) {
          return;
        }
        setItems(result);
        onCatalogLoaded?.(result);
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
  }, [worldId, query, refreshKey, internalRefreshTick]);

  // FR-020: non-blocking "did you mean?" nudge as the DM types a new
  // item's name — debounced, never blocks handleAddItem.
  const suggestionQuery = newItemName.trim();
  const suggestion =
    isGm &&
    suggestionQuery.length >= 2 &&
    loadedSuggestion?.worldId === worldId &&
    loadedSuggestion.query === suggestionQuery
      ? loadedSuggestion.match
      : null;

  useEffect(() => {
    const name = newItemName.trim();
    if (!isGm || name.length < 2) {
      return;
    }
    let active = true;
    const timer = setTimeout(() => {
      suggestItemName(worldId, name)
        .then((matches) => {
          if (active) {
            setLoadedSuggestion({
              worldId,
              query: name,
              match: matches[0] ?? null,
            });
          }
        })
        .catch(() => {
          if (active) {
            setLoadedSuggestion({ worldId, query: name, match: null });
          }
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [worldId, newItemName, isGm]);

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load items: {error.message}
      </p>
    );
  }

  if (items === null) {
    return <p className="text-sm text-muted-foreground">Loading items…</p>;
  }

  return (
    <div className="grid gap-3">
      <Input
        type="search"
        placeholder="Search items by name or description…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="item-catalog-search-input"
        aria-label="Search items"
      />

      {items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {query ? `No items match "${query}".` : "No Items yet."}
        </p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="item-catalog-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Name</th>
                <th className="p-2 font-semibold">Description</th>
                <th className="p-2 font-semibold">Actions</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <tr
                  key={item.id}
                  className={cn(
                    "cursor-pointer border-b border-border last:border-0 hover:bg-muted/40",
                    selectedItemId === item.id && "bg-muted",
                  )}
                  data-testid={`item-catalog-row-${item.id}`}
                  onClick={() => onSelect(item.id)}
                  aria-selected={selectedItemId === item.id}
                >
                  <td className="p-2 font-medium">{item.name}</td>
                  <td className="max-w-xs truncate p-2 text-muted-foreground">
                    {item.description || (
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
                        data-testid={`item-catalog-view-${item.id}`}
                      >
                        <Link to={`/world/${worldId}/item/${item.id}/view`}>
                          View
                        </Link>
                      </Button>
                      {item.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          asChild
                          variant="ghost"
                          size="sm"
                          data-testid={`item-catalog-edit-${item.id}`}
                        >
                          <Link to={`/world/${worldId}/item/${item.id}/edit`}>
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
        <div className="grid gap-2">
          <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
            <Input
              value={newItemName}
              onChange={(event) => setNewItemName(event.target.value)}
              placeholder="New item name"
              disabled={isCreatingItem}
              data-testid="new-item-name-input"
            />
            <Input
              value={newItemDescription}
              onChange={(event) => setNewItemDescription(event.target.value)}
              placeholder="Description (optional)"
              disabled={isCreatingItem}
              data-testid="new-item-description-input"
            />
            <Button
              type="button"
              size="sm"
              icon="inventory"
              onClick={() => void handleAddItem()}
              disabled={isCreatingItem || !newItemName.trim()}
              data-testid="add-item-button"
            >
              Add Item
            </Button>
          </div>
          {suggestion ? (
            <p
              className="text-xs text-muted-foreground"
              data-testid="item-name-suggestion"
            >
              Did you mean{" "}
              <span className="font-medium">{suggestion.name}</span>? Names can
              be reused if that's intentional.
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
