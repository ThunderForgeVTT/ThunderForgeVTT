# A Game Master's Item Price Is Presentational; Systems Own Economies

- **Date**: 2026-09-01
- **Status**: Accepted
- **Spec**: `specs/031-playability/` (US8, FR-037)

## Context

A Game Master wants to write a price on an item so they have something to
role-play from — "the smith wants forty for it" — without that price meaning
anything mechanically.

The complication is that pricing **already exists**, and it is already
system-specific. `world_genie_shop_listings` models a full economy:
`price_kind`, `price_resource_type`, `price_resource_amount`, `price_item_id`,
`price_item_quantity`, keyed by `actor_id` + `item_id`. Genie put its economy in
**its own table**, not in `world_items`.

That is a decision worth reading twice, because it is evidence rather than
opinion: the one game system this project ships declined to put price on the
generic item. Different rulesets price differently — a currency, a barter, a
resource, a favour — and 5e, Pathfinder and Blades in the Dark would not agree
on what a price even is.

So a naive "add a price column to items" risks creating a **second economy**
running alongside the system's, with no rule about which one is true.

## Decision

**The generic item price is presentational and nothing else.**

It is a Game Master's note, stored in `world_item_prices`, one per item:

| Column | Purpose |
|---|---|
| `item_id` | which item |
| `amount` | the number |
| `currency_label` | free text; this layer names no currency system |
| `is_suggested` | whether it is a suggestion or a set price |

Three rules define it:

1. **It participates in no transaction.** Nothing spends, deducts, validates
   against, or settles with this value. It is text with a number in it.
2. **Systems keep their economies.** `world_genie_shop_listings` and anything
   like it continue to own trade. A system's view is free to display this
   value, ignore it, or override it.
3. **The generic layer never reimplements vendor pricing.** Genie's model is
   **per-vendor** — this NPC sells this item at this price. A GM's note is
   **per-item** and vendor-independent. They are different quantities and both
   may exist; the generic one must not grow toward the other.

## Rationale (Y-Statement)

In the context of letting a Game Master record what an item is worth, facing a
game system that already models pricing per vendor in its own table, we decided
**to store a presentational, per-item note that participates in no
transaction** and neglected **a generic economy on `world_items`**, to achieve
**a useful GM affordance with no second source of truth about value**,
accepting **that the same item can show a GM's note and a system's price at once,
and the interface must make clear which is which**.

## Consequences

**The obvious feature request is now bounded.** "Can items have prices?" has a
cheap yes that cannot metastasise into a shadow economy, because rule 1 forbids
the next step rather than leaving it open.

**Two numbers can appear for one item.** A GM note and a system price may both
exist. That is acceptable and must be presented honestly — a suggested price
labelled as the GM's, a system price presented by the system's own view. What
would not be acceptable is silently showing one and meaning the other.

**`is_suggested` carries intent, not behaviour.** It distinguishes "this is
roughly what it goes for" from "this is the price"; neither is enforced.

**If a generic economy is ever wanted, this is not it.** That would be a new
decision with a different shape — most likely contributed by systems, per
ADR-054's seam, rather than added to the core. This ADR should be revisited
rather than extended.

## Alternatives Considered

- **A `price` column on `world_items`.** Rejected: it makes the generic layer
  look authoritative about value, which it cannot be across rulesets, and it
  would sit in direct tension with the per-vendor model genie already uses.
- **Promote genie's shop model to the generic layer.** Rejected: it encodes one
  ruleset's economy — resources, barter, per-vendor listings — as everyone's.
  Blades in the Dark does not price like Genie, and neither prices like 5e.
- **Defer the whole thing until systems can contribute economies.** Rejected as
  disproportionate: a GM wanting to scribble a number on an item should not have
  to wait for a pack architecture.

## Related Decisions

- **ADR-054** — the contribution seam; the shape any *real* generic economy
  would have to take.
- **ADR-045** — Genie session state and two-party consent for resource trades;
  the existing system-owned economy this decision stays clear of.
