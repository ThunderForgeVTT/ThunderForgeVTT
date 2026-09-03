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
├── engine/          # optional — a Rust crate, bundled packs only
├── web/             # optional
└── seed-content/    # optional
```

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

| Key | Meaning |
|---|---|
| `id` | Stable identifier, and the directory name. |
| `title` | What a person reads in a picker. |
| `version` | The pack's own version. |
| `description` | One paragraph, shown beside the title. |
| `author`, `url`, `license` | Provenance. |
| `compatibility` | `{ "minimum", "verified", "maximum" }` — product versions. |
| `legal` | See [Legal metadata](#legal-metadata). Required, and enforced. |

`template: true` declares that the pack is a starting point rather than a
ruleset. A template is **not offered** as a system a world can be bound to.
`basic-game-system` in this directory declares it, and is the pack to copy
when starting a new one.

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

### `sheet`

The rest of the character sheet: everything that is not a score or a pool.
An ordered list of entries, each with `id`, `label`, `kind`, `slot`, `source`,
and optionally `group`.

| `kind` | What it is | Extra keys |
|---|---|---|
| `text` | Free text — an aspect, a concept, a note | |
| `number` | A single number | |
| `list` | An ordered list of strings | |
| `slots` | `count` blank slots the player names and fills | `count` |
| `track` | `of` marks, ticked or not | `of` |
| `state` | One of a set of named states, in order | `options` |

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

### Anything else

A manifest may carry keys this document does not describe. Genie's
`wishPoints`, Fate's `ladder`, Cypher's `taskResolution` and Pathfinder's
`coreCheck` are each read by that pack's own crate and by nothing else. They
are the pack's business, and shared code neither reads them nor knows they
are there.

## What a bundled pack may contribute

A bundled pack may carry `server/`, a Rust crate that submits a
`SystemContribution` through `inventory`. Nothing collects it by name — the
server discovers what is linked (FR-029), and
`scripts/check-system-registry.mjs` fails the build if a system identifier
appears anywhere in shared server code.

```rust
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        ability_data: Some(validate_abilities),
        rules: Some(build_rules),
        ..SystemContribution::new(SYSTEM_ID)
    }
}
```

| Field | What it contributes |
|---|---|
| `id` | Must equal the manifest's `id`. |
| `ability_data`, `resource_data`, `proficiency_data`, `trait_data`, `spell_data` | Validate one slot of an actor's stored data. |
| `rules` | The system's **derived** values — see below. |

**Every field beyond `id` is optional, and absence is a fact about the
ruleset rather than an omission.** Genie has no spellcasting and therefore no
`spell_data`; Fate Core declares no abilities at all; a pack that computes
nothing has no `rules`.

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

### The one line outside your directory

SC-004 says adding a system touches only that system's own pack directory,
and there is exactly one honest exception. A statically linked Rust crate
that nothing references is never linked, and its `inventory` submissions
vanish with it — measured, not assumed. So a bundled pack with a `server/`
crate needs:

- one `use <pack> as _;` line in `src/server/src/system_packs.rs`, and
- one dependency in `src/server/Cargo.toml`.

Both are build-graph facts: they say a crate exists and should be linked, and
say nothing about what it contains, so they cannot drift out of step with
your pack the way a validator list can. **A pack with no `server/` crate
needs neither** — drop the directory in and the product offers it.

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
