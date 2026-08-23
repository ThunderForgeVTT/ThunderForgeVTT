import { useEffect, useState } from "react";
import { addItemToInventory, adjustInventoryQuantity, getActorInventory, removeInventoryEntry } from "@/api/inventory";
import { getWorldItems } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { InventoryEntryRecord } from "@/types/inventory";
import type { WorldItemRecord } from "@/types/item";

export interface ActorInventoryPanelProps {
  actorId: string;
  worldId: string;
  /** Editor/Owner on the ACTOR gates add/adjust/remove controls (FR-013,
   * spec Assumptions) — NOT the caller's permission on any given Item. */
  canManage: boolean;
}

/**
 * Spec 013 (T036, User Story 2): an Actor's inventory — Item + quantity
 * list with add/adjust/remove controls. Deleted-item rows render via
 * `itemName` (the server's `item_name_snapshot`) rather than vanishing
 * (Edge Cases).
 */
export function ActorInventoryPanel({ actorId, worldId, canManage }: ActorInventoryPanelProps) {
  const [entries, setEntries] = useState<InventoryEntryRecord[] | null>(null);
  const [catalog, setCatalog] = useState<WorldItemRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedItemId, setSelectedItemId] = useState("");
  const [addQuantity, setAddQuantity] = useState("1");
  const [isSaving, setIsSaving] = useState(false);
  const [pendingEntryId, setPendingEntryId] = useState<string | null>(null);

  const refresh = () => {
    getActorInventory(actorId)
      .then(setEntries)
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load inventory"));
  };

  useEffect(() => {
    let active = true;
    getActorInventory(actorId)
      .then((rows) => {
        if (active) {
          setEntries(rows);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err.message : "Failed to load inventory");
        }
      });
    return () => {
      active = false;
    };
  }, [actorId]);

  useEffect(() => {
    if (!canManage) {
      return;
    }
    let active = true;
    getWorldItems(worldId)
      .then((rows) => {
        if (active) {
          setCatalog(rows);
        }
      })
      .catch(() => {
        if (active) {
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId, canManage]);

  const handleAdd = async () => {
    const quantity = Number.parseInt(addQuantity, 10);
    if (!selectedItemId || !Number.isFinite(quantity) || quantity < 1) {
      return;
    }
    setIsSaving(true);
    setError(null);
    try {
      await addItemToInventory(actorId, selectedItemId, quantity);
      setSelectedItemId("");
      setAddQuantity("1");
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add item");
    } finally {
      setIsSaving(false);
    }
  };

  const handleAdjust = async (entry: InventoryEntryRecord, nextQuantity: number) => {
    if (nextQuantity < 0) {
      return;
    }
    setPendingEntryId(entry.id);
    setError(null);
    try {
      await adjustInventoryQuantity(entry.id, nextQuantity);
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to adjust quantity");
    } finally {
      setPendingEntryId(null);
    }
  };

  const handleRemove = async (entry: InventoryEntryRecord) => {
    setPendingEntryId(entry.id);
    setError(null);
    try {
      await removeInventoryEntry(entry.id);
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to remove item");
    } finally {
      setPendingEntryId(null);
    }
  };

  if (entries === null) {
    return <p className="text-sm text-muted-foreground">Loading inventory…</p>;
  }

  return (
    <Card className="grid gap-4 p-6" data-testid="actor-inventory-panel">
      <h3 className="text-lg font-semibold">Inventory</h3>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">No items yet.</p>
      ) : (
        <div className="grid gap-2">
          {entries.map((entry) => (
            <div
              key={entry.id}
              data-testid={`inventory-entry-${entry.id}`}
              className="flex items-center justify-between gap-3 rounded-lg border border-border p-3"
            >
              <div>
                <strong className="text-sm">
                  {entry.itemName}
                  {entry.itemId === null ? (
                    <span className="ml-2 text-xs text-muted-foreground italic">(deleted item)</span>
                  ) : null}
                </strong>
                <p className="text-xs text-muted-foreground">Quantity: {entry.quantity}</p>
              </div>
              {canManage ? (
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={pendingEntryId === entry.id}
                    onClick={() => void handleAdjust(entry, entry.quantity - 1)}
                    aria-label={`Decrease quantity of ${entry.itemName}`}
                  >
                    −
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={pendingEntryId === entry.id}
                    onClick={() => void handleAdjust(entry, entry.quantity + 1)}
                    aria-label={`Increase quantity of ${entry.itemName}`}
                  >
                    +
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={pendingEntryId === entry.id}
                    onClick={() => void handleRemove(entry)}
                    data-testid={`inventory-remove-${entry.id}`}
                  >
                    Remove
                  </Button>
                </div>
              ) : null}
            </div>
          ))}
        </div>
      )}

      {canManage ? (
        <div className="grid gap-2 sm:grid-cols-[2fr_1fr_auto]">
          <select
            value={selectedItemId}
            onChange={(event) => setSelectedItemId(event.target.value)}
            disabled={isSaving || catalog === null}
            className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
            data-testid="inventory-add-item-select"
            aria-label="Item to add"
          >
            <option value="">{catalog === null ? "Loading items…" : "Select an item…"}</option>
            {(catalog ?? []).map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
          <Input
            type="number"
            min={1}
            value={addQuantity}
            onChange={(event) => setAddQuantity(event.target.value)}
            disabled={isSaving}
            data-testid="inventory-add-quantity-input"
            aria-label="Quantity to add"
          />
          <Button
            type="button"
            size="sm"
            onClick={() => void handleAdd()}
            disabled={isSaving || !selectedItemId}
            data-testid="inventory-add-button"
          >
            Add
          </Button>
        </div>
      ) : null}
    </Card>
  );
}
