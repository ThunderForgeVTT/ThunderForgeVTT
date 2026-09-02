import type { ItemPriceRecord } from "@/api/items";

/**
 * Rendering a Game Master's price note.
 *
 * # Why formatting is all there is
 *
 * Spec 031 FR-037, ADR-058. The note participates in no transaction: nothing
 * converts it, totals it, or checks a purse against it, and there is no
 * currency system to convert *to* — `currencyLabel` is whatever the Game
 * Master typed. So the whole of this module is turning three stored fields
 * into one honest line of text.
 *
 * # Why "suggested" is said out loud
 *
 * The same item can carry this note and a game system's own price at once
 * (`world_genie_shop_listings` prices per vendor). ADR-058 accepts that on the
 * condition that the interface never shows one and means the other, so a
 * suggestion is labelled as a suggestion rather than presented as the price.
 */
export function formatItemPrice(
  price: ItemPriceRecord | null | undefined,
): string | null {
  if (!price) {
    return null;
  }
  const label = price.currencyLabel?.trim();
  const amount = label ? `${price.amount} ${label}` : `${price.amount}`;
  return price.isSuggested ? `${amount} (suggested)` : amount;
}

/**
 * What the Game Master typed into the amount box, as a number — or `null` if
 * it is not one yet.
 *
 * An empty box means "no note", which is why it is not read as zero: a free
 * item and an unpriced item are different things, and only one of them should
 * write a row.
 */
export function parsePriceAmount(raw: string): number | null {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return null;
  }
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && Number.isInteger(parsed) ? parsed : null;
}
