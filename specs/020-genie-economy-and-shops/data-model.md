# Phase 1 Data Model: Genie Session Resource Economy

Extends spec 018/019's existing `world_genie_sessions` / `world_genie_puzzle_clocks`
/ `world_genie_resource_holdings` / `world_genie_trade_proposals` model. No
existing table's shape changes — this spec only adds new tables/columns.

## `worlds.genie_resource_carryover_enabled` (new column)

| Column | Type | Notes |
|---|---|---|
| `genie_resource_carryover_enabled` | `boolean NOT NULL DEFAULT false` | Per-world GM setting (research.md R1). When `true`, `startGenieSession` copies ending holdings from the most recently concluded session into the new one (FR-003). |

## `world_genie_shop_listings` (new table)

| Column | Type | Notes |
|---|---|---|
| `id` | `uuid PRIMARY KEY DEFAULT gen_random_uuid()` | |
| `actor_id` | `uuid NOT NULL REFERENCES world_actors(id)` | The NPC actor selling this listing. Any actor row works mechanically; UI only exposes "add listing" on NPC actors (User Story 2). |
| `item_id` | `uuid NOT NULL REFERENCES world_items(id)` | The item being sold. Must already exist in `actor_id`'s `world_actor_inventory` (stock is that inventory row's `quantity`, not a separate count — FR-004). |
| `price_kind` | `text NOT NULL CHECK (price_kind IN ('resource', 'item'))` | Which of the two price shapes below is populated. |
| `price_resource_type` | `text NULL` | Populated iff `price_kind = 'resource'`. One of the Genie Session Resource types (`insight`/`favor`/`essence`). |
| `price_resource_amount` | `integer NULL CHECK (price_resource_amount IS NULL OR price_resource_amount > 0)` | Populated iff `price_kind = 'resource'`. |
| `price_item_id` | `uuid NULL REFERENCES world_items(id)` | Populated iff `price_kind = 'item'` (barter). |
| `price_item_quantity` | `integer NULL CHECK (price_item_quantity IS NULL OR price_item_quantity > 0)` | Populated iff `price_kind = 'item'`. |
| `created_by` | `uuid NOT NULL REFERENCES users(id)` | Provenance (Constitution Principle III). |
| `created_at` | `timestamptz NOT NULL DEFAULT now()` | |

**Validation rules**:
- Exactly one of `(price_resource_type, price_resource_amount)` or
  `(price_item_id, price_item_quantity)` is non-null, matching `price_kind`
  (DB `CHECK` constraint plus mutation-side validation, mirroring the
  existing resource-vs-item split pattern already used for wish-granted
  items).
- A listing's "stock" is not a separate counter — it *is*
  `world_actor_inventory.quantity` for `(actor_id, item_id)`. A listing with
  zero backing inventory quantity is not purchasable (FR-005) and the GM UI
  hides/removes it once its backing stock hits 0 (User Story 2, Scenario 1).

## `world_genie_puzzle_clock_rewards` (new table)

| Column | Type | Notes |
|---|---|---|
| `id` | `uuid PRIMARY KEY DEFAULT gen_random_uuid()` | |
| `clock_id` | `uuid NOT NULL REFERENCES world_genie_puzzle_clocks(id)` | |
| `trigger_segment` | `integer NOT NULL CHECK (trigger_segment > 0)` | Which `segments_current` value fires this entry. |
| `reward_resource_type` | `text NULL` | Populated iff this entry grants a resource. |
| `reward_resource_amount` | `integer NULL CHECK (reward_resource_amount IS NULL OR reward_resource_amount > 0)` | |
| `reward_item_id` | `uuid NULL REFERENCES world_items(id)` | Populated iff this entry grants an item. |
| `reward_item_quantity` | `integer NULL CHECK (reward_item_quantity IS NULL OR reward_item_quantity > 0)` | |
| `recipient_mode` | `text NOT NULL CHECK (recipient_mode IN ('triggering_actor', 'whole_party'))` | Configuration section, spec.md. |
| `granted_at` | `timestamptz NULL` | Set the instant this entry's reward fires (research.md R4); `NULL` means not yet granted. |
| `created_by` | `uuid NOT NULL REFERENCES users(id)` | |
| `created_at` | `timestamptz NOT NULL DEFAULT now()` | |

**Validation rules**:
- Exactly one of `(reward_resource_type, reward_resource_amount)` or
  `(reward_item_id, reward_item_quantity)` is non-null per row (DB `CHECK`
  plus mutation-side validation). A clock may have multiple reward rows at
  the same `trigger_segment` (e.g. one resource + one item firing together —
  spec.md Configuration section).
- A row with `granted_at IS NOT NULL` is never re-granted — `advancePuzzleClock`
  only selects/grants rows where `granted_at IS NULL` and
  `trigger_segment <= <new segments_current>` and
  `trigger_segment > <old segments_current>` (i.e. newly crossed by this
  advance, not previously-crossed-and-already-granted, not
  not-yet-reached).
- A `recipient_mode = 'triggering_actor'` row grants to the `actorId`
  passed to `advancePuzzleClock` (FR-006a); if that call omitted `actorId`,
  the row is treated as `whole_party` for that grant only (fallback, not a
  stored state change to the row's `recipient_mode`).
- A clock with zero reward rows behaves identically to spec 018/019 today
  (User Story 3, Scenario 4) — the reward-row lookup is simply empty.

## Relationships (delta over spec 018/019's existing model)

```text
worlds (existing)
  └─ genie_resource_carryover_enabled (new column)

world_actors (existing, NPC or PC)
  └─ world_actor_inventory (existing) ── referenced as shop stock, unchanged
  └─ world_genie_shop_listings (new) ── one NPC actor, many listings

world_genie_puzzle_clocks (existing)
  └─ world_genie_puzzle_clock_rewards (new) ── one clock, many reward entries
```

No changes to `world_genie_sessions`, `world_genie_resource_holdings`, or
`world_genie_trade_proposals` — `grantSessionResource` writes through the
existing `set_holding_quantity` helper against the existing holdings table.
