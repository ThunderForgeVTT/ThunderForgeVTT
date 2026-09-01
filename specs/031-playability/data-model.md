# Phase 1 Data Model: Playability 001

Derived from the spec's Key Entities and the decisions in `research.md`. Every
persisted table follows the project's `created_by` / `updated_by` provenance
convention (Constitution III) and ships as a Diesel migration with paired
`up.sql` / `down.sql`.

Most of this feature needs **no new persistence**. Placement, selection
filtering, snapping and scene transition are engine or client state. What
follows is only what must outlive a session.

---

## New persisted entities

### Actor imagery — `world_actor_images`

Rows keyed by role rather than columns on `world_actors` (research R4).

| Field | Type | Notes |
|---|---|---|
| `id` | Uuid | PK |
| `actor_id` | Uuid | FK `world_actors`, cascade delete |
| `role` | Varchar | which image this is for the actor |
| `asset_id` | Uuid | the stored, transcoded image |
| `created_by`, `updated_by` | Uuid | provenance |
| `created_at`, `updated_at` | Timestamp | |

**Rules**
- At most one image per (`actor_id`, `role`) — unique constraint.
- Roles required by this feature: portrait, token. The column is open so the
  deferred animated set is additive.
- Deleting an actor deletes its imagery rows; the underlying stored objects are
  reclaimed by the same path other image deletions use.

**Why not two columns on `world_actors`**: the deferred VTuber set is *n*
images. Columns would force it into a second mechanism. Recorded in an ADR.

**Related asymmetry**: `world_items` already has `icon_asset_id`. Items are not
migrated by this feature, but the inconsistency is noted so a later pass can
settle it deliberately.

---

### Item price — `world_item_prices`

| Field | Type | Notes |
|---|---|---|
| `id` | Uuid | PK |
| `item_id` | Uuid | FK `world_items`, cascade delete |
| `amount` | Integer | |
| `currency_label` | Text | free text; this layer names no currency system |
| `is_suggested` | Bool | a suggestion to role-play from, versus a set price |
| `created_by`, `updated_by` | Uuid | provenance |
| `created_at`, `updated_at` | Timestamp | |

**Rules**
- At most one price per item — this is the GM's note, not a price list.
- **Presentational only.** It does not participate in any transaction.
- A game system with its own economy — `world_genie_shop_listings` being the
  existing example, keyed per *vendor* — continues to own trade. A system view
  may display, ignore or override this value. The generic layer must not
  reimplement vendor pricing (research R5).

---

### Lore organisation

Two additions to existing lore:

**`world_lore_entries.parent_id`** — nullable self-FK giving the tree.

| Rule |
|---|
| A null parent is a root entry. |
| Cycles are rejected at the data boundary. |
| Deleting a parent **re-parents its children to the grandparent** — the deleted entry's own parent. A deleted root's children become roots, since there is no grandparent to inherit them. Never refuse the delete, and never orphan a subtree to the root when a real home exists. |
| The database keeps `ON DELETE SET NULL` as a backstop only. It is the safe fallback if the application path is ever bypassed — content is never destroyed, merely flattened — but the delete mutation is what implements the rule above. |

**`world_lore_tags`** — many-to-many.

| Field | Type | Notes |
|---|---|---|
| `id` | Uuid | PK |
| `lore_entry_id` | Uuid | FK, cascade delete |
| `tag` | Text | normalised for comparison |
| `created_by` | Uuid | provenance |
| `created_at` | Timestamp | |

Unique on (`lore_entry_id`, `tag`).

**Note for spec 034**: the tree is a blocking dependency of repository path
mapping. Identity stays with the entry's id; the path is a label.

---

## Changed entities

### `tokens` — bringing the party across a scene change

**Blocked on the ADR named in Constitution Check IV.1.** Two candidates from
research R2:

- **A — re-create on arrival.** No schema change. Tokens are created in the
  destination scene from the party's characters, preserving art, ownership and
  size; identity is not preserved.
- **B — party membership.** A token that follows the party, resolved per scene.
  Preserves identity; touches the ownership boundary ADR-040 settled.

Either way: **a character that already has a token in the destination scene
must not gain a second one.**

---

## Client-only state (not persisted server-side)

### Selection filter preference

Which kinds the Select tool acts on, and whether its menu is collapsed.
Per-user, per-device (research R10). Defaults to every kind enabled and the
menu open on first use. A second machine starting from defaults is the safe
outcome.

### Placement in progress

A token attached to the cursor awaiting confirmation. **Engine state, never
persisted — modelled as a Bevy state (`idle → carrying → placed | cancelled`),
not a flag, so that leaving the carry is a single transition with one exit
hook.** It becomes a token only when the server accepts the placement; a
cancelled or interrupted placement leaves nothing behind — including when the
connection drops mid-carry (spec edge case).

---

## Entities this feature reads but does not change

| Entity | Used for |
|---|---|
| `world_actor_claims` | which character a player is bound to (FR-033, FR-034) |
| `world_items`, `world_item_effects` | item presentation and pickup |
| `world_lore_entries`, `world_lore_links` | lore markers, interlinking |
| `world_combats` (`round`, `active_combatant_id`) | combat roster; FR-031's system-driven turn structure is blocked on spec 032 |
| `scenes` (`description`, grid type), `scene_preview_images` | scene list rendering (FR-023) |
| `canvas_image_assets` (`content_hash`) | world cache reporting (FR-042) |

---

## Interaction effect vocabulary

`item.pickup` is **new** and is contributed by the item subsystem, not the
interaction core (research R3, ADR-054). The existing declared set is
`door.reveal`, `door.set_lock`, `door.set_state`, `light.toggle`, `lore.open`,
`nav.request_scene`.

**Pickup is two writes** — remove the scene token, add the inventory entry —
and they must not half-apply. Concurrency is resolved at the database boundary,
exactly as spec 017 resolves two players claiming one character: exactly one
winner, the loser told the item is gone.
