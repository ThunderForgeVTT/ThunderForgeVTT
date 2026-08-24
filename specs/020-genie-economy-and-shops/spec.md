# Feature Specification: Genie Session Resource Economy — Grants, NPC Shops, Quest/Contract Rewards

**Feature Branch**: `020-genie-economy-and-shops`

**Created**: 2026-08-24

**Status**: Draft (design only — not yet implemented)

**Input**: "let's tackle SessionResourceTrade holdings — how do players actually earn resources? the dm should have capacity to just give them to players and add it to their inventories with an npc actor setup a shop or quest/contract results these are 2 systems we havent built yet can you spec them out"

## Clarifications

### Session 2026-08-24

- Q: When a Puzzle Clock's reward is configured as `"triggering_actor"`, which actor gets credited if the GM advances the clock through the existing `advancePuzzleClock` mutation — which today takes only `clockId` and `delta`, no actor at all? → A: Add an optional `actorId` param to `advancePuzzleClock`; when a `triggering_actor`-mode reward fires and no `actorId` was passed, fall back to whole-party split.
- Q: When a shop listing's stock reaches its last unit, what should happen if two players try to buy it at essentially the same moment? → A: Atomic conditional decrement inside the purchase transaction (`UPDATE ... SET quantity = quantity - 1 WHERE quantity > 0`); the second concurrent buyer gets a clean "out of stock" error.

## Problem

Spec 019 wired peer-to-peer Session Resource trading (`proposeResourceTrade`/
`acceptResourceTrade`/`declineResourceTrade`), but there is **no way for a
Session Resource (Insight/Favor/Essence) to enter the economy at all** —
`world_genie_resource_holdings` can only be modified by accepting a trade or
spending on a Puzzle Clock, both of which require holdings to already exist.
Every player starts, and stays, at 0. Trading is fully built and fully
unusable. This spec designs the three missing pieces:

1. **Direct GM grants** — the GM hands resources (and/or items) to a player
   outright. The minimum viable fix; unblocks everything else.
2. **NPC shops** — an NPC actor a player can buy items from, paying in
   Session Resources.
3. **Quest/Contract rewards** — completing an objective grants
   resources/items automatically.

## Research Summary (confirmed against the current codebase)

- **Item → inventory** already has the right primitive:
  `addItemToInventory(actorId, itemId, quantity)`
  (`src/server/src/graphql/mutations_inventory.rs:196`), upserting
  `world_actor_inventory` on `(actor_id, item_id)`, gated by
  `require_actor_permission(..., Editor)` on the *actor* — not
  item-ownership-based, so this is naturally GM-controlled in practice
  (players default to Viewer on characters they don't own) without any new
  authorization concept. This mutation needs no changes for anything below.
- **`world_genie_resource_holdings`** is keyed
  `(session_id, actor_id, resource_type)` with a reusable
  `set_holding_quantity`/`load_holding_quantity` helper pair
  (`mutations_genie_session.rs:208-259`) already used by trade/clock-spend
  logic. **Real constraint**: holdings have no existence outside an active
  `world_genie_sessions` row — a GM cannot grant resources when no session
  is running. Resolved below: whether holdings carry into the *next*
  session is a per-world GM setting, not a hardcoded rule (see
  "Configuration" section).
- **NPC inventory already works as "shop stock" with zero schema changes** —
  `world_actor_inventory` is keyed purely by `actor_id`, `ActorDetailPage.tsx`
  renders `ActorInventoryPanel` identically for `is_npc: true` or `false`,
  and an existing test (`genie_wish_granted_item_round_trips_through_inventory`)
  already proves items round-trip through an NPC actor's inventory. The gap
  is only *buying* (an atomic price-check + resource-deduct + item-transfer),
  not inventory-holding.
- **No generic currency concept exists.** The only precedent is dnd5e's own
  pack-private `CurrencyPurse` (`packs/systems/dnd5e/server/src/models.rs:40`)
  embedded in that pack's `resource_data` — not a shared table. Consistent
  with that precedent, and with Session Resources already being
  Genie-specific rules (spec 019's finding, unchanged), **shop pricing stays
  scoped to Genie's own economy** (its Session Resources and its own
  items), not a new cross-pack currency — but *within* that scope, a price
  can be either a Session Resource amount or an item-for-item trade,
  per-listing (see "Configuration").
- **No quest/contract/objective tracking exists anywhere in this repo** —
  confirmed by repo-wide grep. However, `world_genie_puzzle_clocks`
  (`label, segments_current, segments_max, resolved_at`) is *already*
  mechanically "an objective, tracked with progress, with completion
  events at each tick" — the same shape a repeatable production/quest
  process needs (see the blacksmithing example below). This spec extends
  Puzzle Clocks with configurable, per-segment rewards rather than
  inventing a parallel system.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A GM grants Session Resources and items directly (Priority: P1)

A GM wants to hand a player 2 Insight for good roleplay, or drop a
Wish-Granted Item into their inventory, without needing an NPC, a shop, or a
trade partner. This is the bootstrapping fix everything else depends on for
testing — right now there is no way to get a single unit of any resource
into the economy at all.

**Independent Test**: As GM, with an active Genie session, grant a
resource/item to a player's character; confirm it appears in that
character's holdings/inventory immediately, live on the recipient's own
client (spec 018's cross-client sync applies here identically).

**Acceptance Scenarios**:

1. **Given** an active Genie session and a party member's character,
   **When** the GM grants 3 Essence to that character, **Then** the
   character's `genieResourceHoldings` for `essence` increases by exactly 3,
   and this is visible on that player's own `SessionResourceTrade` panel
   without a manual reload.
2. **Given** the same setup, **When** the GM grants an existing world item to
   that character, **Then** it appears in `ActorInventoryPanel` for that
   character.
3. **Given** no Genie session is currently active, **When** the GM attempts
   to grant a resource, **Then** the mutation fails with a clear error
   ("start a session first") — granting still requires a session to exist
   (holdings have no meaning outside one), independent of the world's
   carryover setting below.
4. **Given** a non-GM caller, **When** they attempt to grant a resource or
   item to any character (including their own), **Then** the mutation is
   rejected — granting is GM-only, distinct from the peer-to-peer trading
   spec 019 already allows between two players.
5. **Given** a world with `resourceCarryover` enabled (see "Configuration"),
   **When** the GM starts a new Genie session after a previous one
   concluded, **Then** every character's ending holdings from the prior
   session are copied into the new session's holdings before play begins —
   nobody has to be re-granted resources they already earned. **Given**
   `resourceCarryover` disabled (the default), **When** a new session
   starts, **Then** every character begins at 0, matching the Wish Pool's
   existing per-session reset.

---

### User Story 2 — An NPC actor sells items for Session Resources or barter (Priority: P2)

A GM sets dressing for an NPC merchant: stock a few items in the NPC's own
inventory, and for each listing choose how it's priced — a Session Resource
amount (2 Insight), or an item-for-item trade (this dagger for that
lantern) — and let players buy. This reuses the existing
NPC-inventory-as-stock pattern rather than inventing a second inventory
concept; only the price side is configurable per listing.

**Independent Test**: As GM, add items to an NPC's inventory, price one in
Session Resources and another as an item-for-item trade; as a player,
complete a purchase of each kind; confirm the correct payment is deducted,
the item appears in the buyer's inventory, and the NPC's stock decrements.

**Acceptance Scenarios**:

1. **Given** a GM-designated NPC with an item in its inventory and a
   resource-priced listing (2 Insight), **When** a player with ≥2 Insight
   buys it, **Then** the player's Insight holding decreases by 2, the item
   moves into the player's inventory, and the NPC's stock quantity for that
   item decreases by 1 (or the listing is removed if it was the last unit).
2. **Given** the same setup, **When** a player with <2 Insight attempts to
   buy it, **Then** the purchase is rejected with a clear "insufficient
   resources" error and no state changes on either side.
3. **Given** a listing priced as an item-for-item trade (e.g. "1 Rusty
   Lantern for 1 Sealed Flask"), **When** a player who owns the required
   item(s) in sufficient quantity purchases it, **Then** that item is
   removed from the player's inventory, the listed item is added to it, and
   the NPC's stock adjusts symmetrically (its stock of the sold item
   decreases, and — if the GM wants the NPC to actually collect what it's
   paid in — the traded-in item increases in the NPC's own inventory).
4. **Given** a player who doesn't hold the required barter item(s), **When**
   they attempt that purchase, **Then** it's rejected the same way an
   underfunded resource purchase is.
5. **Given** a listing with exactly 1 unit in stock, **When** two players
   attempt to buy it at essentially the same moment, **Then** exactly one
   purchase succeeds and the other fails cleanly with "out of stock" — no
   double-sale, and the losing attempt leaves the loser's resources/items
   untouched (FR-005a).
6. **Given** an NPC with no listings configured, **When** any player views
   that NPC, **Then** no shop UI appears (a plain NPC, not every NPC becomes
   a shop by default).

---

### User Story 3 — Puzzle Clock segments can each carry their own configured reward (Priority: P2)

A Puzzle Clock isn't only "one objective, one payout at the end" — the GM
also needs it to model a *production run*: a blacksmith forging 20 daggers,
where each tick of the clock (each day/session/segment of work) completes
one dagger, not just the 20th. So rewards aren't a single field on the
clock — the GM configures **any number of reward entries**, each tied to
whichever segment(s) should trigger it: one entry firing at segment 20 for
a single "Recover the Sealed Lamp"-style payout, or twenty entries (one per
segment, each granting one dagger) for the blacksmithing case, or anything
in between (a milestone at segment 5, another at 10, a big one at 20).

**Independent Test**: As GM, configure a Puzzle Clock with a reward on
every segment (the blacksmithing case) and separately one with a single
reward only at the final segment (the quest case); advance each; confirm
rewards fire exactly at their configured segment(s), not before, not
repeated, and that a clock with zero configured rewards behaves exactly as
it does today (no side effects, spec 018/019 unchanged).

**Acceptance Scenarios**:

1. **Given** a Puzzle Clock ("Forge Daggers," 20 segments) with a reward
   entry configured on every segment 1-20 (`recipient_mode: "triggering_actor"`),
   each granting 1 "Dagger" item, **When** the GM advances it one segment at
   a time (passing the smith's `actorId` on each `advancePuzzleClock` call
   per FR-006a) across several sessions, **Then** each advance grants
   exactly one dagger to that actor — not a lump sum at segment 20.
2. **Given** a Puzzle Clock ("Recover the Sealed Lamp," 4 segments) with a
   single reward entry configured only at segment 4 (2 Favor, split across
   the current party), **When** it's advanced to segment 4, **Then** the
   reward grants once, at that final tick, exactly as spec'd before this
   revision — a single-payout clock is just the one-entry case of the same
   mechanism, not a separate code path.
3. **Given** any reward entry, **When** its configured segment is reached,
   **Then** it grants exactly once — re-advancing past it (impossible once
   `segments_current` is clamped to `segments_max`) or any other replay
   path must not double-grant.
4. **Given** a Puzzle Clock with zero configured reward entries (the common
   case today — existing clocks have none), **When** it's advanced or
   resolves, **Then** behavior is unchanged from spec 018/019.

## Functional Requirements

- **FR-001**: A GM-only `grantSessionResource(sessionId, actorId, resourceType, amount)` mutation MUST increase the named actor's holding via the existing `set_holding_quantity` helper, authorized by the same GM/session-owner check `advanceDoomClock`/`spendWish` already use (not `require_actor_permission`, since holdings aren't actor-permission-gated today).
- **FR-002**: Granting an item to a character MUST reuse the existing `addItemToInventory` mutation unchanged — no new mutation needed for the item half of a grant, only a UI affordance (e.g. a "Grant" action on `ActorInventoryPanel` alongside its existing controls).
- **FR-003**: `startGenieSession` MUST, when the world's `resourceCarryover` setting is enabled, copy every character's ending holdings from the most recently concluded session (by `created_at`) into the newly created session's holdings before returning — otherwise holdings start empty, as today.
- **FR-004**: A new `world_genie_shop_listings` table MUST record a stocked item plus a *configurable price*: either a Session Resource amount, or a required item + quantity (barter) — one listing, one price kind, chosen at creation (see "Configuration" for the exact shape).
- **FR-005**: A `purchaseFromShop(listingId)` mutation MUST atomically, in one DB transaction (mirroring `accept_resource_trade_impl`'s existing pattern): verify the buyer can afford the listing's configured price (resource balance or held item quantity, whichever kind it is), deduct/transfer that price, transfer one unit of the listed item into the buyer's inventory (reusing `addItemToInventory`'s `_impl`), and decrement the NPC's stock.
- **FR-005a**: The stock decrement in FR-005 MUST be a single atomic conditional UPDATE (`SET quantity = quantity - 1 WHERE quantity > 0`, checking rows-affected) inside that same transaction, not a separate read-then-write — so two concurrent purchases of the last unit cannot both succeed. The losing concurrent purchase MUST fail with a clear "out of stock" error and no partial state change (no payment deducted, no item transferred).
- **FR-006**: A new `world_genie_puzzle_clock_rewards` table MUST let the GM configure any number of reward entries per clock, each naming a trigger segment, a resource-or-item payout, and a recipient rule (see "Configuration"). Advancing a clock past a configured segment MUST grant that entry's reward exactly once, in the same transaction as the segment update.
- **FR-006a**: `advancePuzzleClock(clockId, delta)` MUST gain a new optional `actorId` argument (backward-compatible — every existing caller that omits it is unaffected). A `"triggering_actor"`-mode reward entry MUST credit that `actorId` when supplied; if omitted (a GM's plain "Advance" click with no actor context), that reward entry MUST fall back to whole-party split rather than failing or crediting no one.
- **FR-007**: All grant/purchase/reward events MUST record a `world_events` NOTIFY (reusing `EVENT_CODE_GENIE_SESSION_STATE` with a new payload `kind`, e.g. `"resource_grant"`/`"purchase"`/`"clock_reward"`) so spec 018's live cross-client sync reflects them without a manual refetch, consistent with every other Genie session mutation.

## Configuration

Three settings/shapes, all confirmed as GM-configurable rather than fixed
behavior:

**1. Resource carryover** — a per-world boolean, e.g. `worlds.genie_resource_carryover`
(or a small `world_genie_settings(world_id, resource_carryover)` table if a
system-settings pattern elsewhere in this repo already favors a side table
over a column — check `WorldSystemSettingsPage.tsx`'s existing pattern
before picking). Default `false` (reset each session, matching the Wish
Pool). When `true`, `startGenieSession` carries forward ending balances per
FR-003 — "the rope doesn't disappear."

**2. Puzzle Clock rewards** — `world_genie_puzzle_clock_rewards`:
`(id, clock_id, trigger_segment, reward_resource_type, reward_resource_amount, reward_item_id, reward_item_quantity, recipient_mode, granted_at)`.
- `trigger_segment`: which `segments_current` value fires this entry. A GM
  can add one entry at the final segment (single quest payout) or one per
  segment 1..N (the blacksmithing/production case) or any mix.
- Exactly one of the resource pair or the item pair populated per row (a
  clock can have both a resource-reward entry and an item-reward entry at
  the *same* trigger_segment if the GM wants both to fire together — that's
  two rows, not a new column).
- `recipient_mode`: `"triggering_actor"` (the actor who performed the
  advance that hit this segment — the natural default for the
  per-tick/production case, e.g. each dagger goes to whoever forged it) or
  `"whole_party"` (split/granted to every current party member — the
  natural default for a single end-of-quest payout). GM picks per entry
  when configuring it; no single hardcoded rule for the whole feature.
  `"triggering_actor"` requires an actor to attribute to — per FR-006a,
  `advancePuzzleClock` gains an optional `actorId` for exactly this; a
  `spendResourceOnPuzzleClock` advance always has one already. A
  `"triggering_actor"` entry hit via a plain `advancePuzzleClock` call with
  no `actorId` supplied falls back to `"whole_party"` behavior for that
  grant, rather than failing.
- `granted_at`: set the instant this entry's reward fires, both to record
  history and to guarantee FR-006's "exactly once" even if `advancePuzzleClock`
  is somehow called again at the same segment.

**3. Shop listing price** — `world_genie_shop_listings`:
`(id, actor_id, item_id, quantity, price_kind, price_resource_type, price_resource_amount, price_item_id, price_item_quantity)`.
`price_kind` is `"resource"` or `"item"`; only the matching pair of columns
is populated. This is the same "configurable, not hardcoded" shape as
Puzzle Clock rewards above, deliberately — both are "pay in resources or
pay in items" decisions and should look the same in the schema.

## Out of Scope (this spec)

- A generic (non-Genie) currency/shop concept other packs could reuse — per
  spec 019's precedent (dnd5e's own pack-private `CurrencyPurse`), building
  shared infrastructure with only one consumer is premature.
- Player-initiated shop *listings* (an NPC selling back to a player, or a
  player-run shop) — GM-configured listings only, matching who currently
  controls NPCs.
- Any UI/mechanic for *declining* a grant (unlike trades, a GM grant is not
  consensual by design — it's the GM's table, the GM's call).
- A UI to bulk-create "one reward entry per segment" for the blacksmithing
  case (e.g. "grant this item on every segment from 1 to 20" in one click)
  — FR-006's schema supports it (nothing stops a GM from configuring 20
  individual rows), but a bulk-creation convenience is a UI nicety for a
  later pass, not required for the mechanism to work.
