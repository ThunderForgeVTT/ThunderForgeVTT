import { useEffect, useState } from "react";
import { getActorInventory } from "@/api/inventory";
import { getWorldItems } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useGenieSession } from "@/hooks/useGenieSession";
import type { GenieShopListingRecord } from "@/api/genieSession";
import type { InventoryEntryRecord } from "@/types/inventory";
import type { WorldItemRecord } from "@/types/item";

const GENIE_SESSION_RESOURCE_TYPES = [
  { key: "insight", label: "Insight" },
  { key: "favor", label: "Favor" },
  { key: "essence", label: "Essence" },
];

export interface GenieShopPanelProps {
  worldId: string;
  npcActorId: string;
  currentUserId: string | undefined;
  /** GM-only: can create listings on this NPC (FR-004). */
  isGm: boolean;
}

/**
 * Spec 020 (User Story 2): an NPC actor's shop — GM authors listings
 * (resource-priced or item-for-item barter) against the NPC's own
 * inventory; any world member with a controlled PC can buy. Renders
 * nothing for a player when the NPC has zero listings (Scenario 6) —
 * only the GM sees the "create listing" affordance in that case.
 */
export function GenieShopPanel({
  worldId,
  npcActorId,
  currentUserId,
  isGm,
}: GenieShopPanelProps) {
  const { myActor, fetchShopListings, createShopListing, purchaseFromShop } =
    useGenieSession(worldId, currentUserId);
  const [listings, setListings] = useState<GenieShopListingRecord[] | null>(
    null,
  );
  const [npcInventory, setNpcInventory] = useState<
    InventoryEntryRecord[] | null
  >(null);
  const [worldItems, setWorldItems] = useState<WorldItemRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingListingId, setPendingListingId] = useState<string | null>(null);

  // New-listing form state.
  const [newItemId, setNewItemId] = useState("");
  const [priceKind, setPriceKind] = useState<"RESOURCE" | "ITEM">("RESOURCE");
  const [priceResourceType, setPriceResourceType] = useState("insight");
  const [priceResourceAmount, setPriceResourceAmount] = useState("1");
  const [priceItemId, setPriceItemId] = useState("");
  const [priceItemQuantity, setPriceItemQuantity] = useState("1");
  const [isCreating, setIsCreating] = useState(false);

  const refresh = () => {
    fetchShopListings(npcActorId)
      .then(setListings)
      .catch((err) =>
        setError(
          err instanceof Error ? err.message : "Failed to load shop listings",
        ),
      );
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [npcActorId]);

  useEffect(() => {
    if (!isGm) return;
    getActorInventory(npcActorId)
      .then(setNpcInventory)
      .catch(() => setNpcInventory([]));
  }, [isGm, npcActorId]);

  // Every viewer (not just the GM) needs item names to render listing
  // labels — a player buying from a resource-priced or barter listing
  // has no other way to know what "priceItemId: <uuid>" or a listing's
  // own itemId actually names.
  useEffect(() => {
    getWorldItems(worldId)
      .then(setWorldItems)
      .catch(() => setWorldItems([]));
  }, [worldId]);

  const handleCreateListing = async () => {
    if (!newItemId) return;
    setIsCreating(true);
    setError(null);
    try {
      await createShopListing({
        actorId: npcActorId,
        itemId: newItemId,
        priceKind,
        priceResourceType:
          priceKind === "RESOURCE" ? priceResourceType : undefined,
        priceResourceAmount:
          priceKind === "RESOURCE"
            ? Number.parseInt(priceResourceAmount, 10)
            : undefined,
        priceItemId: priceKind === "ITEM" ? priceItemId : undefined,
        priceItemQuantity:
          priceKind === "ITEM"
            ? Number.parseInt(priceItemQuantity, 10)
            : undefined,
      });
      setNewItemId("");
      setPriceItemId("");
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create listing");
    } finally {
      setIsCreating(false);
    }
  };

  const handlePurchase = async (listing: GenieShopListingRecord) => {
    if (!myActor) return;
    setPendingListingId(listing.id);
    setError(null);
    try {
      await purchaseFromShop(listing.id, myActor.id, listing.actorId);
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Purchase failed");
    } finally {
      setPendingListingId(null);
    }
  };

  const itemLabel = (itemId: string) =>
    worldItems?.find((i) => i.id === itemId)?.name ??
    npcInventory?.find((e) => e.itemId === itemId)?.itemName ??
    itemId;

  // Scenario 6: a plain NPC with no listings shows nothing to a non-GM viewer.
  if (listings === null) {
    return isGm ? (
      <p className="text-sm text-muted-foreground">Loading shop…</p>
    ) : null;
  }
  if (!isGm && listings.length === 0) {
    return null;
  }

  return (
    <Card className="grid gap-4 p-6" data-testid="genie-shop-panel">
      <h3 className="text-lg font-semibold">Shop</h3>
      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      {listings.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">No listings yet.</p>
      ) : (
        <div className="grid gap-2">
          {listings.map((listing) => (
            <div
              key={listing.id}
              data-testid={`shop-listing-${listing.id}`}
              className="flex items-center justify-between gap-3 rounded-lg border border-border p-3"
            >
              <div>
                <strong className="text-sm">{itemLabel(listing.itemId)}</strong>
                <p className="text-xs text-muted-foreground">
                  Price:{" "}
                  {listing.priceKind === "resource"
                    ? `${listing.priceResourceAmount} ${listing.priceResourceType}`
                    : `${listing.priceItemQuantity} × ${itemLabel(listing.priceItemId ?? "")}`}
                  {" · "}
                  Stock: {listing.stockQuantity}
                </p>
              </div>
              {!isGm && myActor ? (
                <Button
                  type="button"
                  size="sm"
                  disabled={
                    pendingListingId === listing.id ||
                    listing.stockQuantity <= 0
                  }
                  onClick={() => void handlePurchase(listing)}
                  data-testid={`shop-buy-${listing.id}`}
                >
                  {listing.stockQuantity <= 0 ? "Out of stock" : "Buy"}
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      )}

      {isGm ? (
        <div className="grid gap-2 border-t border-border pt-4">
          <h4 className="text-sm font-semibold">Add listing</h4>
          <select
            value={newItemId}
            onChange={(event) => setNewItemId(event.target.value)}
            disabled={isCreating || npcInventory === null}
            className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
            data-testid="shop-new-listing-item-select"
            aria-label="Item to sell"
          >
            <option value="">
              {npcInventory === null
                ? "Loading NPC inventory…"
                : "Select an item from this NPC's inventory…"}
            </option>
            {(npcInventory ?? []).map((entry) =>
              entry.itemId ? (
                <option key={entry.itemId} value={entry.itemId}>
                  {entry.itemName}
                </option>
              ) : null,
            )}
          </select>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-1 text-sm">
              <input
                type="radio"
                checked={priceKind === "RESOURCE"}
                onChange={() => setPriceKind("RESOURCE")}
              />
              Resource price
            </label>
            <label className="flex items-center gap-1 text-sm">
              <input
                type="radio"
                checked={priceKind === "ITEM"}
                onChange={() => setPriceKind("ITEM")}
              />
              Item barter
            </label>
          </div>
          {priceKind === "RESOURCE" ? (
            <div className="grid grid-cols-2 gap-2">
              <select
                value={priceResourceType}
                onChange={(event) => setPriceResourceType(event.target.value)}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                aria-label="Price resource type"
              >
                {GENIE_SESSION_RESOURCE_TYPES.map((r) => (
                  <option key={r.key} value={r.key}>
                    {r.label}
                  </option>
                ))}
              </select>
              <Input
                type="number"
                min={1}
                value={priceResourceAmount}
                onChange={(event) => setPriceResourceAmount(event.target.value)}
                aria-label="Price amount"
              />
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              <select
                value={priceItemId}
                onChange={(event) => setPriceItemId(event.target.value)}
                disabled={worldItems === null}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                aria-label="Required barter item"
              >
                <option value="">
                  {worldItems === null ? "Loading items…" : "Required item…"}
                </option>
                {(worldItems ?? []).map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                  </option>
                ))}
              </select>
              <Input
                type="number"
                min={1}
                value={priceItemQuantity}
                onChange={(event) => setPriceItemQuantity(event.target.value)}
                aria-label="Required barter quantity"
              />
            </div>
          )}
          <Button
            type="button"
            size="sm"
            onClick={() => void handleCreateListing()}
            disabled={
              isCreating || !newItemId || (priceKind === "ITEM" && !priceItemId)
            }
            data-testid="shop-create-listing-button"
          >
            Add listing
          </Button>
        </div>
      ) : null}
    </Card>
  );
}
