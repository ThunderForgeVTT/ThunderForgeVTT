# Contract: the D&D Beyond PDF to 5e mapping

The mapping has two parts:

- a declaration in `packs/systems/dnd5e/system.json` under `sheetImport`;
- the `refine` hook in `packs/systems/dnd5e/server/src/sheet_import.rs`,
  which covers what data cannot express.

The reader is `ddb-pdf` in `packs/systems/dnd5e/sheet/`.

## Declaration shape (shared by every system)

```jsonc
"sheetImport": {
  "readers": ["ddb-pdf"],
  "fields": {                                   // neutral path -> "dataType.field"
    "abilities.str": "ability_data.strength",
    "...": "..."
  },
  "content": {                                  // neutral content kind -> where it lands
    "spell":         { "ability": "spell" },
    "feat":          { "ability": "feat" },
    "feature":       { "ability": "feature" },
    "species_trait": { "ability": "species_trait" },
    "item":          { "item": true },
    "attack":        { "refine": true }         // the hook decides item vs ability
  },
  "derived": ["skill.*", "save.*", "passive.*", "initiative", "proficiency_bonus"],
  "playState": [
    "resource_data.current_hp", "resource_data.temporary_hp",
    "resource_data.hit_dice_pools[].used", "resource_data.death_save_successes",
    "resource_data.death_save_failures", "spell_data.spell_slots_used",
    "spell_data.pact_slots.used", "trait_data.inspiration",
    "trait_data.luck_points_used", "links.uses_used"
  ],
  "ignore": ["identity.player_name"],           // read, shown as ignored, kept nowhere
  "notes": "trait_data.notes"                   // where `unmapped` values are appended, labelled
}
```

The host validates this block when it loads the pack:

- every target must name a declared data type and field;
- every `ability` must name a vocabulary type;
- `derived` and `playState` entries must parse as paths.

A pack with a malformed block fails to load, and that failure is a test.

## Field table: 5e

| D&D Beyond label (anchor) | Neutral path | 5e target | Notes |
| --- | --- | --- | --- |
| CHARACTER NAME | `identity.name` | actor `label` | Shown, never used for identity or authorization |
| PLAYER NAME | `identity.player_name` | — (ignored) | Read and shown as ignored. Grants nothing |
| CLASS & LEVEL | `classes[]` | `trait_data.classes`, `level`, `class`, `subclass` | "Fighter 3 / Wizard 2" gives two entries. A class with no number is Uncertain. Refine sets `level` = Σ, and `class`/`subclass` from the first |
| SPECIES / RACE | `identity.species` | `trait_data.race` | |
| BACKGROUND | `identity.background` | `trait_data.background` | Also a content `feat` if the background grants one by name |
| ALIGNMENT | `identity.alignment` | `trait_data.alignment` | |
| SIZE | `identity.size` | `trait_data.size` | Unread when the export omits it |
| EXPERIENCE POINTS | `identity.xp` | `trait_data.experience` | "Milestone" is Read, with value null |
| STR … CHA score boxes | `abilities.*` | `ability_data.strength` … `charisma` | The modifier printed beside each score is cross-checked against `floor((s-10)/2)` |
| PROFICIENCY BONUS | `derived.proficiency_bonus` | — (cross-check) | Checked against the level |
| SAVING THROWS marks | `proficiencies.saves` | `proficiency_data.saving_throw_proficiencies` | The printed totals are cross-checks |
| SKILLS marks | `proficiencies.skills` | `skill_proficiencies` / `skill_expertise` | The glyph table maps marks to none, proficient or expertise. "half" (Jack of All Trades) is not held; it becomes an unmapped note, and the feature arrives as content |
| PASSIVE WISDOM (PERCEPTION) etc. | `derived.passive.*` | — (cross-check) | |
| ARMOR CLASS | `defences.armour_class` | `ability_data.armor_class` | Stored as the sheet prints it. The pack does not derive AC today |
| INITIATIVE | `derived.initiative` | — (cross-check) | |
| SPEED | `movement.speeds.walk` | `trait_data.speed_walk` | Other speeds come from FEATURES & TRAITS text, as Uncertain |
| HIT POINT MAXIMUM | `resources.hp_max` | `resource_data.max_hp` | |
| CURRENT HIT POINTS | `resources.hp_current` | `resource_data.current_hp` | Play state. The first import writes it, and a re-import keeps the table's value |
| TEMPORARY HIT POINTS | `resources.hp_temp` | `resource_data.temporary_hp` | Play state |
| HIT DICE | `resources.hit_dice[]` | `resource_data.hit_dice_pools` | Refine writes `hit_dice` ("3d10, 2d6") and `hit_dice_used` |
| DEATH SAVES | `resources.death_saves` | `death_save_successes` / `failures` | Play state |
| ADDITIONAL SENSES | `movement.senses` | `trait_data.darkvision`, `blindsight`, … | "Darkvision 60 ft." → 60 |
| DEFENSES | `defences.*` | `trait_data.resistances`, `immunities`, `vulnerabilities`, `condition_immunities` | A Warforged's "Poison (resistance)" is read here |
| PROFICIENCIES & LANGUAGES | `proficiencies.armour`, `weapons`, `tools`, `languages` | `proficiency_data.armor`, `weapons`, `tools`, `languages` | |
| CP SP EP GP PP | `resources.coins` | `resource_data.coins` | |
| HEROIC INSPIRATION | `resources.inspiration` | `trait_data.inspiration` | Play state |
| SPELLCASTING CLASS / ABILITY / SAVE DC / ATTACK BONUS | `spellcasting[]` | `spell_data.spellcasting_classes`, and the first into `spellcasting_ability`, `spell_save_dc`, `spell_attack_bonus` | One block per casting class |
| SPELL SLOTS (per level) | `spellcasting[].slots` | `spell_data.spell_slots.level_N` | A Warlock's slots go to `pact_slots` |
| Spell rows (name, prepared mark, level, source) | `content[kind=spell]` | linked ability, `prepared`, `granted_by` | Cantrips are grade 0 |
| FEATURES & TRAITS headings | `content[kind=feature \| species_trait \| feat]` | linked ability, `granted_by`, `uses_max`, `recharge` | "Second Wind • 1 / Short Rest" sets uses 1 and recharge `short_rest` |
| EQUIPMENT rows (name, qty, weight, equipped mark) | `content[kind=item]` | linked item, `quantity`, `equipped`, `attuned`, `world_items.weight` | |
| ATTACKS rows (name, hit, damage, notes) | `content[kind=attack]` | refine: a matching equipment item gains spec 046's attack fields; otherwise an ability with `attack_roll` and `damage` effects | The way `stat_blocks.rs` maps a monster's attack |
| APPEARANCE, PERSONALITY TRAITS, IDEALS, BONDS, FLAWS, BACKSTORY, ALLIES | `persona.*` | `trait_data.age` … `backstory`, `allies_and_organizations` | |
| anything else read | `notes[]` | `trait_data.notes` (appended, headed "From the imported sheet") | Reported as unmapped in the review (FR-013) |

## Refine hook rules (5e)

1. `level` = Σ `classes[].level`. If the sheet's total and the sum differ,
   the class line is Uncertain and the reason shows both numbers. A
   multiclass level is never the sum of the printed text (spec edge case).
2. The hit-dice pools are folded into the legacy string, ordered by die
   size, largest first.
3. For each attack row, the hook looks for an equipment item with the same
   normalised name. If it finds one, it attaches `attack` fields to that
   content change. If not, it emits an ability content change with effects.
   Either way, the to-hit and damage numbers the sheet printed are kept as
   cross-checks against the pack's own attack maths, where the pack can
   compute them.
4. A spell that appears in two casting classes' lists is one content change,
   with `granted_by` naming both.

## Recognition and refusals

- **Recognised** when page 1 has "CHARACTER NAME" and "CLASS & LEVEL", and
  some page has "FEATURES & TRAITS". A spell page is optional, so a
  non-caster's export is recognised.
- **Not recognised**: a refusal with "This does not look like a D&D Beyond
  character sheet." A sheet from another game, a book, or a blank page all
  end here (FR-004).
- **Recognised, but a required block is missing** (no ability scores): a
  refusal with the missing block named. Nothing is applied in part.

## Fixture expectations (generated; R17)

| Fixture | Asserts |
| --- | --- |
| `fighter-5.pdf` | Every table row above Read, or Unread where the export omits it. No spell page, and no spell content |
| `fighter3-wizard2.pdf` | Two classes, level 5. `hit_dice_pools` d10×3 and d6×2. One spellcasting class. Proficiency bonus +3 is cross-checked |
| `fighter3-wizard2-l6.pdf` | The same character at level 6 (Wizard 3), for the re-import diff |
| `cleric-7.pdf` | Prepared marks are read, a domain spell carries `granted_by`, and slots go up to level 4 |
| `rogue-4.pdf` | Expertise is read from the glyph. Sneak Attack is a feature with no uses |
| `warforged-defences.pdf` | Poison lands in `resistances`. "Immune to disease" has no field and becomes an unmapped note |
| `uncertain-mark.pdf` | A smudged proficiency mark is Uncertain with a reason, and nothing is invented |
| `not-a-ddb-sheet.pdf` | Not recognised |
