# System packs

A pack in this directory is a **game system**: what a ruleset tracks about a
character, what its numbers mean, and — for a bundled pack — what it computes.

This is the author-facing contract (FR-015). Everything a pack may declare is
described here. You should not have to read the application's source to write
one, and if you find yourself doing so, that is a defect in this document.

The companion contract for the other kind of pack is
[`../interface/README.md`](../interface/README.md). **A pack is a system pack
or an interface pack, never both** (FR-002), and the directory it lives in is
what decides.

## The shape of a pack

```text
packs/systems/<pack-id>/
├── system.json      # required — everything below is declared here
├── server/          # optional — a Rust crate, bundled packs only
├── sheet/           # optional — a Rust crate that reads a character sheet (spec 048)
├── web/             # optional — only with something the host mounts
├── seed-content/    # optional
└── README.md        # optional
```

That is the whole list. A pack may also keep a data file beside its manifest
when its own `server/` crate reads it by name, as 5e keeps `stat-blocks.json`.
`scripts/check-packs.mjs` runs before every commit and refuses anything else,
naming the pack and the entry.

**There is no `engine/`.** A pack extends the engine with data, not code
(ADR-062): the engine is one WebAssembly binary, nothing can load a second
crate into it in the browser, and what a system needs from it — bars,
movement, vision, turn structure — it declares in the manifest. Seven packs
once carried an engine crate; nothing ever depended on one, and spec 066
removed them.

**`web/` exists only when the host mounts something from it**: at least one
of `web/src/ActorSheet.tsx`, `web/src/StatBlocks.ts` or
`web/src/panels/<slot>.tsx`. Those three paths are the ones `apps/web` globs
at build time, and a file anywhere else is reached only if one of them
imports it. A pack with none of them has no `web/`, and its sheet is drawn
from the manifest.

**`sheet/` is a character sheet reader** (spec 048): a Rust crate with a
`wasm` feature that turns an exported sheet into the system-neutral
character `crates/thunderforge-sheet-import` defines. The server links it
natively, and `scripts/shared.mjs` builds it for the browser into
`dist/sheet-<pack-id>`, so a player's browser and the server read a sheet
with the same code. The pack's `system.json` says where each part of the
reading lands, under `sheetImport`.

`system.json` alone makes a working pack. Every bundled system renders a
usable character sheet from its manifest and nothing else (SC-012) — the base
interface pack lays out whatever a system declares, so a pack needs no
presentation code and no interface pack written for it.

## Who may contribute code

**Bundled packs only.** ADR-029 is the decision of record: packs from outside
the product are data, and executable extension is bundled-only. A pack you
install is a manifest; a pack compiled into the product may also carry a Rust
crate. This is not a limitation waiting to be lifted on a schedule — ADR-029
records the four conditions that would change the answer.

So: **everything in "What you declare" is available to any pack.** Everything
in "What a bundled pack may contribute" requires the pack to be in this
repository and in the build.

## What you declare

### Identity — required

| Key                        | Meaning                                                        |
| -------------------------- | -------------------------------------------------------------- |
| `id`                       | Stable identifier, and the directory name.                     |
| `title`                    | What a person reads in a picker.                               |
| `version`                  | The pack's own version.                                        |
| `description`              | One paragraph, shown beside the title.                         |
| `author`, `url`, `license` | Provenance, for a person reading the file. Nothing reads them. |
| `compatibility`            | `{ "minimum", "verified", "maximum" }` — product versions.     |
| `legal`                    | See [Legal metadata](#legal-metadata). Required, and enforced. |

`template: true` declares that the pack is a starting point rather than a
ruleset. A template is **not offered** as a system a world can be bound to.
`basic-game-system` in this directory declares it, and is the pack to copy
when starting a new one. It is a manifest and nothing else, which is the
smallest pack there is.

### Legal metadata — required, and enforced

```json
"legal": {
  "licenseName": "Open Gaming License 1.0a",
  "attributionText": "…",
  "requiredNotice": null,
  "disclaimer": null,
  "trademarkRestrictions": [],
  "requiredUiPlacement": null,
  "sourceUrl": null
}
```

`licenseName` and `attributionText` are required and must be non-empty. **A
manifest without compliant `legal` is refused rather than served** — the pack
does not half-load, it does not load. A pack's legal metadata must not claim a
licence it does not hold (FR-003b).

### Character data — `data_types`

What a character of this system stores, and how it is validated. Each slot is
a name — `ability_data`, `resource_data`, `trait_data`, `proficiency_data`,
`spell_data` — holding `properties` and `required`:

```json
"data_types": {
  "ability_data": {
    "description": "Genie ability scores",
    "properties": { "might": { "type": "integer", "label": "Might" } },
    "required": ["might"]
  }
}
```

Everything below that reads stored data names one of these slots in its
`slot` field, and a key within it in `source`.

### `abilities`

The scores the system tracks, keyed by identifier. `order` is **the system's
own order and is honoured** — Genie's might, cunning, spirit appear in that
order and are never alphabetised.

```json
"abilities": {
  "might": { "label": "Might", "abbreviation": "MGT", "order": 0 }
}
```

A system with no ability scores declares none. Fate Core does exactly that,
and its sheet is correct for it.

### `resources`

Pools and counters, drawn on tokens as well as on the sheet.

```json
"resources": [{
  "id": "health", "label": "Health", "kind": "bar", "order": 0,
  "allowStacking": false,
  "source": { "slot": "resourceData",
              "entries": [{ "current": "current_health", "max": "max_health" }] }
}]
```

`kind` is `bar` (has a maximum) or `counter` (does not). **An absent or empty
list means the system's tokens carry no bars at all**, which is the correct
result for a system that tracks no pools — not a gap to fill with a default.
The engine holds no built-in notion of "health".

### `movement`

```json
"movement": { "stride": { "label": "Stride", "source": "stride",
                          "default": 6, "order": 0 } }
```

Absent means the system has no movement budget. Four of the eight bundled
packs declare none, and their characters are none the worse for it.

### `vision`

How a creature sees, in the system's own units (spec 045, owner decision 2).
Every part optional; a system that declares none gets ordinary sight, which is
correct rather than a fallback.

```json
"vision": {
  "unitsPerCell": 5, "unitLabel": "ft",
  "darkvision": { "slot": "traitData", "source": "darkvision" },
  "carriedLight": {
    "bright": { "slot": "traitData", "source": "light_bright" },
    "dim":    { "slot": "traitData", "source": "light_dim" }
  }
}
```

Each distance names a `slot` and a `source`, the same pair a `sheet` entry
uses; `slot` defaults to `traitData`. A `default` applies only when the sheet
is silent, and an explicit zero is a creature that cannot see in the dark, not
an absent value.

`unitsPerCell` is what a grid square is worth — 5, for a five-foot square. It
is declared **here** because it exists nowhere else: a scene's `grid_size` is
pixels per cell, and `movement` declares "30" with the unit implicit, which
works only because nothing converts it. A distance that has to be drawn cannot
be left implicit. When `movement` needs the same answer, lift this out and
share it rather than declaring it twice.

D&D 5e declares darkvision and a carried light's reaches. Genie declares
nothing yet, and its tokens see by the default rules until it does.

### `sheet`

The rest of the character sheet: everything that is not a score or a pool.
An ordered list of entries, each with `id`, `label`, `kind`, `slot`, `source`,
and optionally `group`.

| `kind`   | What it is                                     | Extra keys |
| -------- | ---------------------------------------------- | ---------- |
| `text`   | Free text — an aspect, a concept, a note       |            |
| `number` | A single number                                |            |
| `list`   | An ordered list of strings                     |            |
| `slots`  | `count` blank slots the player names and fills | `count`    |
| `track`  | `of` marks, ticked or not                      | `of`       |
| `state`  | One of a set of named states, in order         | `options`  |

```json
"sheet": [
  { "id": "trouble", "label": "Trouble", "kind": "text",
    "slot": "traitData", "source": "trouble" },
  { "id": "stress", "label": "Stress", "kind": "track", "of": 8,
    "slot": "resourceData", "source": "stress" }
]
```

`slot` defaults to `traitData` and `source` defaults to `id`.

**An entry whose `kind` this build does not know is skipped, not guessed at.**
A pack declaring a kind from a newer product version loses that entry and
keeps the rest.

Every kind here is a **shape of value**, never a rule about one. There is no
conditional, no expression and no formula, and that is a boundary rather than
a missing feature: the moment the format can express a rule it becomes a
language, and a pack from outside the product would be running code again.

### `groups`

Parts of a sheet that belong together. Cypher's three pools each have a
current, a maximum and an edge; Fate's consequences each have a severity and
a text.

```json
"groups": [{ "id": "mightPool", "label": "Might", "headline": "mightPool" }]
```

A `sheet` entry joins a group with `"group": "<group id>"`. `headline` names
the entry that leads the group.

### `turnStructure`

```json
"turnStructure": { "rounds": true, "roundLabel": "Exchange" }
```

Whether the system counts rounds, and what it calls one. Fate counts
**exchanges**; Blades in the Dark counts nothing. **Absence means no rounds**
— the product declines to assume that every ruleset has them, and a system
with no rounds shows no round counter (SC-011).

A system may also declare what one turn affords (spec 046):

```json
"turnStructure": { "rounds": true, "roundLabel": "Round",
  "budget": { "action": 1, "bonusAction": 1, "reaction": 1,
              "movement": { "speed": "walk" } } }
```

`movement.speed` names a key of your `movement` block, and a manifest that
names one it does not declare is refused at install. A budget is shown and
spent; it never refuses anybody — a table that lets somebody overspend is
playing the game, not breaking it.

### `combat`

What a fight means in your system (spec 046). Every part is optional, and a
system that declares none of it still has initiative and turns.

```json
"combat": {
  "hitPoints": { "slot": "resourceData", "current": "current_hp",
                 "max": "max_hp", "temporary": "temporary_hp" },
  "defence":   { "slot": "abilityData", "field": "armor_class",
                 "label": "Armour Class", "abbrev": "AC" },
  "sizes": {
    "source": { "slot": "traitData", "field": "size" },
    "categories": [ { "id": "medium", "label": "Medium", "footprint": 1 },
                    { "id": "large",  "label": "Large",  "footprint": 2 } ]
  },
  "legendary": { "slot": "traitData", "field": "legendary_actions" }
}
```

- **`hitPoints`** is what damage and healing write. Temporary hit points are
  spent first, current stops at zero, and healing stops at the maximum. A
  creature taken to zero leaves the turn order until it is healed.
- **`defence`** is the number an attack's total must reach to hit.
- **`sizes`** says how many cells a creature of each size fills, per side.
  It is the creature's footprint on the grid — what it is snapped, clicked,
  moved and measured by — not how large its art is drawn (a token's `scale`
  is only that). A creature with no size recorded, or one you do not declare,
  fills one cell. Reach is never derived from size: an attack says its own.
  Validate the size field against this list by reading your manifest rather
  than keeping a second copy of it.
- **`legendary`** is where a creature's legendary actions per round are read,
  once, when it joins the turn order. The tracker shows how many remain; a
  legendary action spends its `legendaryCost` from them and refills at the
  start of the creature's own turn. One taken on its own turn is flagged, and
  one past zero is shown as a debt; neither is refused. A creature with none
  recorded, or 0, has no legendary line at all.

These are the rules the product enforces on a declaration, at install time:

| #   | Rule                                                                                                                                                                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1  | Every block is optional. A system without `hitPoints` has no damage operation; its attacks still roll, and a hit's damage is shown as a number nobody can take.             |
| M2  | `hitPoints`, `defence`, `sizes.source` and `legendary` name a `slot` and a field your `data_types` declares. A name it does not declare is refused, and the error names it. |
| M3  | Every `sizes.categories[].footprint` is at least 0.5, and every `id` is unique.                                                                                             |
| M4  | `turnStructure.budget.movement.speed` names a key of your `movement` block.                                                                                                 |
| M5  | Shared code names no system's fields. The product reads `current_hp` because 5e's manifest says so, never because the platform knows what 5e calls it.                      |

`slot` uses the manifest's own vocabulary (`resourceData`, `traitData`), as
`resources` and `vision` do.

D&D 5e declares `hitPoints` today. The rest of its block arrives with the
spec 046 phases that read it.

### `appearance`

Where a creature's race is written, so the hero builder can roll a face
narrowed to it (spec 044). It is read the way `combat.sizes.source` is.

```json
"appearance": { "race": { "source": { "slot": "traitData", "field": "race" } } }
```

- **`race.source`** names a `slot` and a field your `data_types` declares; a
  name it does not declare is refused at install, as M2 refuses one in
  `combat`. The field's free text is matched, trimmed and ignoring case,
  against the race keys and aliases in `packages/heroes/src/races.ts`
  ("High Elf" is an elf); text that matches none rolls as any race.
- The race narrows a roll only. Nothing writes it back to the sheet.
- A system with no races declares nothing, and its creatures roll as any race.

### `checks`

```json
"checks": [
  {
    "id": "strength",
    "label": "Strength",
    "group": "abilities",
    "formula": "1d20 + MODIFIER",
    "bindings": { "MODIFIER": { "from": "value", "id": "strengthMod" } }
  }
]
```

What a character can be asked to roll **from a sheet**, and how. A player on a
second screen presses a button; the sheet sends only this `id`; the server
resolves the bindings against the actor and rolls the finished formula on the
one path allowed to produce a result. See ADR-074 and ADR-044.

- `id` — yours, unique within the system. The sheet sends it and nothing else.
- `label` — what a person reads on the button.
- `group` — the set it belongs to (`"abilities"`, `"skills"`), when it is in
  one. Optional; used only to arrange the buttons.
- `formula` — `thunderforge_dice` syntax, with placeholders in capitals.
- `bindings` — placeholder → where its number comes from on the actor.
  `{ "from": "value", "id": "..." }` reads a value the system publishes, and
  makes no distinction between one the player typed and one your ruleset
  derived: which half of the sheet a number lives on is your business.

Three rules worth knowing before you write one:

- **A binding looks a number up. It does not compute one.** If your check
  needs `(score - 10) / 2`, declare the modifier as a derived value and bind
  to that. The arithmetic belongs in your pack, not in this contract.
- **A placeholder with no binding is not zero.** An unfilled sheet is the
  absence of a number, not the number nought, and the roll is refused rather
  than rolled short.
- **Declaring none is a complete answer.** A system with no sheet-initiated
  rolls offers no button, and that is a fact about the ruleset rather than an
  omission. Seven of the nine bundled packs ship this way today.

`formula` is not validated when the manifest is read — the crate that parses
manifests compiles for `wasm32` and does not depend on the dice engine. A
formula that does not parse is refused at roll time, and a test walks every
bundled pack so a shipped one cannot carry a broken formula.

**State your system's roll here or nowhere.** Six packs once described their
core roll under a key of their own — `coreCheck`, `actionRoll`,
`taskResolution`, `ladderRoll`, `skillRoll`, `manifestationRoll` — and
nothing read any of them. A roll the bindings can express is a `checks`
entry; one they cannot (the number it needs is not a value the sheet
publishes) is rolled by your own web code through the host's `rollDice`, and
is not written in the manifest at all. ADR-074's amendment records what
became of each.

### `settings`

What a table may choose about how it plays your system: a house rule on or
off, a number of something, one of a few modes. Declare each choice and the
host does the rest — it stores a world's answer, draws the control on the
world's System settings page for the Game Master, shows it read-only to
players, and tells every open client when it changes. You write no migration,
no GraphQL and no panel.

| `type`    | A world's answer is | Extra keys                              |
| --------- | ------------------- | --------------------------------------- |
| `boolean` | on or off           |                                         |
| `integer` | a whole number      | `min`, `max`                            |
| `choice`  | one of `options`    | `options`: a list of `{ value, label }` |
| `text`    | a line of text      | `maxLength` (500 when absent)           |

```json
"settings": [
  { "id": "inspiration", "label": "Heroic Inspiration",
    "description": "Show Inspiration on character sheets.",
    "type": "boolean", "default": true }
]
```

`id`, `label`, `type` and `default` are required; `description` and `order`
(lower first; ties keep the order written) are optional. A pack is refused if
two settings share an `id`, a `choice` has no `options`, or a `default` is not
a value its own declaration allows.

**A world that has never answered plays by `default`.** So does a world whose
stored answer you have since stopped allowing, and a setting you remove simply
stops being read — its rows are kept, and come back if you restore it. Changing
a `default` therefore changes every world that never chose.

Read a setting from your web code with `useWorldSystemSettings(worldId)` from
`@thunderforge/host`: `valueOf("inspiration")` is the world's answer, kept
current while the page is open, and `undefined` until the first read lands.
5e's `web/src/ActorSheet.tsx` is the worked example. Server code reads the same
value with `thunderforge_server::world_system_settings::effective_value`.

If a value needs a check the table above cannot express, give your
`SystemContribution` a `world_setting` function; it runs after the declared
check and its message is shown to the Game Master.

### `conditions`

```json
"conditions": [
  {
    "id": "poisoned",
    "label": "Poisoned",
    "description": "Disadvantage on attack rolls and ability checks.",
    "marker": { "glyph": "dot", "color": "danger" }
  }
]
```

The states a character can be in that the table needs to see at a glance. A
condition lives on the **character**, not on one token: it follows the
character from scene to scene, and is drawn on every token of theirs for
whoever may see that token.

The marker is data, never art (ADR-062). `glyph` is one of the shapes the
board can compose — `dot`, `ring`, `bar`, `cross`, `split`, `corner` — named
for what is drawn rather than what it means, and `color` is one of the
board's tokens — `danger`, `warning`, `positive`, `info`, `arcane`,
`neutral`. The meaning is yours and lives in the label. A manifest is refused
if two conditions share an `id`, a label is blank, a marker is missing, or a
glyph or colour is not on those lists. Give each condition a pairing no other
has, so two markers on one token can be told apart.

Declaring a condition is all a pack does. The host does the rest:

- A Game Master right-clicks a character's token and chooses **Conditions…**,
  which lists what you declared by your labels (`applyActorCondition`,
  `clearActorCondition`). Any member may read the list
  (`worldSystemConditions`); only a Game Master may change what a character
  is under, and not while play is paused.
- What a character is under travels on its tokens, so it reaches exactly the
  seats those tokens reach. A creature the Game Master has not shown the
  players is still on their board, nameless, and its marker is drawn there
  too; someone who is sent no token is sent no condition.
- The board draws one small badge per condition along the token's bottom
  edge, in the order you declared them.
- A stored condition your manifest no longer declares is not drawn and not
  deleted. Declare it again and the characters under it are as they were.

### Every key

This is the whole list. `scripts/check-packs.mjs` reads it from here and refuses a
manifest with a top-level key that is not in it: a key nothing reads is a
promise nobody keeps, and the place to find that out is before it ships.

<!-- manifest-keys -->

| Key                                  | Read by                                                                            |
| ------------------------------------ | ---------------------------------------------------------------------------------- |
| `id`, `title`, `version`             | The host, wherever a system is listed or chosen.                                   |
| `description`, `template`            | The host's system picker.                                                          |
| `compatibility`, `legal`             | The host, when the manifest is installed and when it is served.                    |
| `author`, `url`, `license`           | Nothing. Provenance, kept for a person reading the file.                           |
| `data_types`                         | The host, to check `combat` and `appearance` name fields that exist.               |
| `abilities`, `resources`, `movement` | The host: the values a sheet, a token and a check may read.                        |
| `sheet`, `groups`                    | The host: what the declared sheet draws, and how it is arranged.                   |
| `vision`                             | The host: the senses a token may be given.                                         |
| `turnStructure`, `combat`            | The host's combat tracker.                                                         |
| `appearance`                         | The host's hero builder.                                                           |
| `checks`                             | The host's `rollCheck`.                                                            |
| `itemProperties`                     | The host's item editor and `setItemAttack`; your `roll_facets` gives them meaning. |
| `settings`                           | The host's world settings.                                                         |
| `contentPatterns`                    | The host's book import.                                                            |
| `abilityVocabulary`, `abilityFacets` | The host's ability editor. `abilityFacets` is the older spelling.                  |
| `skills`                             | Your own rules, if they publish a value per skill (5e's do). Not the host.         |
| `conditions`                         | The host: the states a character may be in, and the marker the board draws.        |
| `damageTypes`                        | Your own validators, which check a resistance names one (5e's do).                 |
| `sheetImport`                        | The host's sheet import: where a read value lands. Your `sheet_import` refines it. |
| `wishPoints`                         | Genie's rules.                                                                     |
| `startingSkills`                     | Roll for Shoes' crate.                                                             |

<!-- /manifest-keys -->

The last two are one pack's own. A key of your own is allowed on the same
terms: your crate reads it, and it is added to this table in the change that
introduces it, naming the reader.

## What a bundled pack may contribute

A bundled pack may carry `server/`, a Rust crate that submits a
`SystemContribution` through `inventory`. Nothing collects it by name — the
server discovers what is linked (FR-029), and
`scripts/check-system-registry.mjs` fails the build if a system identifier
appears anywhere in shared server, engine or web code.

```rust
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        ability_data: Some(validate_abilities),
        rules: Some(build_rules),
        ..SystemContribution::new(SYSTEM_ID)
    }
}
```

| Field                                                                           | What it contributes                          |
| ------------------------------------------------------------------------------- | -------------------------------------------- |
| `id`                                                                            | Must equal the manifest's `id`.              |
| `ability_data`, `resource_data`, `proficiency_data`, `trait_data`, `spell_data` | Validate one slot of an actor's stored data. |
| `rules`                                                                         | The system's **derived** values — see below. |
| `adjudicate`                                                                    | What a roll **meant** — see below.           |

**Every field beyond `id` is optional, and absence is a fact about the
ruleset rather than an omission.** Genie has no spellcasting and therefore no
`spell_data`; Fate Core declares no abilities at all; a pack that computes
nothing has no `rules`.

### Root GraphQL fields, and a paused world

A pack that merges its own query, mutation or subscription types into the
schema must say, for **every root field** it adds, whether a paused world
refuses it (spec 051). Submit one `PackSurface`
(`thunderforge_server::play_pause::surface`) listing each field once:

- `gated` — world-scoped. The resolver calls
  `play_pause::gate::refuse_world_if_paused` beside its role check, never
  inside it, and before any write. Each entry carries a request document.
- `not_world_scoped` — touches no single world.
- `reads` — a read-only query, answered while paused. A query that starts
  play is `gated`.

`seed` makes, through the schema, the rows the gated documents name. The app
crate's `play_pause_surface_tests` fails on any pack root field left
unclassified, and calls every `gated` document against a paused world as its
Game Master and as a site admin, expecting `WORLD_PLAY_PAUSED`. Genie's
`server/src/session/play_pause_surface.rs` is a worked example.

### Derived values

`rules` builds a `SystemRules` implementation from the pack's own manifest —
a constructor rather than a value, so the manifest stays the authority on
tables like Genie's by-level Wish Points ladder instead of those numbers
being copied into Rust where they would need keeping in step by hand.

`derived_declarations()` says what the system derives; `derive()` computes it.
They are separate so a pack can be validated against a system without running
it, and **an identifier `derive()` returns that `derived_declarations()` did
not declare is rejected rather than silently rendered**.

**`derive` must be pure** — no I/O, no clock, no randomness. A derived value
is recomputed on every read and never stored, so an impure one shows two
viewers of the same character two different sheets.

### What a roll meant

The host rolls the dice; `adjudicate` says what the result meant. It is a
plain function:

```rust
fn adjudicate(roll: &RollFacts<'_>, context: &serde_json::Value) -> Option<RollOutcome>
```

`RollFacts` is the check's identifier, the dice that were kept and the total.
`RollOutcome` is a `verdict` and a `label`. The verdict comes from the host's
closed list — success, failure, tie, critical success, critical failure — so
the host can show and store any system's outcome without knowing the system.
The label is your wording for it. `None` means the roll was made and not
judged, which is a fact about the roll rather than an error: a Roll for Shoes
roll with nothing to beat has no verdict.

**`adjudicate` must be pure** — no I/O, no clock, no randomness. Whatever it
needs beyond the dice is gathered first and handed to it as `context`:

- For a declared [`checks`](#checks) entry rolled through the host's
  `rollCheck`, the context is the world's system settings, keyed by setting
  identifier.
- For a roll of your own, your resolver gathers the context and calls
  `thunderforge_server::graphql::mutations_roll::roll_and_settle`. Its
  closure is given the resolution and a connection **inside the transaction
  that records the roll**, and returns the outcome. Anything the outcome
  pays for is written there, so a roll and its consequence are stored
  together or not at all.

The outcome is stored with the roll and answered as `outcome` on every roll
resolution, so a client is told the verdict and never computes one. Roll for
Shoes' `server/src/roll/` is the worked example: the character's skill, the
difficulty and the table's tie rule are gathered on the server, and a failed
roll's experience is awarded in the same transaction as its verdict.

### Panels the host will mount

A pack may fill any of four **panel slots** by dropping a file at
`web/src/panels/<slot>.tsx` that default-exports a React component. There is
no registration step: `apps/web/src/panels/systemPanels.ts` globs that path at
build time, and the page owning each slot renders whatever it finds for the
world's system.

| Slot             | Where it appears                                          | Props                     |
| ---------------- | --------------------------------------------------------- | ------------------------- |
| `npc-detail`     | The actor page, below inventory and abilities, for an NPC | `NpcDetailPanelProps`     |
| `world-staging`  | The pre-session staging page, below session notes         | `WorldStagingPanelProps`  |
| `world-settings` | The world's system-settings page                          | `WorldSettingsPanelProps` |
| `dock`           | The play dock, as a tab of its own                        | `DockPanelProps`          |

The slot names and their props are declared in `apps/web/src/host/index.ts`
(`PanelSlot`, `PanelSlotProps`), and the registry is typed against them — a
`panels/dock.tsx` written for staging's props fails to compile rather than
failing at a table. **A file named for a slot that does not exist is refused**
by `scripts/check-packs.mjs`, which reads the slot list from that same
declaration and names every slot in its message.

A `dock` panel may also `export const title = "…"`, which is what its tab in
the play dock reads; without one the tab reads "Game system". A system that
fills no `dock` slot gets no tab, so a pack never has to draw an empty state.

A panel gets its props and nothing else. It reaches the server the same way
every other caller does, through `postGraphQL` from `@thunderforge/host`;
`world-settings` in particular is handed the `WorldRecord`, which carries no
per-system settings, so a pack storing its own reads them itself.

**Declare a setting before you store one.** A choice a table makes about how
it plays belongs in your manifest's [`settings`](#settings) block, which needs
no table and no panel. A pack whose settings that block cannot express owns
the table they live in (ADR-063), declares it in a migration under
`crates/thunderforge-server/migrations/`, and adds it to `except_tables` in
both `diesel.toml` files so shared schema generation leaves it alone. Roll for
Shoes' `server/src/settings/` predates the `settings` block and is the worked
example of that second route.

### What you touch outside your directory

SC-004 says adding a system touches only that system's own pack directory. A
pack that is only a manifest meets it exactly: drop the directory in and the
product offers it. A pack with a `web/` half meets it too, because the pnpm
workspace, the type-check and the host's globs all find `packs/systems/*/web`
by pattern.

A `server/` crate does not, and the list below is every file it costs. None
of them can be discovered, and each entry says why.

**Any pack with a `server/` crate:**

| File                                           | What you add                | Why it cannot be found for you                                                                                |
| ---------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml` (repository root)                 | the crate, in `members`     | Cargo builds what the workspace lists.                                                                        |
| `apps/thunderforge/Cargo.toml`                 | one dependency              | A crate the binary does not depend on is not compiled into it.                                                |
| `apps/thunderforge/src/system_packs.rs`        | one `use <pack> as _;` line | A statically linked crate nothing references is never linked, and its `inventory` submissions vanish with it. |
| `crates/thunderforge-server/Cargo.toml`        | one dev-dependency          | The server library's tests are their own binary, and link nothing on their own.                               |
| `crates/thunderforge-server/src/test_packs.rs` | one `use <pack> as _;` line | The same fact as above, for that test binary.                                                                 |

These are build-graph facts: they say a crate exists and should be linked,
and say nothing about what it contains, so they cannot drift out of step with
your pack the way a validator list can. The linker fact was measured, not
assumed — a binary depending on a submitting crate without naming a symbol
from it collected an empty set in debug and in release.
`scripts/check-packs.mjs` refuses a `server/` crate either linkage module
does not name.

`apps/thunderforge/src/system_packs.rs` also holds a test listing every bundled
system by id. Add yours to it: that list is the assertion that a deleted
`use` line fails loudly, not the mechanism that finds the pack.

**A pack that adds GraphQL root fields, as well:**

| File                                    | What you add                                | Why                                                                                   |
| --------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------- |
| `apps/thunderforge/src/schema_roots.rs` | your query, mutation and subscription types | The schema's roots are one merged Rust type, and a type has to be named to be merged. |

**A pack that owns tables (ADR-063), as well:**

| File                                               | What you add                    | Why                                                                                       |
| -------------------------------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------- |
| `crates/thunderforge-server/migrations/<stamp>_…/` | the migration that creates them | One database has one ordered migration history, and the server runs it at start.          |
| `diesel.toml` (repository root)                    | a pattern in `except_tables`    | Shared schema generation would otherwise write your tables into the server's `schema.rs`. |
| `crates/thunderforge-server/diesel.toml`           | each table in `except_tables`   | The same, for the copy of the configuration the crate's own tooling reads.                |

Genie and Roll for Shoes each touch every file in all three tables, and are
the worked examples. Moving a pack's migrations into the pack is the
remaining step ADR-063 describes; until it is taken, this is the cost.

## How to check a pack

`apps/web` owns the `@thunderforge/host` module and lists every pack in its
own `include`, so a pack's web code is type-checked as part of that program,
not its own. The real gate is:

```sh
pnpm --filter @thunderforge/web typecheck
```

A pack's `pnpm type-check` delegates to exactly that, and deliberately does
not run `tsc` against the pack alone. It used to, and it reported phantom
TS2307/TS18046/TS7006 errors for code that compiled perfectly — `@thunderforge/host`
is a tsconfig path alias, not a package on disk, so a standalone `tsc` cannot
see it. Mapping the alias into each pack drags all of `apps/web` into the
pack's program and then fails on that app's environment rather than the
pack's, which is worse than not running.

Everything else is per-pack and means what it says: `pnpm test` for the web
logic, `cargo test -p <pack>-server` for the validators, and the feature's
own e2e slice for the rest.

## How a pack is found

`/api/systems` lists this directory. There is no database row to insert, no
registration step and no restart-with-a-flag: a pack that is here is a pack
that is offered, minus any that declares `template: true` or whose manifest
cannot be read.

On a development machine, `scripts/dev.mjs` links this directory into the
server's data directory on every start, so a pack added to the repository is
offered the next time the stack comes up.

## When a pack fails

A surface a pack draws is wrapped so that a failure inside it is contained to
that surface, leaves the rest of the session usable, and names your pack
(FR-016). You will see the pack id in the message and the stack in the
browser console.

A pack that is named by a world but is not installed does not stop the world
opening. The world opens, the missing pack is named once, and the base
presentation applies.
