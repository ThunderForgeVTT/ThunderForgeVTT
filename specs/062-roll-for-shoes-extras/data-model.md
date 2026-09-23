# Data Model: Roll for Shoes Extras

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Date**: 2026-09-23

Three places hold state: a new per-world row, two additions to the character's
existing JSON, and two manifest declarations. Nothing else changes.

---

## 1. World settings — `world_roll_for_shoes_settings`

A new table owned by `roll-for-shoes-server` (research D1, ADR-063). One row per
world, written on first change. **A world with no row reads as every default**,
so nothing seeds a row at world creation and an untouched world is byte-identical
to one that existed before this feature.

| Column | Type | Default | Meaning |
| --- | --- | --- | --- |
| `world_id` | `Uuid` PK, FK → `worlds(id)` ON DELETE CASCADE | — | The world these settings belong to |
| `difficulty_mode` | `Varchar` NOT NULL | `'free'` | `free` \| `rolled` \| `target` (FR-008) |
| `tie_succeeds` | `Bool` NOT NULL | `false` | On, a tie is a success and awards no XP (FR-015–018) |
| `statuses_enabled` | `Bool` NOT NULL | `false` | On, characters may hold statuses (FR-019) |
| `skill_slots_enabled` | `Bool` NOT NULL | `false` | On, per-level caps apply to gaining a skill (FR-028) |
| `starting_skills` | `Jsonb` NOT NULL | `'[]'` | Empty means "the core default, `Do Anything 1`" (FR-037) |
| `created_at` / `updated_at` | `Timestamptz` NOT NULL | `now()` | As every other table |
| `updated_by` | `Uuid` NULL | — | Who last changed them |

**Why five settings in one row rather than five rows or five columns on
`worlds`**: they are one thing a Game Master configures in one place, they are
read together on every sheet mount, and one row means one gated mutation and one
`PackSurface` entry instead of five (research D1).

**Validation**

- `difficulty_mode` ∈ {`free`, `rolled`, `target`}; anything else is refused.
- `starting_skills` is an array of `{ name: string, level: integer ≥ 1 }`, names
  non-empty after trimming. An empty array is legal and means the core default.
- Every field is independent. Writing one never derives or clears another
  (FR-002) — the mutation takes all five and the panel sends what it has read
  back with one field changed.

**`except_tables`**: `^world_roll_for_shoes_` joins `^world_genie_.*` in the root
`diesel.toml` and the table name joins the list in `src/server/diesel.toml`, so
`diesel print_schema` never re-adds it to the server's schema.

---

## 2. Character state — additions to existing JSON

No new actor table. Both additions go in slots `world_actors` already has and
`validators.rs` already inspects.

### 2a. Statuses — `trait_data.statuses`

```
statuses: [ { id: string, name: string, modifier: integer } ]
```

| Field | Rule |
| --- | --- |
| `id` | Unique within the character |
| `name` | Non-empty after trimming. A label the table writes — the system ships no list (FR-020) |
| `modifier` | A signed integer. May be negative, may be zero |

Absent or empty means no statuses, which is what every existing character has.

**Validator rules** (new, alongside T1–T11): ids unique, names non-empty,
`modifier` an integer. Nothing caps the count or the magnitude — a table that
wants a −100 status may have one.

**What statuses never touch**: the count of dice rolled, and the faces those dice
show. They are applied once, flat, to the *total* (FR-021, FR-022). See §4.

### 2b. Bought slots — `resource_data.boughtSlots`

```
boughtSlots: { "<level>": integer ≥ 0 }
```

A map from skill level to how many extra slots this character has purchased at
that level. Absent or empty means none bought, which is what every existing
character has.

**Validator rule** (new): keys parse as integers ≥ 1, values are integers ≥ 0.

**Deliberately not validated**: that the character's skills fit within the caps.
Enabling skill slots in a world whose characters already exceed them must not
make those characters unstorable (FR-036). The cap governs *gaining* a skill —
a rule about a transition, not about a stored shape (research D6).

### The XP ledger

`resource_data.xp` is unchanged and remains the single balance. It is now spent
two ways — buying a die into a six (existing) and buying a slot (new, at twice
the level, FR-031) — so both must debit the same number and neither may proceed
on an insufficient balance. This is the one place the two Extras touch (FR-035).

---

## 3. Manifest — `system.json`

Two declarations, both additive, both accepted by the existing
`deny_unknown_fields` parser only if the keys are ones it names.

- **`data_types.trait_data.properties.statuses`** — the array shape above, so the
  slot is declared like `skills` already is. `required` stays `[]`.
- **`data_types.resource_data.properties.boughtSlots`** — the map shape above.
  `required` stays `[]`.
- **`startingSkills`** replaces the existing pack-custom `startingSkill`, and is
  an array so a world's override and the pack's default have the same shape. The
  pack's default is `[{ "name": "Do Anything", "level": 1 }]`, which is the core
  rule restated, not changed.

`startingSkill` → `startingSkills` is read by the pack's own crate and by nothing
else (`packs/systems/README.md`, "Anything else"), so the rename is contained.

---

## 4. Derived, never stored: the shape of a resolved roll

The settings and the character's statuses come together in one pure function in
`game.ts`. It stores nothing; it is listed here because it is the entity every
requirement about a roll actually describes, and because its **field order is the
decision** (research D3).

```
resolve({ faces, statuses, opposition, tieSucceeds }) →
  { sum, modifier, total, verdict, xpAwarded }
```

| Step | Reads | Requirement |
| --- | --- | --- |
| 1. `sum` | `faces` | The dice, as rolled and stored |
| 2. `modifier` | `statuses` | Their signed modifiers, summed (FR-021) |
| 3. `total` | `sum + modifier` | Applied flat, once (FR-022) |
| 4. opposition | free number, rolled GM dice, or a band target | FR-008–014 |
| 5. `verdict` | `total` vs opposition, under `tieSucceeds` | FR-015–017 |
| 6. `xpAwarded` | `verdict`, and the tie rule's suppression | FR-018 |

**`isAdvancement(faces, bought)` is not in this chain and never receives a
status.** A function that cannot see statuses cannot be affected by them, which
is how FR-023 — a status can neither create nor destroy an advancement — is made
structural rather than remembered (research D4).

**Game Master dice are a separate roll.** A `rolled` difficulty is its own
`rollDice` call with a `(BAND)d6` binding, so its faces are never in the `faces`
array step 6 reads (research D7, FR-014).

---

## 5. Starting skills, and the read-time default

`skillsOf(traitData)` today returns `Do Anything 1` when nothing is stored — a
read-time default, not a seeded value. That is why FR-039 ("changing a world's
starting skills must not alter an existing character") needs care: a character
that has never been saved has nothing of its own to show instead.

**The model**: a character's skills are resolved from the world's starting skills
**at the moment the character first stores anything**, and are its own from then
on. Before that it has no stored skills and reads as the world's current set,
which is the honest description of a character nobody has opened. FR-039
therefore protects a character whose skills have been stored — a narrowing
recorded in research D5 rather than assumed here.

---

## 6. Relationships

```
worlds ──1:0..1──> world_roll_for_shoes_settings     (pack-owned, CASCADE)
  │
  └──1:N──> world_actors
              ├─ trait_data.skills      (existing)
              ├─ trait_data.statuses    (new, §2a)
              ├─ resource_data.xp       (existing, now spent two ways)
              └─ resource_data.boughtSlots (new, §2b)
```

The settings row and the actors are related only through the world. Changing a
setting writes no actor, and writing an actor reads no setting — the settings are
consulted at the moment of a roll or an advancement, in the browser, not baked
into stored state. That is what keeps every Extra reversible: turn one off and
the characters are unchanged.
