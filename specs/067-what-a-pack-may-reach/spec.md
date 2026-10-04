# Feature Specification: What a Pack May Reach

**Feature Branch**: `067-what-a-pack-may-reach`
**Created**: 2026-10-04
**Status**: Draft
**Input**: Spec 066 made every pack one shape and listed, under "What this spec does not do", four places where that shape stops short: no generic per-world settings, four panel slots named for one pack's needs, no pack-contributed rules beyond derived values and sheet checks, and no engine declaration beyond appearance. The owner's direction on 2026-10-04: close all four.

## Why

A pack today can describe a ruleset's data and draw its sheet. The moment it
wants a table to *choose* something, it needs a migration, two `diesel.toml`
entries, a GraphQL root and a panel — Roll for Shoes paid that for five
checkboxes (ADR-108). The moment it wants a place in the play dock, it fills
a slot called `clocks` whether or not it has clocks — Roll for Shoes does,
with a difficulty picker. Seven manifests declare how their system rolls
under seven different keys, and nothing shared reads any of them. Genie
declares conditions nothing draws.

Each of those is the same fault: the pack had something to say and the host
had no word for it, so the pack either paid full price or said it to nobody.

## Decisions already made

These were settled with the owner on 2026-10-04 and are not open:

- **All four gaps are in scope**, as four stories that ship independently, in
  the order below.
- **Settings are one shared table**, rows keyed by world, system and key, the
  value JSON. A pack declares its settings; no pack writes a migration for
  one. Genie and Roll for Shoes keep the tables they have until someone
  chooses to move them.
- **ADR-062 and ADR-029 stand.** A pack extends the engine with data, and
  nothing is loaded at runtime.
- **Roll for Shoes and 5e are the systems that must be field-test ready.**
  Every story is proven on one of them, not on a pack nobody plays.

## User Scenarios & Testing

### User Story 1 — A table chooses how it plays, and the pack wrote no plumbing (Priority: P1)

A Game Master opens the world's System settings and finds the options the
ruleset offers, with the ruleset's own labels and defaults. They change one.
Every member's session plays by it. The pack's author added a block to
`system.json` and read the value where the rule is applied — no migration,
no GraphQL type, no panel.

**Why this priority**: it is the gap a pack author hits first and pays most
for, and ADR-108 recorded its threshold as already met.

**Independent test**: declare one setting in a full-time system's manifest;
with no other server change, the settings page shows it, a Game Master
changes it, a player's session sees the new value, and a non-GM is refused.

**Acceptance scenarios**:

1. **Given** a system whose manifest declares settings, **when** any member
   opens a world of that system, **then** each setting reads as its declared
   default and no row exists for it.
2. **Given** a Game Master, **when** they change a setting, **then** the value
   is stored, a change is recorded with who and when, and every connected
   member's view of the setting updates without a reload.
3. **Given** a player, **when** they try to change a setting, **then** they
   are refused and nothing is stored.
4. **Given** a value the declaration does not allow — wrong type, out of
   bounds, not one of the options, or refused by the pack's own validator —
   **when** it is submitted, **then** it is refused with a message naming
   the setting, never coerced.
5. **Given** a stored row whose key the manifest no longer declares, **when**
   settings are read, **then** the row is ignored and left in place.
6. **Given** a world that changes system and changes back, **when** settings
   are read, **then** the first system's values are as they were left.

### User Story 2 — A pack's panel sits in a place named for where it is, not for what Genie put there (Priority: P2)

A Roll for Shoes Game Master opens the play dock and finds a tab called
"Table", not one called "Clocks" holding a difficulty picker. A pack author
who names a panel file something the host does not mount is told at commit
time, not left wondering why it never appears.

**Independent test**: Roll for Shoes' dock tab carries the pack's own title;
Genie's still reads "Clocks"; a panel file named for no slot fails
`check-packs`.

**Acceptance scenarios**:

1. **Given** a pack that fills the dock slot and states a title, **when** the
   dock renders, **then** the tab carries that title.
2. **Given** a system that fills no dock slot, **when** the dock renders,
   **then** there is no empty tab for it.
3. **Given** a file in a pack's `panels/` whose name is not a slot, **when**
   a commit is attempted, **then** it is refused and the slots are listed.

### User Story 3 — How a system rolls is said once, in a word the host reads (Priority: P3)

Every system states its core roll. Today seven do so under seven keys the
host never reads, and one — 5e — uses `checks`, which it does. After this
story a manifest states a roll in one shared vocabulary, the sheet offers
it, and a manifest key no shared code reads is refused.

A pack whose rule cannot be data — the outcome of a roll depends on the
world's settings, or on the dice shown — contributes it through its server
crate, on the same contribution it already submits.

**Independent test**: `check-packs` fails a manifest carrying a top-level key
outside the contract; a Roll for Shoes roll's outcome is decided by the
server using the world's tie setting, not by the browser.

### User Story 4 — A condition a system declares is one the board can show (Priority: P4)

A system declares its conditions — id, label, description, a marker. A Game
Master puts one on a token; everyone who may see that token sees the
marker. The engine learns nothing about any ruleset: it is handed
identifier and marker, as ADR-062 requires.

**Independent test**: with Genie's three declared conditions and a set
declared for 5e, a condition applied to a token is drawn for a second
client and is absent for a client who cannot see the token.

### Edge cases

- A setting's declaration changes type between versions: a stored value that
  no longer validates reads as the default, and the row is left in place.
- Two Game Masters change the same setting: last write wins, both changes
  are recorded.
- A system with no `settings` block: the settings page shows no generic
  section, and the query returns an empty list rather than an error.
- A world is paused: a setting change is refused, as every other gated
  write is.

## Requirements

### Story 1 — settings

- **FR-001**: A manifest MAY carry a `settings` array. Each entry declares
  `id`, `label`, `type` (`boolean`, `integer`, `choice` or `text`) and
  `default`; and MAY declare `description`, `min`/`max` (integer),
  `options` (choice: value and label), `maxLength` (text) and `order`.
- **FR-002**: Manifest validation MUST refuse a `settings` block whose ids
  repeat, whose default does not satisfy its own declaration, or whose type
  is unknown. Bundled manifests are held to it by the existing walk.
- **FR-003**: Values live in one table, `world_system_settings`, keyed by
  `(world_id, system_id, key)` with a JSON value. A missing row is the
  declared default. Nothing is seeded.
- **FR-004**: Every change appends to `world_system_setting_changes` — world,
  system, key, old value, new value, who, when. It is never updated or
  deleted except by the world's own deletion.
- **FR-005**: Any member of a world MAY read its active system's settings:
  the declaration and the effective value together, so a client needs no
  second source to render a form.
- **FR-006**: Only a Game Master MAY write, one setting per call, and a
  paused world refuses the write.
- **FR-007**: A write is validated against the declaration, then by the
  pack's own validator if its contribution supplies one. A refusal names the
  setting.
- **FR-008**: A row whose key is undeclared, or whose value no longer
  validates, is inert: it reads as absent and is never deleted by a read.
- **FR-009**: A change is announced to the world's connected clients through
  the existing world-event backplane; clients do not poll.
- **FR-010**: The world's System settings page renders the declared settings
  generically, above any `world-settings` panel the pack supplies.
- **FR-011**: Server code and pack web code each have one function to read a
  world's effective setting. Neither touches the table directly.
- **FR-012**: No shared source names a system. `check-system-registry` stays
  green with `KNOWN` empty.
- **FR-013**: The first declared setting belongs to a full-time system and is
  read by that system's code. A setting nothing reads is not shipped.

### Story 2 — panel slots

- **FR-020**: The slot the play dock mounts is named `dock`. A pack's dock
  panel module MAY export a `title`; the dock tab uses it and falls back to
  the system's title.
- **FR-021**: The dock shows no tab for a system that fills no dock slot.
- **FR-022**: The slot vocabulary stays a closed, typed list in
  `@thunderforge/host`. `check-packs` refuses a `panels/` file whose name is
  not in it, reading the list from the host rather than repeating it.
- **FR-023**: The pack contract lists every slot, where it mounts and what
  it is handed.

### Story 3 — rules

- **FR-030**: The manifest keys the host reads are a written list in the
  pack contract. `check-packs` refuses a top-level key outside it.
- **FR-031**: The seven single-pack roll keys are restated as `checks`
  entries where the dice grammar can express them, and removed where it
  cannot, with the reason recorded in that pack's research digest.
- **FR-032**: A pack's contribution MAY supply a roll adjudicator: given a
  check's id, the dice result and the world's effective settings, it returns
  an outcome the host stores with the roll and shows to the table. It is
  pure, as `derive` is.
- **FR-033**: Roll for Shoes' success-or-failure, including its tie rule, is
  decided by that adjudicator on the server.

### Story 4 — conditions

- **FR-040**: A manifest MAY carry a `conditions` array: `id`, `label`,
  `description`, and a marker (a named glyph and a colour token).
- **FR-041**: A Game Master MAY apply or clear a declared condition on a
  token. The server refuses an undeclared one.
- **FR-042**: Conditions on a token reach exactly the clients that may see
  the token, through the world store, and the engine draws the marker from
  identifier and marker alone.
- **FR-043**: Genie's existing `conditions` map is restated in this shape;
  5e declares its own.

## Success Criteria

- **SC-001**: Adding a setting to a pack touches that pack's directory and
  nothing else, and the commit that does so passes every check.
- **SC-002**: A setting changed by a Game Master is visible in a second
  member's session within two seconds, with no reload.
- **SC-003**: No bundled manifest carries a top-level key the host does not
  read.
- **SC-004**: No slot is filled with something its name does not describe.
- **SC-005**: A Roll for Shoes outcome cannot be changed by editing the
  browser's copy of the world's settings.
- **SC-006**: `make test-rust`, the web unit tests, `pnpm test:scripts`,
  `make lint` and the nine pre-commit checks pass after each story, and each
  story's e2e slice passes from the main checkout.

## Assumptions

- Settings are per world and per system. A per-scene or per-actor setting is
  the actor's or scene's own data and is out of scope.
- A setting is public to the world's members. A secret a Game Master keeps
  from players is not a setting.
- Stories 3 and 4 are described at the level the survey supports. Each gets
  its design confirmed with the owner before its first task starts: the
  adjudicator's exact signature and where an outcome is stored (Story 3),
  and whether a condition lives on the token or on the actor behind it
  (Story 4). Stories 1 and 2 do not wait on either.

## What this spec does not do

- It does not move Genie's or Roll for Shoes' existing tables onto the
  shared one.
- It does not let a pack run code in the engine or load at runtime.
- It does not add a slot nobody fills.
- It does not give conditions mechanical effect. A condition is shown; what
  it does is the table's business until a later spec says otherwise.
