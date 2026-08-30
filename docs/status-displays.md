# Status displays

Bars and counters drawn on the map, and a panel in a screen corner for the
selected token. Spec 029.

## What problem this solves

A person at a table needs to read the board, not interrogate it. Before this,
every token was a coloured square or a piece of art, and everything else about
it — how hurt it is, how much of its magic is spent, whether it is about to
drop — was invisible until somebody clicked. In a fight with a dozen
combatants that is a dozen clicks to answer a question the board should simply
be showing.

Two surfaces answer it:

- **Token-attached bars**, drawn by the engine
  (`src/engine/src/plugins/status_display.rs`). They track the token's
  position, scale with the camera, and reorder with other entities.
- **A corner panel** for the selected token, drawn in React
  (`apps/web/src/components/StatusPanel/StatusPanel.tsx`). It is screen-space
  text, and drawing it in WebGL would mean reimplementing text layout, focus
  handling and screen readers to obey a principle that explicitly permits
  panels in React.

That split is ADR-053
(`docs/adrs/20260829-053-generated-engine-sdk-and-status-presentation-split.md`),
and it is the Constitution's spatial-versus-chrome line rather than a
convenience. What keeps it from becoming the two competing stores Principle I
exists to prevent: **the engine owns the resolved display state and React
observes it.** The panel computes no value, holds no resource state and makes
no disclosure decision; it reads what the engine already holds, through
`readTokenStatus` in `apps/web/src/engine/bevy/tokenStatus.ts`.

## The engine does not know what "health" is

One system tracks hit points and nothing else. Another tracks health, stamina
and mana. A third tracks stress, trauma and coin. Hard-coding the first would
make every system after it a special case, so the engine contains no built-in
notion of any named resource (FR-001). It draws what the active game system
declares.

A system declares resources in its `system.json`, under a top-level
`resources` array. The server reads it in `declarations_for_system`
(`src/server/src/status_display.rs`) from the installed systems directory.

```jsonc
{
  "id": "health", // stable identifier; disclosure overrides key on this
  "label": "Health", // what a human sees
  "kind": "bar", // "bar" (has a maximum) or "counter" (does not)
  "order": 0, // display order; the engine imposes none
  "allowStacking": false, // may this resource have more than one entry?
  "source": {
    "slot": "resourceData", // which stored actor slot to read
    "entries": [{ "current": "current_health", "max": "max_health" }],
  },
}
```

`source` is the whole point of FR-001. The server never learns what "health"
means — only that this resource's first entry takes its current value from the
field `current_health` and its maximum from `max_health`, inside the actor's
`resourceData` slot (`world_actor_system_data`). Without that indirection,
every new game system would need server changes before it could be displayed,
and the engine would accumulate one special case per ruleset.

An `EntrySource` has five fields:

| field      | meaning                                                              |
| ---------- | -------------------------------------------------------------------- |
| `current`  | field holding this entry's current value. Required.                  |
| `max`      | field holding its maximum. Absent for a counter, or a granted layer. |
| `maxValue` | a maximum fixed by the rules rather than stored per character.       |
| `label`    | name for this layer, shown when there is more than one.              |
| `optional` | skip this entry when the field is missing or zero.                   |

`max` wins when both `max` and `maxValue` are given: a stored value is about
_this_ character, a literal is about everyone.

A field that is absent, non-numeric, or zero on an `optional` entry yields **no
entry**, not a zeroed one. A zero would draw an empty bar, and an empty bar
claims the creature is at zero — a far stronger statement than "this system
stored nothing". A resource with no entries at all is omitted from the token
entirely (FR-007); no empty container is drawn.

### The four systems that ship, and what each one exercises

These are not illustrations; they are the four real declarations in
`packs/systems/*/system.json`, and each was added because it forced something
the model had to handle.

- **Genie** — `health` and `wishPoints`, both plain bars from stored
  current/max pairs. The straightforward case, and the one the end-to-end test
  pins.
- **D&D 5e** — one `hitPoints` resource with `allowStacking: true` and two
  entries: `current_hp`/`max_hp`, then an optional `temporary_hp` layer
  labelled "Temporary" with no maximum of its own. That system represents
  temporary hit points by letting `current_hp` exceed `max_hp`; the entry model
  is what makes that expressible without a value above a maximum.
- **Pathfinder 2e** — hit points as a bar, plus `focusPoints` and `heroPoints`
  as **counters**: a current field and nothing else, so there is no proportion
  to draw and they render as counts.
- **Blades in the Dark** — `stress` and `trauma` as bars whose maxima come from
  `maxValue` (9 and 4), because the cap is a rule and no character stores it
  anywhere, plus `coin` as a counter. Without `maxValue` these could only be
  shown as bare counts, losing the thing a player most needs to see: how close
  to the cap they are.

### Adding resources to a new system

1. Add a `resources` array to the system's `system.json`, one object per
   resource, in the shape above.
2. Point each entry's `current`/`max` at fields that actually exist in the
   actor data your system writes. The names are yours; nothing outside your
   manifest interprets them.
3. Choose `kind`. A `counter` is a resource with no maximum from either source
   — it draws as a number, and `proportion` answers `None` for it.
4. Set `allowStacking: true` only if the resource genuinely layers. If it does
   not, a second entry is rejected outright rather than merged, because merging
   loses which pool was temporary.
5. Set `order`. The engine sorts by it and imposes nothing of its own.

No engine change, no server change, no rebuild of either. A system declaring
none is correct and yields tokens with no status furniture at all — that is a
ruleset that tracks no pools, not a gap to fill with a default.

Two behaviours to know when a declaration changes: a resource present in stored
data but absent from the current declaration is **not displayed** (FR-005), and
a `kind` the renderer cannot draw is dropped rather than guessed at.

## Why a resource is a list of entries

The obvious model is `{ current, max }`, and it immediately raises a question
it cannot answer: what does a value above the maximum mean? Temporary hit
points, a shield, the second stage of a boss — all real, all expressible only
as "more than full", which then needs a clamping rule that will be wrong for at
least one of them.

`ResourceEntry` removes the question. **Overflow is not a value exceeding a
bound; it is a further entry.** A boss with three stages is three entries. A
shield is an entry stacked above the base pool. There is no state in which a
value exceeds its own entry's maximum, so nothing has to decide what to do
about one — and a value that _does_ exceed it is an unambiguous defect to
report (`EntryError::ValueOutOfRange`), not a judgement call.

Consequences worth knowing:

- **Depletion consumes the topmost entry first** (`deplete`). A shield over
  health goes first, then spills into the pool below.
- **A spent entry stays in the list at zero** rather than being removed. A boss
  on its last stage should still read as being on its _last_ stage, and that
  needs the exhausted ones to remain visible.
- **Damage beyond the whole pool is returned** rather than swallowed, so a
  caller can act on the remainder.
- **Banding spans every entry**, not just the top one: `proportion` and
  `quarter` sum current and maximum across the list.
- A resource with no maximum anywhere has no proportion, and `proportion` and
  `quarter` both answer `None`.

`quarter` rounds **down**, so anything short of full reads as less than full
and only a genuinely empty pool reads as empty. Rounding to nearest would show
a creature at 88% as "full" and one at 12% as "empty", both of which are lies a
player would act on.

The model, its depletion order and its banding all live in
`crates/thunderforge-canvas-core/src/resource_display.rs` rather than in the
engine crate. That placement is forced: the engine crate builds for
`wasm32-unknown-unknown` and its tests compile without ever running, so a rule
placed there is untested by construction. These execute.

## Disclosure

A bar is a disclosure channel. A player watching a boss's health bar learns
something whether or not anybody meant them to, and this project has already
shipped one bug of exactly that class — a hidden scene's art was reachable by
asking for its id directly, because two call sites answered the same question
differently.

So disclosure is **part of the model, not a filter over it**. The server
resolves what each viewer may see and sends only that; a client is never sent a
figure it may not display — not sent and hidden, **not sent**. A UI that
conceals a field the API still returns is a UI, not a permission. The
resolution happens in `resolve_token` (`src/server/src/status_display.rs`) and
the reduction in `disclose`.

The wire type is tagged on `disclosure`, so each state's payload carries
exactly the one field it permits and no other. An over-disclosing payload is
_unrepresentable_ rather than forbidden by a rule somebody has to remember —
and on the TypeScript side it generates a discriminated union that narrows.

### The four states

The Game Master sets these **per token and per resource** — two tokens of the
same creature in one scene may legitimately differ — and the GM continues to
see the true value in every one of them. Listed least to most revealing, which
is the order the control presents them in:

- **Greyed** — the bar's _presence_ is disclosed and its value is not. This is
  the honest form of "hidden": removing the bar entirely also discloses
  something, because a token conspicuously lacking a bar every other token has
  is itself a signal. It arrives as `{"disclosure":"greyed"}` and nothing else.
- **Chunked** — the proportion rounded down to quarters, sent as a quarter
  index 0–4. The coarsest disclosure that still communicates "hurt" versus
  "nearly dead".
- **Percentage** — a proportion, with no maximum attached. A viewer learns the
  creature is at 40% and not that it has 400 hit points.
- **Visible** — the exact entries, as the GM sees them.

### Percentage discloses more than it appears to

This is the one thing in the feature that must not be buried, and it is the
reason the interface does not present the four states as four appearances.

**A viewer who knows the damage they dealt can divide it by the percentage
change, recover the maximum, and read exact values from then on.** Hit a
creature for 12 and watch the bar move 4%: the pool is 300, and every later
percentage is now an exact figure. One informative hit is enough, and no
further leak is required — the coarsening has been inverted.

**Chunked resists this** because a quarter index rarely moves on a single hit.
The same 12 damage against a 300-point pool moves nothing at all, and when the
index does move it says only that a boundary was crossed somewhere in a
75-point band.

Both are offered. A readable boss fight is a legitimate thing to want, and
percentage serves it. But they are **not equally safe**, and a Game Master
choosing between them should be choosing with that in front of them —
`TokenDisclosureControl` carries the caveat on the option it applies to, and
`FR-013c` is the requirement that says it must.

### The default is derived from the actor, not configured

**There is no world-level default setting**, and that is the design rather than
an omission.

A token is bound to an actor, and the actor already records what it is. The
viewer's relationship to it is likewise already known — ownership says whose
character a token is. `subject_for` turns those two facts into a
`TokenSubject`, and `default_disclosure` turns that into a state:

| subject                           | default     | why                                                                                    |
| --------------------------------- | ----------- | -------------------------------------------------------------------------------------- |
| your own character                | **visible** | you always know your own hit points                                                    |
| another player's character        | **visible** | a party shares this at a table; coarsening it makes four players worse at coordinating |
| anyone the Game Master runs (NPC) | **chunked** | readable enough to play — "that ogre is nearly dead" — without handing out figures     |

Deriving the answer from data that already exists beats a setting somebody has
to discover, because a table that never finds the setting plays under whatever
we guessed, while a derived default is correct for a table that configures
nothing — which is most tables.

NPCs default to chunked rather than greyed for a reason worth stating: a board
where every NPC bar is blank is a board that gives players nothing, and they
will ask the GM for the number instead, which is worse than telling them a
quarter band.

An explicit per-token override still wins. This is the floor, not a ceiling: a
GM who wants a boss fully visible or fully greyed says so, through
`setTokenDisclosure`, and is obeyed. The rows live in
`token_resource_disclosure`, keyed by token and resource id.

Two failure directions are deliberate:

- A **stored state this build does not recognise** is skipped, so the derived
  default applies. Failing closed toward _less_ disclosure than the row asked
  for is the safe direction to be wrong in.
- The **GM branch ignores the override entirely**. A GM who has hidden a boss
  from the table still has to run the fight. `ResolvedResource` therefore
  carries `configured` alongside `disclosed`, so a control can show what the
  table is under without confusing it with what the GM is seeing.

### Not disclosed is not zero

An undisclosed bar renders as a mid-grey fill, deliberately not an empty one.
An empty bar says "at zero", which is a different and much more actionable
claim than "you have not been told" — rendering the two alike would leak by
implication, and a player would read a withheld boss as nearly dead. The panel
says "Not disclosed" in words for the same reason (FR-008, FR-014).

## The SDK

The types crossing the engine boundary are **generated, not mirrored**. They
are defined once in `crates/thunderforge-canvas-core/src/resource_display.rs`
and emitted to `apps/web/src/engine/sdk/` by `ts-rs`.

```bash
pnpm sdk:bindings     # regenerate
pnpm sdk:check        # regenerate and fail if the committed output differs
```

The generated files are committed so the web application builds without a Rust
toolchain; `sdk:check` in CI is what keeps that from silently rotting. The
shapes are generated; the typed wrappers application code actually calls are
hand-written beside them.

The reason is the failure mode of the alternative. `apply_world_command(json)`
deserializes what it recognises and ignores the rest, so a renamed or mistyped
field produced a display that silently did not appear — no error, no warning,
no log. That has already happened here in adjacent forms: the engine's `Token`
component carried a `token_type` nothing drew, and `WorldTokenPayload` did not
deserialize `health`/`maxHealth` although the client had been sending both
since spec 004.

Commands carry an integer `sdkVersion` (`SDK_VERSION` in `src/engine/src/lib.rs`,
currently 1). A mismatch is rejected outright with a reported error and applies
nothing.

`ts-rs` rather than the `schemars` already in the tree, and the distinction is
by job: JSON Schema is a validation vocabulary, and a Rust enum round-trips
through it as `oneOf` that narrows poorly. What is being bought here is a
discriminated union that makes a wrong payload a compile error, and `ts-rs`
emits that directly. `schemars` keeps manifest validation and the published API
schemas.

## How a value reaches the screen

1. A game system's `system.json` declares the resource and where to read it.
2. `declarations_for_system` reads that manifest server-side.
3. `entries_from` pulls the named fields out of the actor's stored slot.
4. `subject_for` and any `token_resource_disclosure` row settle the state;
   `disclose` reduces the entries to what that state permits.
5. The `tokenStatus` GraphQL query carries the reduced form to the client, and
   `worldEventsCreated` carries later changes live — one subscription per
   mounted scene in `WorldPage.tsx`, primed with a fetch so the first paint
   already has bars rather than waiting for something to change.
6. The engine attaches a `TokenStatus` component and draws the bars; the React
   panel reads the same resolved state back through `readTokenStatus`.

Token spawn and status arrival can happen in either order, and both directions
are handled: a status arriving before its token is held in a slot the spawn
adopts.

## Panel placement

The corner is the viewer's choice — top-left, top-right, bottom-left,
bottom-right — and persists in `localStorage` under
`thunderforge.statusPanel.corner`, defaulting to bottom-right. It is a
per-viewer convenience and deliberately does not follow anyone to another
device.

The panel follows **selection**, not ownership, and is cleared on deselection
rather than retaining the last token's numbers, which would be actively
misleading mid-fight.

## What this does not do

Stated because the gaps matter more than the features:

- **No theming.** Appearance is shaped so a later theming feature has something
  to configure — the constants live in one place in the status-display plugin —
  but there is no theming UI, no user-authored themes, and no per-world
  palette.
- **No editing from the panel.** This feature displays; it does not mutate. The
  read surface is read-only on purpose, because a debugging surface that can
  also write becomes a way to write tests that pass against situations the
  application cannot reach.
- **No clocks.** Blades in the Dark runs on progress clocks, and Genie has
  puzzle and doom clocks; they look like a bar and are not one. A clock fills
  _toward_ an event where a resource depletes toward zero, it belongs to the
  situation rather than to a token, and its segments are discrete and named
  rather than a proportion — four of six is not 67%. Declaring one as a
  resource to make it fit would be the mistake this feature twice avoided.
  Spec 029 records it as a follow-up needing its own spec: a home that is not a
  token, a fill direction, and segment rendering.
- **Nothing acts on these values.** The engine's derived-statistics subsystem
  now runs on real tokens, but nothing gates movement or any other behaviour on
  a computed figure. That is the game-system rule-enforcement work (MVP Phase
  8), and spec 029 puts it explicitly out of scope.
- **Ability scores are not populated.** `Token.abilities` is spawned empty
  because the server does not send them, so `calculate_ability_stats` still
  matches nothing. Filling it with zeroes would be worse than leaving it empty:
  derived values computed from invented ability scores would be confidently
  wrong.
