# Roll for Shoes

A whole roleplaying game in six rules, by Ben Wray — [rollforshoes.com](https://rollforshoes.com/).
Dedicated to the public domain under **CC0 1.0 Universal**, which is why it
ships bundled. `system.json`'s `legal` block carries the attribution.

Spec 061 built the core game. Spec 062 added the five optional rules the site
publishes as **Extras**, and every one of them is off until a Game Master turns
it on.

## The core game

A character holds skills, each at a level. Rolling one means rolling a d6 for
each level and summing them, and the attempt succeeds only by going **higher
than** the opposition — a tie is a failure. A failure earns 1 XP. If every die
comes up a six, the character learns a new skill, one level above the one just
rolled and narrower than it; who judges "narrower" is the table, never this
code. XP buys a die up to a six, and only for that purpose.

Everyone starts with `Do Anything 1`.

## The Extras

Each is an independent per-world setting, off by default, set on the world's
system-settings page (`web/src/panels/world-settings.tsx`). A world that
touches none of them plays exactly the core game.

| Setting               | Off (default)                     | On                                                                                                                        |
| --------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `difficulty_mode`     | `free` — the GM types a number    | `rolled`: GM dice by band (Easy 1, Moderate 2, Hard 3, Very Hard 4), rolled and summed. `target`: a fixed 3 / 6 / 9 / 12.   |
| `tie_succeeds`        | a tie fails and earns 1 XP        | a tie succeeds, and therefore earns nothing                                                                                 |
| `statuses_enabled`    | no statuses                       | named conditions with signed modifiers, summed and applied **to the total, never to a die**                                 |
| `skill_slots_enabled` | any number of skills at any level | 4 slots at level 2, 3 at level 3, 2 at level 4; levels 1 and 5+ uncapped. More cost `2 × level` XP.                          |
| `starting_skills`     | `Do Anything 1`                   | whatever skills, at whatever levels, the world declares                                                                     |

The band is a per-roll choice, not a setting, in every difficulty mode; the
free number stays available in all three.

Two properties hold by construction rather than by care:

- **A status can neither create nor destroy an advancement.** Advancement reads
  the rolled faces; the modifier never reaches them. `isAdvancement` has no
  parameter a status could enter by, and the unit tests assert its arity.
- **Turning slots on never orphans anyone.** The cap governs gaining a skill,
  not storing one. A character already over it reads as negative room — not an
  error, and not something the server refuses to save.
- **Changing the starting skills never touches anyone already playing.** A
  character with stored skills never reaches the branch that reads the setting,
  so nothing compares dates and nothing needs migrating.

## Where things live

```
server/src/validators.rs     what a stored character may look like
server/src/settings/         the settings table, its GraphQL fields, its surface
web/src/game.ts              every rule, as pure functions
web/src/ActorSheet.tsx       the sheet
web/src/panels/world-settings.tsx  the Game Master's switches
```

The settings live in **`world_roll_for_shoes_settings`**, a table this pack
owns (ADR-063), one row per world, written on first change. A world with no row
reads as every default — which is what makes the whole feature opt-in rather
than merely defaulted-off. See
[ADR-108](../../../docs/adrs/20260923-108-a_generic_world_settings_surface_is_deferred.md)
for why there is not yet a generic per-world settings surface to use instead,
and what the next pack wanting one should do.

## Checking it

```sh
cargo test -p thunderforge-system-roll-for-shoes --lib          # validators
pnpm --filter @thunderforge/roll-for-shoes test    # the rules
pnpm --filter @thunderforge/web typecheck          # the sheet and the panel
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:game-systems
```

## Not built

The **New Shoes!** variant is a separate nine-rule replacement ruleset that
needs a first-class scene boundary the core game has no use for; it is not a
setting on this one. The site's published scenarios and its D66 NPC tables are
content, not rules, and belong in a collection rather than here.
