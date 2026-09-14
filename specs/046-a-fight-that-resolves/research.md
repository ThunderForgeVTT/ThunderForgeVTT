# Research: A Fight That Resolves

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-09-14

Every decision below was taken against the code as it stood on `main` at
`ec570f6`. File references are to that tree.

## What the spec got wrong about today

The spec's Context was written on 2026-09-11. Three of its claims no longer
hold, and the plan works from what is true instead.

1. **`updateActorSystemData` does emit a world event.** Since d24913d (spec
   045 FR-067) it records `EVENT_CODE_ACTOR_SHEET_CHANGED` (26) with
   `{action, actorId, dataType}` (`mutations_actor_system_data.rs:259-285`).
   The bars still do not move because `engine/world/sync/tokenStatus.ts`
   listens only for 14 and 19. The fix is a listener, not a new event.
2. **`useUpdateActorData` is used**, by Genie's sheet through
   `@thunderforge/host`. The 5e pack has no sheet writer, so for 5e the claim
   stands in effect.
3. **"A player is refused the turn by the server"** means only that
   `advanceTurn` is GM-only. No server path checks whose turn it is before a
   move, a roll or anything else.

The playtest's FINDING at `combat-5e.playtest.ts:522` repeats claim 1's stale
wording, and is corrected when phase 1 turns it into a hard check.

---

## R1. Where an attack is resolved

**Decision**: On the server, in one new mutation, `makeAttack`. The server
rolls the to-hit and the damage with the existing dice crate, reads the
target's defence, measures reach, range and line of sight, checks the turn,
writes one roll record, and creates the offer (or applies it, under
auto-apply) in the same transaction.

**Rationale**:
- Principle III puts authority at the data boundary, and a hit changes another
  user's creature. A client that rolled and reported would be trusted with
  somebody else's hit points.
- The pieces are already server-side: `thunderforge-dice` resolves with an
  injected RNG (`crates/thunderforge-dice/src/lib.rs:16`), `roll_dice_impl`
  writes `world_roll_records`, and `thunderforge-canvas-core` already judges
  geometry for movement on the server (`src/server/src/movement/mod.rs`).
- One mutation keeps the order of judgements in one place: turn → reach and
  range → line of sight → to-hit → defence → damage → offer.

**Alternatives considered**:
- *Client rolls, server applies*: the client could claim any total. Rejected.
- *Reuse `rollDice` with a target argument*: `rollDice` is a free-formula
  roller used by the dice panel; overloading it with attack semantics would
  make every free roll pay for combat checks. Kept separate; both write the
  same table.

This is a new ownership boundary (the server adjudicates an attack), so it
lands with **ADR-101, "An attack is resolved on the server, and its damage is
an offer"**.

## R2. What an attack is made of

**Decision**: An attack is an ability or item that carries an `ATTACK_ROLL`
effect and at least one `DAMAGE` effect, as abilities already model it
(`types_abilities.rs:36-41`). The ability or item gains structured fields,
all optional:

| Field | Meaning |
|---|---|
| `reach` | melee reach, in the system's units |
| `range_normal`, `range_long` | ranged distances, in the system's units |
| `needs_line_of_sight` | default `true` (decision 3) |
| `action_cost` | `action`, `bonus_action`, `reaction`, `legendary`, `free` |
| `legendary_cost` | how many legendary actions it spends (default 1) |
| `multiattack` | ordered ids of the abilities one use makes |

An attack with neither reach nor range is treated as reach of one cell and is
flagged "no reach declared" to the Game Master only.

**Rationale**: reach belongs to the attack (spec US4 scenario 4), so it lives
on the ability, not the creature. The effect table stays as it is; the new
fields describe the ability as a whole, and the existing `target TEXT` column
is left untouched (it is free text and nothing reads it).

**Alternatives considered**: reach on the effect row (one ability with two
damage effects would carry reach twice); reach on the actor (the ogre's
greatclub reaches five feet while its size is ten).

**Known gap, not closed here**: the 5e pack's `content_refine::Action`
parses `reach_feet` from prose, but the in-browser source-book reader never
runs the refinement (`crates/thunderforge-pdf/src/wasm.rs:177-269`), and
nothing turns a creature entry into abilities. Imported monsters therefore
arrive without structured attacks. The plan's playtest and e2e author their
attacks directly; importing them is spec 047's (the bestiary).

## R3. Defence

**Decision**: The pack declares its defence in a new manifest block,
`combat.defence`, naming a slot and a field (`{"slot": "ability_data" |
..., "field": "armor_class", "label": "Armour Class", "abbrev": "AC"}`). The
5e pack adds `armor_class` (integer, min 0) to one of its data types, since
today it exists only in a legacy Rust struct and the pack's web schema.
The comparison is "total ≥ defence hits".

**Rationale**: the platform holds the concept, the pack the rule (spec
Assumptions). Declaring the field keeps shared code free of `armor_class`
(`check-system-registry.mjs` fails a build that names a system's fields in
shared code).

**Alternatives considered**: a platform `defence` column on actors (every
system would carry a 5e concept); a derived AC computed in `DnD5eRules`
(armour and shields are not modelled, so it would be a fiction).

## R4. The hit-point record: linked and unlinked tokens

**Decision**:
- `tokens` gains `linked BOOLEAN NOT NULL` and `system_data JSONB NULL`.
- A **linked** token (`linked = true`) has no `system_data`; its hit points
  are its actor's `world_actor_system_data.resource_data`.
- An **unlinked copy** (`linked = false`) holds `system_data` seeded from its
  actor's `resource_data` at placement, and damage writes there.
- `tokens.health` and `tokens.max_health` are **retired**: dropped by a
  migration after any non-null values are copied into `system_data` for
  unlinked tokens. The engine's `DerivedStats.is_dead` and the legacy bar in
  `TokenPanel.tsx:398-409` go with them.
- `tokenStatus` reads `system_data` for an unlinked token and the actor's
  data for a linked one, and stops skipping actorless tokens only if they
  carry `system_data`.
- `world_actors` gains `is_unique BOOLEAN NOT NULL DEFAULT false`.

Placement defaults (FR-016): a character's token is linked; an NPC's token is
an unlinked copy unless the NPC `is_unique`. `createToken` resolves this on
the server from the actor, and in the same step fixes the loose end where an
NPC's token is typed `character` (`parse_token_kind(None)` defaults to
`Character`, `mutations_tokens.rs:642-645`): the kind is derived from
`world_actors.actor_type` when the caller gives none.

**Rationale**: two pools that can disagree is the defect (FR-015). Copies
must not need actors (FR-017, SC-008), and copies of one NPC must not share a
pool. Storing a copy's data on its own token row keeps "two hundred goblins"
a single-table read for `tokenStatus`.

**Alternatives considered**:
- *A `token_system_data` side table*: one more join per status read, for no
  benefit; a token has at most one record.
- *Keep `tokens.health` as the copy's pool*: it is a single integer and the
  pack's resource is a structure (current, max, temporary). Rejected.

This changes what a token is, so it lands with **ADR-102, "A token is its
actor, or a copy of it"**.

## R5. Damage as an operation

**Decision**: One server function, `apply_hit_point_change(token, kind,
amount, cause)`, used by taking an offer, by auto-apply, and by the Game
Master's direct "Damage / Heal" (FR-014). It:
1. locks the record it writes (the token row for a copy, the actor's
   system-data row for a linked token) with `SELECT … FOR UPDATE`, so two
   offers taken together both land (spec edge case);
2. reads the pack's declared hit-point fields (R6), spends temporary hit
   points first, bounds current at zero and healing at the maximum;
3. validates the result with the pack's validator;
4. writes, then records the event: 26 for a linked token's actor, 14 for a
   copy;
5. crosses zero in either direction → updates the combatant (R9).

The whole-blob `updateActorSystemData` remains for sheets; it is not the
damage path.

## R6. Which fields are hit points

**Decision**: The `combat` manifest block names them: `"hitPoints": {"slot":
"resourceData", "current": "current_hp", "max": "max_hp", "temporary":
"temporary_hp"}`. 5e declares it; a pack without it has no damage operation,
and its creatures show the attack roll and offer only as text ("5 damage")
that nobody can take.

**Rationale**: `resources[]` already names `current_hp`/`max_hp` for bars
(`dnd5e/system.json:963-985`), but a bar's entries are display, and an
optional "Temporary" entry is not a statement that temporary hit points are
spent first. A separate declaration says what damage does.

## R7. Who controls a creature

**Decision**: The **controllers** of a token are the users who may move it
with `moveOwnToken` today: its `owner_user_id`, and holders of Owner
permission on its actor (`mutations_tokens.rs:417-432`). A token with no
controller is controlled by the world's Game Masters. An offer is addressed
to a token and may be resolved by any of its controllers, or by a Game Master
(FR-009).

**Rationale**: the codebase has four "controller" signals (token owner, actor
Owner permission, claims, `owned_by`). Movement is the one that already
answers "who acts for this creature on the board", and reusing it means a
player who can move their token is the player who takes its hits. Claims are
not consulted directly; spec 044 FR-030 is the place that reconciles claims
with permissions.

**Known gap**: `bringPartyToScene` sets `owner_user_id` from `owned_by`, so a
GM-created character claimed by a player arrives with the GM as owner
(`mutations_party.rs:291-295`). Its offers would go to the GM. Recorded as a
task in phase 3 (set the owner from the claim when one exists).

## R8. Telling the table, and redacting per viewer

**Decision**:
- New event `EVENT_CODE_ATTACK_MADE` (29), payload `{attackId}` only, and
  `EVENT_CODE_OFFER_CHANGED` (30), payload `{offerId, tokenId}`.
- Clients refetch through `attack(id)` / `sceneAttacks(sceneId, since)`,
  which build the answer **per viewer**, following the pattern the codebase
  already uses for chat (`mutations_chat.rs:17-24`) and hidden names
  (`GraphQLToken::for_viewer`, `load_combat`'s "Unknown").
- The attacker is shown as "Unknown", with no token id, actor id, name or
  ability name, when the viewer is not a Game Master and either the
  attacker's token has `name_visible_to_players = false`, or **none of the
  viewer's controlled tokens can see the attacker's token**. The same test
  hides the target from a viewer who cannot see it.
- "Can see" is computed on the server with
  `thunderforge_canvas_core::vision::visibility_of`, from each of the
  viewer's controlled tokens in the scene, against the scene's walls and
  lights and ambient light, using the vision profiles `vision_profiles.rs`
  already resolves. A viewer with no token in the scene sees neither party.

**Rationale**: the event stream has no per-viewer filtering
(`subscriptions.rs:137`, `world_events_since.rs:94`), so anything in a
payload reaches every member. Ids-only plus a per-viewer read is the
established answer.

**Consistency note, deliberately accepted**: spec 045 hides unseen *tokens*
by not drawing them, and still sends them ("Hidden means not drawn, not
withheld", 045 spec.md:598). This spec withholds an unseen *attacker's
identity* on the server (FR-002a). A player reading network traffic can
still find a hidden token's position through the scene's token list, but
cannot learn that it attacked, or with what. Withholding tokens themselves
remains 045's out-of-scope item.

**Cost**: one visibility pass per (viewer token × attack) on read. Attacks
are read one at a time on an event, and a player controls one or a few
tokens, so this is small; `sceneAttacks` pages at 50.

## R9. Out of the fight

**Decision**: `world_combatants` gains `downed_by TEXT NULL CHECK IN
('hit_points', 'game_master')`. When a hit-point change takes a combatant's
creature to zero, the server sets `active = false, downed_by = 'hit_points'`
and records event 18. Healing above zero reactivates it only when
`downed_by = 'hit_points'`; a Game Master's manual Down (`downed_by =
'game_master'`) is never undone by healing (FR-022). A combatant is matched
to its creature through `token_id`, falling back to `actor_id` for a
combatant with no token (linked creatures only).

## R10. Size and footprint

**Decision**:
- The manifest's `combat.sizes` block generalises Genie's `sizeCategories`:
  `[{"id": "large", "label": "Large", "footprint": 2}, …]`, plus `"source":
  {"slot": "traitData", "field": "size"}`. 5e adds `size` to `trait_data`
  and declares Tiny 0.5 … Gargantuan 4. Genie's `sizeCategories` moves into
  the same block, and its hard-coded validator list reads the manifest.
- `pack_system_spec` gains the typed block and its validation, following the
  `vision` precedent.
- The server resolves each token's footprint in a new query,
  `tokenGrid(sceneId)` → `[{tokenId, footprint}]`: an unlinked copy takes its
  NPC's size, a linked token its actor's, a token with no actor or no size
  1. The web syncs it the way `sync/tokenVision.ts` syncs vision, with a new
  `SetTokenGridCommand` driving the engine's existing `set_token_grid`
  (`sdk.rs:193-200`), re-read on events 14 and 26.
- `tokens.scale` goes back to being an art multiplier on a sprite that is
  already sized to its footprint. The playtest's ogre stops using `scale = 2`
  and gets `size: large`.

**Rationale**: everything that should follow size already reads
`TokenGridBehaviour.footprint` in the engine (snapping, hit-testing, keyboard
commits, nameplates, bars, context menu); only the setter was missing.

**Out of this spec**: hex footprints stay centred on one hex (canvas-core
records the seven-hex flower as unmodelled). A Large token's collision with
walls stays judged by its centre (045's assumption); reach is not.

## R11. Distance, reach and range

**Decision**: A new canvas-core function, `footprint_distance(grid, a_pos,
a_footprint, b_pos, b_footprint) -> i32`, the minimum Chebyshev cell distance
between the two covered rectangles on square grids (5-5-5, matching
`GridSpec::cell_distance`), axial distance between centres on hex, and
Euclidean world distance ÷ cell size on gridless. Reach and range compare
`distance × units_per_cell` against the attack's figures. Adjacent is
distance 1. Units come from the manifest's `vision.unitsPerCell` (5 ft for
5e), which is also where `GridUnits` already reads them.

Line of sight for an attack (decision 3) is `is_visible` between the nearest
pair of covered cell centres, trying every pair for footprints above 1 until
one is clear: an ogre peering round a corner with one of its four squares can
see.

Out-of-reach, beyond-range and no-line-of-sight are **flags on the attack
record**, shown to the table, never refusals (clarification 1). An attack
without line of sight is not eligible for auto-apply unless its ability says
`needs_line_of_sight = false`.

**Rationale**: the same function on both sides would matter if the engine
enforced reach; here the engine only previews it (the warning before rolling,
FR-033), so the server's answer is authoritative and the web asks the server
through `previewAttack` rather than reimplementing distance.

## R12. Turn order

**Decision**: While a combat is running in a scene, a player's move
(`moveOwnToken`), a queued offline move (`reconcileQueuedChanges`), and a
player's `makeAttack` are refused when the acting token is a combatant and
not the active combatant, with "It is <label>'s turn", where the label obeys
the same "Unknown" rule as the tracker. Exceptions: `makeAttack` with
`actionCost: reaction`; any Game Master; and tokens that are not combatants
(a player exploring elsewhere on the scene is not in the fight).

**Rationale**: the playtest's FINDING at line 606 is a player moving on the
ogre's turn. The two player move paths are the ones research found; the
offline path has no wall judgement either, which is 045's to close and is
noted there.

## R13. The economy of a round

**Decision**: A new table, `world_combatant_budgets`, one row per combatant:
`action_spent`, `bonus_action_spent`, `reaction_spent` (integers, so an
overspend is visible as 2), `movement_spent` (world units), and
`legendary_remaining`. The pack declares what a turn affords in
`turnStructure.budget` (5e: one action, one bonus action, one reaction,
movement = the `walk` speed from `movement`). `advanceTurn` resets the new
active combatant's action, bonus action, movement and reaction, and refills
its legendary actions. `makeAttack` spends by `action_cost`; `moveOwnToken`
spends the path's cost (`movement_budget::cost_path`, which exists and has no
caller today). Nothing is refused (decision 2); the tracker shows what
remains, and negative remaining is shown as a debt.

Multiattack: one `makeAttack` on an ability with `multiattack` makes each
named attack in order against the same target (or targets given per attack)
and spends one action.

## R14. Legendary actions and the lair

**Decision**: A combatant's legendary actions per round come from the pack's
declared field (`combat.legendary: {"slot": "traitData", "field":
"legendary_actions"}`), read when it is added. `makeAttack` with an
ability whose `action_cost = legendary` spends `legendary_cost` from the
pool; it is flagged, not refused, when made on the creature's own turn. A
lair is a combatant with `kind = 'lair'`, no token or actor, initiative fixed
at 20 and losing ties (tiebreak `-1`); the Game Master adds it from the
tracker.

## R15. Auto-apply

**Decision**: `worlds.auto_apply_npc_damage BOOLEAN NOT NULL DEFAULT false`,
set by a GM-only mutation following
`update_world_genie_resource_carryover_impl`; and `world_combats.auto_apply
BOOLEAN NULL`, where null means "use the world's". An attack is auto-applied
when all hold: the effective setting is on; the target token has no
controller other than Game Masters (R7); the attack named a target; it hit;
and it had line of sight or its ability does not need it. Damage to a
creature any player controls is always an offer.

## R16. Offline attacks

**Decision**: An attack made offline is queued as a new intent kind in the
existing offline queue and resolved by `makeAttack`'s logic at replay,
against the state the server then holds (spec edge case). A turn-order
refusal at replay spends nothing and is shown to the player as a refused
queued action, the way queued conflicts are shown today.

## R17. Performance

**Decision and targets**:
- A scene of 200 unlinked copies: `tokenStatus` and `tokenGrid` each answer
  in one query over `tokens` with one join to `world_actors`; the web
  dispatches one engine command per token, as `tokenVision` already does.
- **Target**: with 200 copies, scene load to bars-and-footprints-drawn adds
  under 500 ms to the same scene without them, and the frame rate matches
  `engine-status-limits.spec.ts`' 400-token baseline (60 fps on the reference
  host, `marketing/engine-status-capacity.json`).
- An attack's read-side visibility pass is bounded by the viewer's controlled
  tokens × 1 attack; `previewAttack` is one distance and one line-of-sight
  test.
- Measured by extending `engine-status-limits.spec.ts` with a 200-copy level
  that places copies through `createToken` (not one shared actor, as today).

## Loose ends found, not taken into this spec

- **`create_world_token` / `upsert_world_token` check no world membership**
  (`mutations_world_tokens.rs:15-200`). No client calls them. Security fix
  or removal; spun off as its own task.
- **`TokenPanel.tsx:217-218` sends `health`/`maxHealth` to `createToken`**,
  whose input declares neither. Removed with `tokens.health` in phase 3.
- **No foreign key from `tokens.actor_id`** to `world_actors`: deleting an
  actor strands its tokens. An unlinked copy now depends on its actor for its
  size and name; phase 3 adds `ON DELETE SET NULL` and treats a copy with no
  actor as a creature with its own `system_data`, size 1.
