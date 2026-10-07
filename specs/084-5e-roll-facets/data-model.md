# Data Model: 5e Roll Facets

## Migration `2026-10-07-120000-0000_roll_facets`

`crates/thunderforge-server/migrations/2026-10-07-120000-0000_roll_facets/up.sql`

```sql
ALTER TABLE world_roll_records
    ADD COLUMN actor_id UUID NULL REFERENCES world_actors(id) ON DELETE SET NULL,
    ADD COLUMN roll_kind TEXT NULL
        CHECK (roll_kind IS NULL OR roll_kind IN ('check', 'to_hit', 'damage')),
    ADD COLUMN check_id TEXT NULL,
    ADD COLUMN facets TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN reroll_of UUID NULL REFERENCES world_roll_records(id) ON DELETE CASCADE,
    ADD COLUMN reroll_spent TEXT NULL,
    ADD CONSTRAINT world_roll_records_check_id_for_checks
        CHECK ((roll_kind = 'check') = (check_id IS NOT NULL)),
    ADD CONSTRAINT world_roll_records_reroll_pair
        CHECK ((reroll_of IS NULL) = (reroll_spent IS NULL));

-- One replacement per roll (research R8).
CREATE UNIQUE INDEX world_roll_records_reroll_of_key
    ON world_roll_records (reroll_of) WHERE reroll_of IS NOT NULL;

ALTER TABLE world_attacks
    ADD COLUMN reroll_of UUID NULL REFERENCES world_attacks(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX world_attacks_reroll_of_key
    ON world_attacks (reroll_of) WHERE reroll_of IS NOT NULL;
-- A reroll looks its attack up by the roll.
CREATE INDEX IF NOT EXISTS world_attacks_to_hit_roll_id_idx
    ON world_attacks (to_hit_roll_id);

ALTER TABLE world_items
    ADD COLUMN properties TEXT[] NOT NULL DEFAULT '{}';
```

`down.sql` drops the indexes and columns in reverse order.

Every existing row reads as before:

- no actor, no kind, no facets;
- not a reroll;
- an item with no properties.

## `world_roll_records` (existing, gains)

| Column         | Type                  | Set by                                                                                             |
| -------------- | --------------------- | -------------------------------------------------------------------------------------------------- |
| `actor_id`     | `UUID NULL`           | `rollCheck` (the actor); the attack path (the attacker token's actor; null for a lair or a marker) |
| `roll_kind`    | `TEXT NULL`           | `check`, `to_hit`, `damage`; null for `rollDice`                                                   |
| `check_id`     | `TEXT NULL`           | `rollCheck`; copied by a reroll                                                                    |
| `facets`       | `TEXT[]` default `{}` | the pack's `Shaped.facets`; a reroll copies them and appends the spend id                          |
| `reroll_of`    | `UUID NULL`, unique   | `rerollRoll`: the roll replaced                                                                    |
| `reroll_spent` | `TEXT NULL`           | `rerollRoll`: the pack's spend id                                                                  |

`formula` stores the formula as rolled, after the transform (FR-008).

A reroll's row copies these from the roll it replaces:

- `formula`, or the reshaped formula for a Luck Point;
- `bindings`;
- `visibility`, `label`, `actor_id`, `roll_kind` and `check_id`.

Its `triggered_by` is the caller, who is the original's maker.

**Rules**

- Only `roll_kind IN ('check','to_hit')` can be rerolled.
- A roll with `actor_id IS NULL` cannot be rerolled.
- Only `triggered_by = caller` can reroll, and the caller must still have
  Editor on the actor.
- The roll must be no more than `REROLL_WINDOW` (2 minutes) old, and nothing
  may have rerolled it yet.
- A spend id may appear once per chain. The chain runs from the first roll
  through each `reroll_of`.

**Models** (`crates/thunderforge-server/src/models.rs`): the new fields go
on `RollRecord` (Queryable) and `NewRollRecord` (Insertable). Every existing
`NewRollRecord { … }` literal gains `..` defaults through a
`NewRollRecord::plain(...)` constructor. The literals are in
`mutations_roll.rs`, `combat/attack.rs` and the tests.

## `world_attacks` (existing, gains)

| Column      | Type                | Meaning                                                      |
| ----------- | ------------------- | ------------------------------------------------------------ |
| `reroll_of` | `UUID NULL`, unique | the missed attack this row replaces; its `defence` is copied |

## `world_items` (existing, gains)

| Column       | Type                  | Meaning                                                               |
| ------------ | --------------------- | --------------------------------------------------------------------- |
| `properties` | `TEXT[]` default `{}` | the pack's declared item property ids; `setItemAttack` validates them |

## 5e `trait_data` (pack JSON, gains)

In `packs/systems/dnd5e/system.json` → `data_types.trait_data.properties`:

```json
"facets": {
  "type": "array",
  "items": { "type": "string", "enum": ["halfling_luck", "great_weapon_fighting", "lucky"] },
  "label": "Roll facets"
},
"luck_points_used": { "type": "integer", "min": 0, "label": "Luck Points used" }
```

`validate_trait_data` (`packs/systems/dnd5e/server/src/validators.rs`)
enforces the following:

- `facets` is an array of known ids with no duplicates.
- `luck_points_used` is a whole number of 0 or more.
- The maximum, the proficiency bonus, is derived and never stored (FR-002).
  A value above it is accepted, so a level drop does not lock the sheet, and
  it reads as zero points left.

## 5e `itemProperties` (pack manifest, new)

`system.json` top level:

```json
"itemProperties": [
  { "id": "ammunition", "label": "Ammunition" },
  { "id": "finesse", "label": "Finesse" },
  { "id": "heavy", "label": "Heavy" },
  { "id": "light", "label": "Light" },
  { "id": "loading", "label": "Loading" },
  { "id": "reach", "label": "Reach" },
  { "id": "thrown", "label": "Thrown" },
  { "id": "two_handed", "label": "Two-Handed" },
  { "id": "versatile", "label": "Versatile" }
]
```

The server reads it the way `checks_for_system` reads `checks`
(`mutations_roll_check.rs:93`): a new `item_properties_for_system(systems_dir,
system_id)` in `crates/thunderforge-server/src/combat/item_properties.rs`. A
manifest without the key declares none, so other packs need no change.

## Facet ids (owned by the 5e pack)

| Id                      | Kind      | Label                 | Recorded on a roll when                                      |
| ----------------------- | --------- | --------------------- | ------------------------------------------------------------ |
| `advantage`             | per roll  | Advantage             | the choice changed the formula                               |
| `disadvantage`          | per roll  | Disadvantage          | the choice changed the formula                               |
| `halfling_luck`         | passive   | Halfling Luck         | `r1` was added to a d20 term                                 |
| `great_weapon_fighting` | passive   | Great Weapon Fighting | `min3` was added to a damage term                            |
| `lucky`                 | sheet     | Lucky                 | never on a roll; it enables `luck_point`                     |
| `inspiration`           | spendable | Heroic Inspiration    | a reroll spent it (`reroll_spent`, and appended to `facets`) |
| `luck_point`            | spendable | Luck Point            | a reroll spent it                                            |

## State of a roll's chain

```
roll A (created) ──rerollRoll(inspiration)──▶ roll B (reroll_of=A, spent=inspiration)
                                                 │
                                                 └─rerollRoll(luck_point)──▶ roll C (reroll_of=B)
A: rerolledBy=B, can no longer be rerolled        B: rerolledBy=C        C: latest, offers until +2 min
```
