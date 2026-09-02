import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import { getWorldItems, type WorldItemWithPrice } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { formatItemPrice } from "@/pages/world/compendium/itemPrice";

export interface ItemCompendiumTabProps {
  worldId: string;
  onSelect: (itemId: string) => void;
  selectedItemId: string | null;
  /** DM/GM-only — gates the "New item" link (FR-002, spec 010 precedent). */
  isGm: boolean;
  /** Bump to force a re-fetch (e.g. after creating a new Item). */
  refreshKey?: number;
  /** Called whenever the catalog is (re)fetched, mirrors NpcCompendiumTab's onRosterLoaded. */
  onCatalogLoaded?: (items: WorldItemWithPrice[]) => void;
}

/**
 * Spec 013 (T024/T026): the Compendium's Items tab, replacing spec 011's
 * placeholder. Mirrors NpcCompendiumTab's search/table/add-control shape,
 * adapted to `api/items.ts` (server-side `search` param instead of
 * client-side FlexSearch, since there's no existing item search index).
 *
 * Spec 031 (T068/T071, FR-035/FR-037): the inline "Add Item" form is gone —
 * creation moved to `ItemEditorPage` behind an explicit save, and the
 * "did you mean?" nudge went with it, since duplicate-authoring is a creation
 * problem. What the list gained instead is the Game Master's price note,
 * presentational only (ADR-058).
 */
export function ItemCompendiumTab({
  worldId,
  onSelect,
  selectedItemId,
  isGm,
  refreshKey,
  onCatalogLoaded,
}: ItemCompendiumTabProps) {
  const [items, setItems] = useState<WorldItemWithPrice[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${query}|${refreshKey ?? ""}`, () => {
    setItems(null);
    setError(null);
  });

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
  }, [worldId, query, refreshKey]);

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
                <th className="p-2 font-semibold">Price</th>
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
                  <td
                    className="p-2 text-muted-foreground"
                    data-testid={`item-catalog-price-${item.id}`}
                  >
                    {formatItemPrice(item.price) ?? (
                      <span className="italic">—</span>
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
        <Button
          asChild
          size="sm"
          icon="inventory"
          className="justify-self-start"
          data-testid="new-item-link"
        >
          <Link to={`/world/${worldId}/compendium/item/new`}>New item</Link>
        </Button>
      ) : null}
    </div>
  );
}
