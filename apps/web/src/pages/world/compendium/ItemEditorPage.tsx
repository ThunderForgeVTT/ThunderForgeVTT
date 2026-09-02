import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useNavigate, useParams } from "react-router-dom";
import {
  clearItemPrice,
  createItem,
  getItem,
  setItemPrice,
  suggestItemName,
  updateItem,
  type WorldItemWithPrice,
} from "@/api/items";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { Textarea } from "@/components/ui/textarea";
import { parsePriceAmount } from "@/pages/world/compendium/itemPrice";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 031 (T068/T071, FR-035/FR-037): authoring an item.
 *
 * # Why a page and not a row of boxes under the list
 *
 * Same finding as the NPC editor: creating an item meant two inputs crammed
 * beneath the catalogue table, with nowhere to put anything a Game Master
 * might actually want to write down — a description, and now a price. The list
 * is a list again, and creation happens here behind an explicit Save.
 *
 * # Why the price is only ever text with a number in it
 *
 * ADR-058. Game systems own economies — Genie already prices per vendor, in
 * its own table — so this field settles nothing mechanical: nothing spends it,
 * converts it, or checks a purse against it, and the "suggested" flag is the
 * Game Master's intent rather than a rule. It exists so a Game Master can
 * role-play from a number they wrote down, which is exactly what the playtest
 * asked for and the whole of what it should ever do.
 *
 * # Why the price is a second call
 *
 * It lives in its own table, so setting it is its own mutation. Saving the
 * item's own fields first means a refused price leaves the name and
 * description saved, rather than losing everything to one bad number.
 */
export interface ItemEditorPageProps {
  mode: "create" | "edit";
}

export default function ItemEditorPage({ mode }: ItemEditorPageProps) {
  const { id: worldId = "", itemId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [item, setItem] = useState<WorldItemWithPrice | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [priceAmount, setPriceAmount] = useState("");
  const [currencyLabel, setCurrencyLabel] = useState("");
  const [isSuggested, setIsSuggested] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  // Stored against the name it was looked up for, so whether a nudge is
  // showing stays a render-time question — the same shape the compendium tab
  // used before this page took creation over.
  const [loadedSuggestion, setLoadedSuggestion] = useState<{
    query: string;
    match: WorldItemWithPrice | null;
  } | null>(null);

  // Reset during render rather than at the top of the effect below: this is
  // state derived from the arguments, and doing it in the effect commits one
  // render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${itemId}|${mode}`, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let active = true;

    Promise.all([
      getWorld(worldId),
      mode === "edit" ? getItem(itemId) : Promise.resolve(null),
    ])
      .then(([worldResult, itemResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setItem(itemResult);
        if (itemResult) {
          setName(itemResult.name);
          setDescription(itemResult.description ?? "");
          setPriceAmount(
            itemResult.price ? String(itemResult.price.amount) : "",
          );
          setCurrencyLabel(itemResult.price?.currencyLabel ?? "");
          setIsSuggested(itemResult.price?.isSuggested ?? false);
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, itemId, mode]);

  // FR-020's "did you mean?" nudge, kept with creation as it moved: it is
  // about not authoring the same item twice, which is a creation problem.
  useEffect(() => {
    const trimmed = name.trim();
    if (mode !== "create" || trimmed.length < 2) {
      return;
    }
    let active = true;
    const timer = setTimeout(() => {
      suggestItemName(worldId, trimmed)
        .then((matches) => {
          if (active) {
            setLoadedSuggestion({ query: trimmed, match: matches[0] ?? null });
          }
        })
        .catch(() => {
          if (active) {
            setLoadedSuggestion({ query: trimmed, match: null });
          }
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [worldId, name, mode]);

  const savePrice = async (savedItemId: string) => {
    const amount = parsePriceAmount(priceAmount);
    if (amount === null) {
      // An emptied box means the Game Master no longer has a price in mind,
      // which is not the same as pricing it at zero — so the note goes away.
      if (item?.price) {
        await clearItemPrice(savedItemId);
      }
      return;
    }
    await setItemPrice({
      itemId: savedItemId,
      amount,
      currencyLabel: currencyLabel.trim() || null,
      isSuggested,
    });
  };

  const handleSave = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setStatus("An item needs a name.");
      return;
    }
    if (priceAmount.trim() !== "" && parsePriceAmount(priceAmount) === null) {
      setStatus("A price is a whole number, or nothing at all.");
      return;
    }
    setIsSaving(true);
    setStatus(null);
    try {
      if (mode === "create") {
        const created = await createItem({
          worldId,
          name: trimmed,
          description: description.trim() || undefined,
        });
        await savePrice(created.id);
        navigate(`/world/${worldId}/compendium?tab=items`);
        return;
      }
      const updated = await updateItem({
        itemId,
        name: trimmed,
        description: description.trim(),
      });
      await savePrice(itemId);
      setItem({ ...updated, price: (await getItem(itemId)).price });
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save item");
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading item" />;
  }

  if (mode === "edit" && !item) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">Item not found</h1>
            <p className="text-muted-foreground">
              This item doesn't exist or you don't have access to it.
            </p>
          </Card>
        </main>
      </Container>
    );
  }

  const canEdit = mode === "create" || item?.myPermissionLevel !== "VIEWER";
  const suggestion =
    mode === "create" && loadedSuggestion?.query === name.trim()
      ? loadedSuggestion.match
      : null;

  return (
    <>
      <SEO
        title={
          mode === "create"
            ? `New item — ${world?.name ?? "World"}`
            : `${item?.name ?? "Item"} — Edit`
        }
        description="Item authoring"
        noindex
      />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/compendium?tab=items`)}
          data-testid="item-editor-back"
        >
          Back to Compendium
        </Button>

        <Card className="grid gap-4 p-5" data-testid="item-editor-page">
          <h1 className="text-xl font-semibold">
            {mode === "create" ? "New item" : `Edit ${item?.name}`}
          </h1>

          <Field label="Name" htmlFor="item-editor-name">
            <Input
              id="item-editor-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={!canEdit || isSaving}
              placeholder="Lamp of minor binding"
              data-testid="item-editor-name-input"
            />
          </Field>

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

          <Field label="Description" htmlFor="item-editor-description">
            <Textarea
              id="item-editor-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={!canEdit || isSaving}
              rows={5}
              data-testid="item-editor-description-input"
            />
          </Field>

          <div className="grid gap-3 rounded-lg border border-border p-4">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Price
            </p>
            <p className="text-xs text-muted-foreground">
              A note to role-play from. Nothing in the app spends or checks it,
              and a game system's own prices are its own.
            </p>
            <div className="grid gap-3 sm:grid-cols-2">
              <Field label="Amount" htmlFor="item-editor-price-amount">
                <Input
                  id="item-editor-price-amount"
                  inputMode="numeric"
                  value={priceAmount}
                  onChange={(event) => setPriceAmount(event.target.value)}
                  disabled={!canEdit || isSaving}
                  placeholder="40"
                  data-testid="item-editor-price-amount-input"
                />
              </Field>
              <Field
                label="Currency"
                htmlFor="item-editor-price-currency"
                hint="Whatever this world calls money."
              >
                <Input
                  id="item-editor-price-currency"
                  value={currencyLabel}
                  onChange={(event) => setCurrencyLabel(event.target.value)}
                  disabled={!canEdit || isSaving}
                  placeholder="gp"
                  data-testid="item-editor-price-currency-input"
                />
              </Field>
            </div>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <input
                type="checkbox"
                checked={isSuggested}
                onChange={(event) => setIsSuggested(event.target.checked)}
                disabled={!canEdit || isSaving}
                data-testid="item-editor-price-suggested-input"
              />
              This is a suggestion, not a set price
            </label>
          </div>

          {canEdit ? (
            <Button
              type="button"
              icon="inventory"
              className="justify-self-start"
              disabled={isSaving || !name.trim()}
              onClick={() => void handleSave()}
              data-testid="item-editor-save"
            >
              {mode === "create" ? "Create item" : "Save changes"}
            </Button>
          ) : null}

          {status ? (
            <p
              className="text-sm text-muted-foreground"
              data-testid="item-editor-status"
            >
              {status}
            </p>
          ) : null}
        </Card>
      </Container>
    </>
  );
}
