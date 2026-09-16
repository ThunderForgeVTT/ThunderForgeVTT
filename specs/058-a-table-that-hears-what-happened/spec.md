# Feature Specification: A Table That Hears What Happened

**Feature Branch**: `058-a-table-that-hears-what-happened`

**Created**: 2026-09-16

**Status**: Draft — proposal, nothing built

**Input**: Project owner, on a Genie world's play field, 2026-09-15: "the chat
feature doesn't show somebody made a wish. Those should be options we should
enable — they're really attributable to fifth edition or Pathfinder, showing
specific rolls or showing specific abilities. That should be something
malleable, but it's not quite there."

## The problem

### One kind of action reaches the table, and it is attacks

Spec 046 put an attack in front of everyone at the table. `make_attack`
(`src/server/src/combat/attack.rs:456`) writes a `world_attacks` row (`:687`)
and raises `EVENT_CODE_ATTACK_MADE` (`src/server/src/world_events.rs:175`),
whose payload is the attack's id and nothing else. Each client then reads the
attack back through `attack(id)`
(`src/server/src/graphql/queries/attacks.rs:43`), and the server decides what
that viewer may know when it builds the answer
(`src/server/src/graphql/types_attacks.rs:420`, rules in
`src/server/src/combat/redaction.rs`). An attacker the viewer cannot see
arrives as "Unknown", with no token id and no ability name (FR-002a,
`specs/046-a-fight-that-resolves/spec.md:433`). The dock renders it in
`apps/web/src/components/world/PlayDock/AttackLog/AttackLog.tsx:43`.

That is the right shape. It exists for attacks alone.

### Everything else happens in silence

- **A roll announces nothing.** `rollDice`
  (`src/server/src/graphql/mutations_roll.rs:124`, core `:48`) writes a
  `world_roll_records` row and returns the result to the person who rolled.
  It raises no world event and posts nothing. The only read,
  `worldRollRecords` (`src/server/src/graphql/queries/roll.rs:68`), is for
  the Game Master (`:30`), and it is a history screen, not something the
  table sees happen.
- **An ability used announces nothing** unless it is an attack.
- **A wish announces nothing.** `spend_wish_impl`
  (`packs/systems/genie/server/src/session/mutations/clocks.rs:230`) raises
  event 15 so the other clients refetch the pool. Since 2026-09-16 the Wish
  Pool lists what each wish asked for (`genieWishLog`,
  `packs/systems/genie/server/src/session/wish_log.rs:61`), but that list is
  inside Genie's own panel. Nothing tells a player looking at chat that the
  party just spent a wish.
- **Chat holds only what people type.** `world_chat_messages`
  (`src/server/src/schema.rs:1147`) is written in one place,
  `sendChatMessage` (`src/server/src/graphql/mutations_chat.rs:157`). Its
  only visibility setting is `gm_only` (`:80-84`), and
  `apps/web/src/components/world/PlayDock/ChatPanel.tsx` has nothing to show
  a system message with.

### No pack can say "announce this, in these words"

No `packs/systems/*/system.json` has a key for announcements, chat or
message templates. Packs already declare the rolls they make —
`manifestationRoll` (genie `:150`), `checks[]` (dnd5e `:146`), `coreCheck`
(pathfinder2e `:125`), `actionRoll` (blades_in_the_dark `:89`),
`taskResolution` (cypher_system `:56`), `ladderRoll` (fate_core `:94`),
`skillRoll` (year_zero_engine `:89`) — and 033's vocabulary names their
abilities. But a Pathfinder table cannot say "Aurelia rolls a Fortitude
save", and a Genie table cannot say "The party wishes", because nothing asks
the pack how.

### Why this is its own spec and not a section of 033

033 decides what a system *calls* its abilities: umbrella, types and facets.
It has no numbered top-level sections to add to; its requirements are grouped
by user story and end at FR-039. This spec decides which *events of play* a
table is told about, who is told, and through which surface. It uses 033's
vocabulary but is not part of it, much as 046 uses 033 without living in it.

## Proposal

### 1. What a table is told about: announcements

An **announcement** is a record that something happened in play, written by
the server when the action happens, and read back per viewer. There are three
kinds to start with:

| Kind | Raised by | Carries |
|------|-----------|---------|
| `roll` | `rollDice`, when the roll names what it is for (a declared check, save or skill) | actor, the check's id, formula, result, and outcome where the pack grades one |
| `ability` | using an ability that is not an attack | actor, ability id, target(s) if any |
| `pack` | a pack's own mutation (a wish granted, a Doom Clock advancing) | an event id the pack declared, plus the fields it declared |

Attacks keep the log and record they already have (§3). A roll made with no
declared purpose, typed as a bare formula, is not announced by default,
because a table where every stray `1d20` posts to chat is noise.

### 2. Where announcements appear: one feed, shown in two places

**Recommendation: one server-side feed, not a second chat table and not a
second attack log.**

- A `world_announcements` table, read through `sceneAnnouncements(sceneId,
  before)` with the same newest-first paging as `sceneAttacks`
  (`queries/attacks.rs:74`), and nudged by one new event code carrying only
  an id, as 29 does.
- **Chat shows them inline**, visually distinct from typed messages, because
  chat is where the owner looked for the wish. Chat reads both queries and
  merges them by time. It does not copy announcements into
  `world_chat_messages`, which stays for what people type.
- **The attack log stays the attack log.** An attack is not re-announced in
  the feed. Chat shows attacks too, read from `sceneAttacks`, so chat becomes
  the one place a table sees everything, and the attack log stays the focused
  combat view.

### 3. Who sees what: the rules attacks already use

There is one redaction rule, not a second copy.

- **Actor and target** are redacted exactly as `combat/redaction.rs` redacts
  an attack's parties. The Game Master sees everything. A token's controller
  knows that token. Anyone else knows a token only when its name is visible
  and one of their own tokens perceives it. Otherwise it is "Unknown", with
  no id in any field (046 FR-002a). The rule is decided when the answer is
  built, not when the row is stored, so a later reveal applies to earlier
  announcements.
- **An NPC hidden by the NPC visibility rule**
  (`src/server/src/auth/npc_visibility.rs`) is "Unknown" to a player,
  whatever the scene's sight says.
- **A pack may declare an announcement GM-only** (a Doom Clock advance at a
  table that keeps it secret). Players are then not sent the row at all, the
  same as a `gm_only` chat message (`mutations_chat.rs:208`).
- **A roll the roller marks private** is visible to the roller and the Game
  Master only. This is the "roll to GM" every table expects, and it is the
  one control a player has.
- **A result is visible whenever its announcement is.** A table that wants
  hidden results uses a private roll. It does not need a third rule.

### 4. How a pack declares what it announces, in its own words

A new top-level manifest key, `announcements`, keyed by what is announced:

```jsonc
// packs/systems/genie/system.json
"announcements": {
  "events": {
    "wish_spent": {
      "text": "{actor|The party} wishes: {narrative_effect}",
      "audience": "table",
      "default": true
    },
    "doom_clock_advanced": {
      "text": "The Doom Clock advances to {current} of {max}.",
      "audience": "gm",
      "default": true
    }
  },
  "rolls": {
    "manifestationRoll": { "text": "{actor} manifests: {successes} successes", "default": true }
  }
}
```

```jsonc
// packs/systems/pathfinder2e/system.json
"announcements": {
  "rolls": {
    "coreCheck": {
      "text": "{actor} attempts {check}: {degree}",
      "default": true
    }
  },
  "abilities": { "text": "{actor} uses {ability}", "default": true }
}
```

- `rolls` keys are the roll ids the manifest already declares, and
  `abilities` uses 033's vocabulary for `{ability}`. A pack therefore names
  nothing new: it adds wording to things it already has.
- `events` is for a pack's own actions. The server does not know what a wish
  is. The pack's mutation raises `wish_spent` with the fields named in its
  template, and the pack contract check fails a template that names a field
  the event never supplies.
- Templates are **text with named slots, not markup and not code**.
  `{a|fallback}` covers an absent actor. The server fills the slots after
  redaction, so an "Unknown" actor is filled as "Unknown" and a template
  cannot leak what redaction removed.
- Localisation follows whatever 033 decides for vocabulary labels. This spec
  does not invent a second mechanism.

### 5. Malleable per world: the owner's "options we should enable"

The pack sets the defaults and the world decides. A world setting lists every
announcement the pack declares, each with an on/off switch (the pack's
`default` is the initial value) and, where declared, the audience (`table` or
`gm`). It sits beside the world's other system settings
(`apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`). A Game
Master cannot edit a template's wording from the world. That is the pack's
voice, and a world that wants different words is a fork of the pack.

## Requirements (draft)

- **FR-001**: The server MUST write an announcement for every declared,
  enabled roll, ability use and pack event at the moment it happens, in the
  same transaction as the action.
- **FR-002**: Announcements MUST be read per viewer, with actor and target
  redacted by the rule in `combat/redaction.rs` and the NPC visibility rule.
  No identity the viewer may not know may reach their client in any field,
  including a filled template.
- **FR-003**: The world event that nudges clients MUST carry only the
  announcement's id, as event 29 does.
- **FR-004**: Chat MUST show announcements and attacks inline, ordered by
  time, distinct from typed messages. `world_chat_messages` MUST NOT be
  written to by anything but `sendChatMessage`.
- **FR-005**: A pack MUST declare what it announces in
  `system.json#announcements`. A template naming a slot its event does not
  supply MUST fail the pack contract check.
- **FR-006**: A world MUST be able to turn each declared announcement on or
  off, and choose `table` or `gm` where the pack allows it, without editing
  the pack.
- **FR-007**: A roll marked private MUST be visible only to its roller and
  whoever runs the world.
- **FR-008**: A roll with no declared purpose MUST NOT be announced unless the
  world turns bare rolls on.

## Questions for the owner

1. **Should chat show attacks too, or only link to the attack log?**
   Recommendation: show them inline. The owner looked in chat, and a table
   should not have to know which panel an action lands in.
2. **Can a player mark a roll private, or only the Game Master?**
   Recommendation: both. A player rolling Perception they don't want the
   table to see is ordinary play, and the Game Master always sees it anyway.
3. **Should the Genie wish announcement name the Game Master who pressed the
   button, or "the party"?** Recommendation: "the party". A wish is spent by
   group agreement (018 FR-013), and the button is the Game Master's only
   because adjudication is (FR-014).
4. **Bare rolls (a typed `2d6`): off by default, or on?** Recommendation: off,
   with a world switch (FR-008).
5. **Do announcements backfill?** Rolls already in `world_roll_records` and
   wishes already in the audit trail could be shown. Recommendation: no. The
   feed starts when the feature ships, because an old roll made with no
   declared purpose cannot be worded honestly.

## Out of Scope

- Building any of it. This document is the proposal the owner asked for.
- Dice animations, sound, or a 3D dice tray.
- Whispers to a chosen player: chat has only `gm_only` today, and adding
  per-recipient messages is its own decision.
- An AI narrator for announcements. Templates are the pack's words, and the
  Game Master is the table's voice.
