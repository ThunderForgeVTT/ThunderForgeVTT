# Feature Specification: Items & Inventory System

**Feature Branch**: `013-items-inventory`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description: "i want to use /speckit-specify and mimic how we do lore and actors for items. items can be referenced in lore same as actors and actors can have items in their inventory system with quantity amounts think like a mmo rpg style but we will overlay system rules like dnd 5e and others ontop of this so items should be a description and configurable effects. like a healing potion could be applies +3d6 to hit points OR a sword might have roll 1d20 + STAT + MODIFIERS to hit someone and 2d8 to damage hit points"

## Clarifications

### Session 2026-08-23

- Q: Should this spec include an explicit "use item" action that triggers an effect and consumes one unit of a consumable's quantity, or is that future work? → A: Future work, deferred to a dedicated future dice-roller/resolution spec — but the Item Effect data model MUST be scaffolded now (effect types broad enough to cover stat boosts/detriments alongside heal/damage/attack-roll, and structured so a "trigger"/consumption concept can be added later without redesign), not merely left as a TODO with no shape.
- Q: Should an Item's icon/image be required when a DM creates it, or optional? → A: Optional — an Item can be created and saved with just a name and description; icon/image can be added or changed later, matching how Actor and Lore Entry images already work.
- Q: Should Item names be required to be unique within a world, or can two Items share the same name? → A: Names can collide (no uniqueness constraint) — but while a DM is typing a new Item's name, the system surfaces a "did you mean [existing Item]?" prompt when it matches an existing Item closely, so duplicate creation is a deliberate choice rather than an accident. Items are also identified/stored by UUID in the database (same as actors), and participate in the same actor-style share-link-and-copy-to-another-world mechanism (spec 010, User Story 5), not just in-world viewing.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - DM authors an Item with a description and structured effects (Priority: P1)

A DM creates a new Item (e.g., "Potion of Healing" or "Longsword") in the world's Compendium, gives it a name, description, and icon/image, then attaches one or more structured effects to it — for example a "heal" effect with a `3d6` formula targeting Hit Points, or an "attack roll" effect (`1d20 + STAT + MODIFIERS`) paired with a "damage" effect (`2d8` targeting Hit Points). The Item is saved and appears in the world's Item catalog.

**Why this priority**: This is the foundational authoring capability — without a real Item entity with structured, formula-based effects, there is nothing to reference from lore or hold in an actor's inventory.

**Independent Test**: As a DM, create an Item with a name, description, and icon; add a heal effect (`3d6` → Hit Points) and, on a second item, an attack-roll effect (`1d20 + STAT + MODIFIERS`) plus a damage effect (`2d8` → Hit Points); save both and confirm they appear in the world's Item catalog with their effects intact on reload.

**Acceptance Scenarios**:

1. **Given** a DM is in a world's Compendium, **When** they create a new Item and provide a name, **Then** a new Item is created and appears in the world's Item catalog.
2. **Given** a DM is editing an Item, **When** they add an effect specifying a type (heal/damage/modifier/attack-roll), a dice/formula string, and a target resource or attribute name, **Then** the effect is saved as structured data (not freeform text) and is displayed back in the same structured form when the Item is reopened.
3. **Given** a DM is editing an Item, **When** they add more than one effect (e.g., an attack-roll effect and a separate damage effect on the same weapon), **Then** both effects are saved independently and both are shown when viewing the Item.
4. **Given** a DM removes an effect from an Item and saves, **When** the Item is reopened, **Then** the removed effect is no longer present and the remaining effects are unchanged.

---

### User Story 2 - Actors hold Items in a quantity-based inventory (Priority: P1)

A DM or an Editor/Owner of an Actor adds an Item to that Actor's inventory along with a quantity (e.g., 3 Potions of Healing), and can later adjust that quantity (e.g., using one potion drops the count to 2) or remove the Item from the inventory entirely. A Longsword can be added with quantity 1. The Actor's sheet shows every Item currently in its inventory alongside its quantity.

**Why this priority**: This is the other half of the feature's core value — an Item that can never be held by a character isn't useful yet. It depends on Items existing (User Story 1) but is equally central to the "MMO-style inventory" the request calls out.

**Independent Test**: As an Editor/Owner of an Actor, add an existing Item to that Actor's inventory with quantity 3; confirm it appears on the Actor's sheet showing quantity 3; decrease the quantity to 2 and confirm the sheet reflects it; remove the Item entirely and confirm it no longer appears.

**Acceptance Scenarios**:

1. **Given** a user has Editor or Owner access to an Actor, **When** they add an existing Item to that Actor's inventory with a quantity, **Then** the Item appears in the Actor's inventory list showing that quantity.
2. **Given** an Item is already in an Actor's inventory, **When** a user with Editor or Owner access on that Actor changes its quantity (including adding more of the same Item, which increases the existing entry's quantity rather than creating a duplicate row), **Then** the inventory reflects the new quantity immediately.
3. **Given** an Item's quantity in an Actor's inventory is reduced to zero, **When** the change is saved, **Then** the Item is removed from the inventory list (a zero-quantity row is not retained).
4. **Given** a user with only Viewer access to an Actor, **When** they view that Actor's sheet, **Then** they can see the inventory list and quantities but have no add/remove/adjust controls.
5. **Given** a user without at least Viewer access to an Actor, **When** they attempt to view or modify that Actor's inventory, **Then** access is denied, consistent with the Actor's existing permission model.

---

### User Story 3 - Items are referenced from Lore the same way Actors are (Priority: P2)

While writing a lore entry, a DM links to an existing Item using the same in-text link syntax already used to link to lore entries and actors (e.g., `[[Potion of Healing]]`). The Item's detail view gains its own "linked from (lore)" list, exactly mirroring how actors already work with lore.

**Why this priority**: This directly fulfills the request's explicit instruction that items be referenceable from lore the same way actors are; it depends on Items existing (User Story 1) and on the Lore Wiki's in-text linking system (spec 012) already being in place.

**Independent Test**: As a DM, create an Item and a lore entry; from the lore entry, link to the Item using the in-text link syntax; confirm the rendered link navigates to the Item's detail view, and that the Item's detail view lists that lore entry under "linked from."

**Acceptance Scenarios**:

1. **Given** a DM is editing a lore entry's Markdown body, **When** they type the in-text link syntax and select an existing Item from the resolution/autocomplete (alongside lore entries and actors), **Then** the saved content renders that reference as a working link to the Item.
2. **Given** an Item is linked to from one or more lore entries, **When** a user views that Item's detail page, **Then** they see a "linked from" list naming every lore entry that references it, kept in sync automatically as links are added or removed elsewhere.
3. **Given** an in-text link's title matches an Item, a lore entry, and an actor simultaneously, **When** the author selects from the autocomplete, **Then** all three are presented as distinct, disambiguated choices so the author picks the intended target explicitly.
4. **Given** a DM deletes an Item that lore entries link to, **When** those entries are subsequently viewed, **Then** the link renders as unresolved/broken (matching the existing broken-link behavior for deleted actors/lore entries), rather than being blocked or silently cascading.

---

### User Story 4 - Any world member can browse the Item catalog read-only (Priority: P2)

A player (non-DM world member) opens the world's Compendium, selects the Items tab, and browses/searches the full Item catalog and previews any Item's description and effects, exactly as the DM does, but without any create/edit affordances they don't already have permission for — matching the existing NPC catalog behavior on the Compendium's NPCs tab.

**Why this priority**: The Compendium is a shared world-reference surface; players benefit from being able to look up an item's effects without pinging the DM. Depends on the Item catalog existing (User Story 1) and reuses the Compendium shell from spec 011.

**Independent Test**: As a non-DM world member, navigate to the Compendium's Items tab, confirm the table, search, and row-preview work identically to the DM's view, but the "Add Item" control and any Edit actions on Items the player doesn't have Editor/Owner access to are absent.

**Acceptance Scenarios**:

1. **Given** a Player (non-DM) world member opens the Compendium's Items tab, **When** the page loads, **Then** they see a searchable table of the world's Items (name, description) populated with real data, matching the NPCs tab's existing search-as-you-type behavior.
2. **Given** a Player selects an Item row, **When** the preview panel opens, **Then** they see that Item's description and structured effects, with a "View" action always available and an "Edit" action only if their effective permission on that Item is Editor or Owner.
3. **Given** a Player is on the Compendium's Items tab, **When** they look for an "Add Item" control, **Then** it is not present (DM/GM-only, matching spec 010's actor-creation rule).

---

### User Story 5 - Share an Item and copy it into another world (Priority: P3)

A world member with Owner-level access to an Item (including the DM, who always has implicit Owner-level access) generates a shareable link for that Item. Anyone who opens that link sees a read-only preview of the Item's name, description, icon, and effects. If logged in, they can choose "Copy to World," pick one of their own worlds where they hold DM-level access, confirm, and receive a brand-new, fully independent copy of the Item in that world — never referentially linked back to the source.

**Why this priority**: This directly mirrors the actor share/copy mechanism (spec 010, User Story 5) that the request asked Items to follow "the same way," letting DMs build a reusable library of items across worlds/campaigns. It depends on the Item entity existing (User Story 1) and is additive polish sequenced after the core authoring/inventory/lore-linking stories.

**Independent Test**: As a member with Owner-level access to an Item, generate a share link; open it as a different, unrelated user; confirm a read-only preview renders with no edit controls. Click "Copy to World," pick a destination world, confirm, and verify a new, fully independent Item appears in that world's Item catalog. Edit either copy afterward and confirm the other is unaffected.

**Acceptance Scenarios**:

1. **Given** a member with Owner-level access to an Item, **When** they generate a share link, **Then** the system produces a stable, shareable URL for that specific Item.
2. **Given** any user opens a valid share link, **When** the page loads, **Then** they see a read-only preview of the Item's data (name, description, icon, effects) with no edit controls and no ownership-block visibility.
3. **Given** a logged-in user viewing a shared Item, **When** they click "Copy to World," **Then** they are shown a list of their own worlds where they hold DM-level access and must pick one and confirm before anything is copied.
4. **Given** a user confirms copying a shared Item into one of their worlds, **When** the copy completes, **Then** a brand-new Item (new identity, no reference back to the source, empty ownership block, independent copies of all its effects) appears in that world's Item catalog, with a clear success confirmation.
5. **Given** a copied Item exists in a destination world, **When** either the original or the copy is later edited, **Then** the other is completely unaffected.
6. **Given** the Item's Owner-level member revokes a previously generated share link, **When** anyone attempts to open that link afterward, **Then** they see a clear "no longer available" state instead of the Item's data.

---

### Edge Cases

- What happens when a DM types a new Item's name that closely matches an existing Item's name in the same world? The creation form surfaces a "did you mean [existing Item]?" prompt naming the close match, but does not block saving — duplicate names remain allowed; the prompt only makes accidental duplication a deliberate choice.
- What happens when a world has zero Items yet? The Item catalog shows a genuine empty state ("No Items yet" plus, for a DM, the add-Item control), never placeholder/lorem-ipsum content.
- What happens when an Item with a malformed or empty dice formula is saved? The system rejects the save with a clear validation error rather than persisting an effect that cannot later be resolved into a roll.
- What happens when an Item referenced in an Actor's inventory is deleted while still held by one or more Actors? The inventory entry remains visible but is marked as referencing a deleted Item (consistent with the broken-link handling used elsewhere), rather than silently disappearing or blocking the deletion.
- What happens when a DM tries to set an inventory quantity to a negative number? The system rejects the change with a clear validation error; quantity must be a non-negative whole number.
- What happens when the same Item is added to an Actor's inventory twice in quick succession? The system merges into a single inventory entry with a summed quantity rather than creating duplicate rows for the same Item.
- What happens when an effect's target resource/attribute name doesn't correspond to anything the eventual overlaid ruleset recognizes (e.g., a typo'd stat name)? The system stores it as-authored (this spec does not validate stat/resource names against any specific ruleset) — resolution/validation against a concrete ruleset is out of scope here.
- What happens on a very small viewport? The Item catalog, Item detail/editor, and an Actor's inventory list must remain usable — no dedicated mobile layout is required, but nothing should become totally inaccessible.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a world-scoped Item entity with a human-supplied name, a description, an optional icon/image (an Item MAY be created and saved without one, and one MAY be added or changed later), and zero or more structured effects.
- **FR-002**: Only the DM (a world member holding the Owner or GM role, per the precedent in spec 010) MUST be able to create a new Item.
- **FR-003**: Every Item MUST have an ownership block — a per-world-member record of Viewer/Editor/Owner permission level — using the same model, defaults (Viewer for members with no explicit entry), and DM-always-full-control rule established for actors in spec 010 and reused for lore in spec 012.
- **FR-004**: An Item's effect MUST be stored as structured data with at minimum: an effect type (at least `heal`, `damage`, `modifier` — covering stat boosts and detriments/buffs and debuffs alike — and `attack-roll`, extensible to future types), a dice/formula string (e.g., `3d6`, `1d20 + STAT + MODIFIERS`, `2d8`), and a target resource or attribute name (e.g., `Hit Points`) expressed generically — this spec does not define or validate any specific ruleset's set of stats, resources, or modifier calculations.
- **FR-004a**: The Item Effect data model MUST be scaffolded to anticipate (without implementing) a future "trigger"/consumption concept — e.g., distinguishing an on-use consumable effect (a potion, consumed on trigger) from a passive/always-active effect (an equipped weapon's modifier) — so that a later dice-roller/resolution spec can add real triggering and quantity-consumption behavior without redesigning the Item Effect entity. No trigger evaluation, dice rolling, or automatic quantity consumption is implemented by this spec.
- **FR-005**: The system MUST let a user with Editor or Owner access to an Item add, edit, or remove that Item's effects independently of each other (an Item may carry more than one effect, e.g., a weapon with both an attack-roll effect and a separate damage effect).
- **FR-006**: The system MUST reject saving an Item effect with an empty or structurally invalid formula string, with a clear validation error.
- **FR-007**: The Item MUST be presented in the world's Compendium as a fully-functional "Items" tab (replacing the existing placeholder from spec 011), matching the NPCs tab's existing searchable-table-plus-row-preview pattern.
- **FR-008**: Any world member (DM or player) MUST be able to browse, search, and preview Items in the Compendium; create and edit affordances remain gated by the existing DM-only-create and per-item ownership-block rules.
- **FR-009**: Every Actor MUST have an inventory: an ordered collection of (Item, quantity) entries, where quantity is a non-negative whole number.
- **FR-010**: A user with Editor or Owner access to an Actor MUST be able to add an Item to that Actor's inventory with a quantity, adjust an existing entry's quantity (including merging repeated adds of the same Item into a single entry's quantity rather than creating duplicates), or remove an entry entirely.
- **FR-011**: When an inventory entry's quantity is reduced to zero, the system MUST remove that entry from the Actor's inventory rather than retaining a zero-quantity row.
- **FR-012**: The system MUST reject setting an inventory entry's quantity to a negative number, with a clear validation error.
- **FR-013**: A user with at least Viewer access to an Actor MUST be able to view that Actor's full inventory (Items and quantities); inventory add/remove/adjust controls MUST require Editor or Owner access on that Actor, independent of the acting user's permission on the Item itself.
- **FR-014**: The system MUST support the existing in-text link syntax (per spec 012) resolving to an Item, in addition to its existing resolution to lore entries and actors, from within a lore entry's Markdown body.
- **FR-015**: Every Item MUST maintain an automatically-derived, always-current "linked from" list of every lore entry whose body currently contains a resolved in-text link to it, mirroring the existing behavior for actors.
- **FR-016**: When an in-text link's title matches more than one kind of target (lore entry, actor, and/or Item) simultaneously, the authoring UI MUST present all matching candidates as distinct, disambiguated choices so the author explicitly picks the intended target.
- **FR-017**: Deleting an Item MUST NOT be blocked by the existence of other content referencing it (lore in-text links, or Actor inventory entries); lore links to a deleted Item MUST subsequently render as unresolved/broken (matching FR-007 of spec 012), and Actor inventory entries referencing a deleted Item MUST remain visible but clearly marked as referencing a deleted Item.
- **FR-018**: The system MUST deny access to an Item's detail data for any user without at least Viewer access under its ownership block, consistent with the actor and lore permission models.
- **FR-019**: The system MUST NOT enforce Item name uniqueness within a world — two Items may share the same name.
- **FR-020**: While a DM is entering or editing an Item's name, the system MUST surface a non-blocking "did you mean [existing Item]?" prompt when the entered name closely matches an existing Item in the same world, so accidental duplicate naming is a deliberate choice rather than a silent accident.
- **FR-021**: Every Item MUST be identified and stored by a UUID-based identifier, matching the storage convention already established for actors (spec 010).
- **FR-022**: The system MUST let any world member with Owner-level access to an Item (including the DM's implicit access) generate a shareable link for that specific Item, mirroring the actor share-link mechanism (spec 010, FR-023).
- **FR-023**: Opening a valid, non-revoked Item share link MUST show a read-only preview of the Item's full data (name, description, icon, effects) without exposing edit controls or the Item's ownership block, regardless of the viewer's own world membership.
- **FR-024**: A logged-in viewer of a shared Item MUST be able to choose "Copy to World," see a list of their own worlds where they hold DM-level access, and select one as the destination before anything is copied.
- **FR-025**: Confirming a copy MUST create a new, independent Item record (including independent copies of all of its effects) in the destination world, with a new identity that has no live or referential link back to the source; the copy's ownership-block entries MUST start empty (destination DM has implicit full control), mirroring FR-026/FR-030 of spec 010.
- **FR-026**: After copying, edits to the source Item MUST NOT affect the copy, and edits to the copy MUST NOT affect the source.
- **FR-027**: The Item-level Owner (or DM) who generated a share link MUST be able to revoke it; a revoked link MUST show a clear "no longer available" state to anyone who opens it afterward, rather than the Item's data.

### Key Entities *(include if feature involves data)*

- **Item**: A world-scoped entity with a human-supplied name, description, icon/image, an ownership block (Viewer/Editor/Owner per world member, same model as Actor and Lore Entry), and zero or more Item Effects. Valid in-text link target from lore, alongside lore entries and actors.
- **Item Effect**: A structured, system-agnostic building block attached to an Item, with an effect type (`heal`, `damage`, `modifier`, `attack-roll`, extensible to future types), a dice/formula string, and a target resource or attribute name expressed generically (e.g., "Hit Points", "STAT"). Does not itself resolve or apply anything — it is authored data intended for a future ruleset-specific resolution/rolling system to consume.
- **Inventory Entry**: A join between one Actor and one Item recording a non-negative whole-number quantity. An Actor has zero or more Inventory Entries, at most one per distinct Item (repeated adds of the same Item merge into that Item's existing entry).
- **Actor** (existing, reused from spec 010): Gains an inventory (a collection of Inventory Entries); no other change to the Actor entity itself. Inventory visibility follows the Actor's existing Viewer/Editor/Owner permission model — not the Item's.
- **Lore Entry** (existing, reused from spec 012): Gains Items as a valid in-text link target alongside lore entries and actors; no other change to the Lore Entry entity itself.
- **World Member / World** (existing, reused from spec 010): Supplies the pool of assignable subjects for an Item's ownership block and the "DM" authorization concept (Owner or GM role).
- **Item Share Link**: A stable, shareable reference to one specific Item, created by a member with Owner-level access to it, mirroring the Actor Share Link (spec 010). Carries a revoked/active state but no usage cap or expiration by default. A "Copy to World" action performed through it produces a brand-new, independent Item (with cloned effects) in a destination world of the viewer's choosing — a one-time deep copy, not an ongoing link.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A DM can author an Item with a description, an icon, and at least one structured effect (heal, damage, modifier, or attack-roll) in under 2 minutes, with the effect's type, formula, and target all recoverable exactly as authored on reload.
- **SC-002**: A DM or Actor-Editor can add an Item to an Actor's inventory and see the correct quantity reflected on the Actor's sheet within one save action, with zero duplicate rows ever created for the same Item on the same Actor.
- **SC-003**: A user can navigate from a lore entry to a linked Item in one click, and see that correlation reflected on the Item's "linked from" list without any separate manual linking action — matching the existing actor/lore correlation behavior at parity.
- **SC-004**: 100% of attempts to view or edit an Item, or to modify an Actor's inventory, by a user without sufficient permission are blocked, matching the enforcement rate already established for actors and lore entries.
- **SC-005**: 100% of attempts to save an Item effect with an empty/invalid formula, or an inventory quantity below zero, are rejected with a clear validation error, with zero silently-corrupted effects or inventory entries.
- **SC-006**: Adding a new Item Effect type in the future requires adding a new type to the existing structured schema, not redesigning the Item entity, the inventory system, or the lore in-text-link resolution.
- **SC-007**: A member with Owner-level access can generate an Item share link in 2 actions or fewer, and 100% of copy operations produce a fully independent Item in the destination world with zero live references back to the source.

## Assumptions

- The ownership-block/permission model, "DM" authorization definition (Owner or GM role), and default-Viewer-access rule for Items are reused verbatim from the actor system (spec 010) and lore system (spec 012) rather than redesigned.
- Items are otherwise world-scoped (not scene-scoped), matching the existing scope of Actors and Lore Entries. Unlike lore (spec 012, which explicitly deferred cross-world sharing), Items DO participate in a cross-world share-link-and-copy mechanism from day one, mirroring the actor pattern (spec 010, User Story 5) verbatim — per the request's explicit instruction that items follow "the same way" as actors, including sharing.
- This spec defines the Item and Item Effect *data model* (structured formulas, types, and targets) but explicitly does not implement dice-rolling, roll resolution, effect triggering, or any specific tabletop ruleset's mechanics (e.g., how STAT/MODIFIERS resolve to a number for D&D 5e). That is future work layered on top of this generic, system-agnostic foundation, consistent with the request's framing ("we will overlay system rules like dnd 5e and others on top of this"), and is expected to land as its own dedicated dice-roller/resolution spec. This spec's job is to make sure the Item Effect shape (types, formula, target, and a scaffolded trigger concept) doesn't need to be redesigned when that spec arrives.
- A "use item" action (rolling/applying a consumable's effect and auto-decrementing its quantity) is explicitly out of scope for this spec — see Clarifications. Inventory quantity changes in this spec are always explicit, manual adjustments by a user with Editor/Owner access on the Actor (per FR-010), never a side effect of "using" an item.
- Effect target resource/attribute names (e.g., "Hit Points", "STAT") are freeform strings at this layer — this spec does not introduce a controlled vocabulary or validate them against any specific ruleset's stat block. A future ruleset-specific layer is expected to define and validate that vocabulary.
- "Equipping" an item (e.g., wielding a sword vs. simply carrying it) is not modeled as a distinct state in this spec — an Actor's inventory tracks Item + quantity only. Equip/loadout state is a reasonable candidate for future work once a concrete ruleset layer exists to make "equipped" meaningful (e.g., only equipped weapons contribute to derived combat stats).
- The Items tab on the Compendium (introduced as a placeholder in spec 011) is fully implemented by this spec, following the same searchable-table-plus-row-preview UX pattern already established for the NPCs tab, per the Compendium's own stated intent to support additional real tabs without restructuring existing ones.
- Icon/image handling for Items reuses the same asset-upload infrastructure already used for actor/lore imagery where practical (storage, processing) rather than introducing a separate image pipeline; exact rendition/thumbnail sizing is an implementation decision, not specified here.
- Inventory permission is governed by the Actor's ownership block (who can edit *this Actor*), not the Item's — holding Viewer-only access to an Item does not prevent an Actor-Editor from adding that Item to their Actor's inventory, since inventory management is fundamentally an operation on the Actor, not on the Item.
