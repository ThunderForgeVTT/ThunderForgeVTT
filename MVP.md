# MVP 1 Roadmap

The ten phases to Minimum Viable Product 1, and where each actually stands.

**Statuses are verified against the codebase, never assumed.** This document
once sat for months with every box unchecked while half the phases shipped,
and it has since carried a size figure that drifted 16% before anyone noticed.
So each phase below says what was checked and when. `[x] Done`,
`[~] Partial` (with what is missing), `[ ] Not started`.

**Last verified: 2026-08-31.**

## Core concepts

- **World** — the container for one game: scenes, actors, game system,
  members.
- **World events** — the append-only log of everything that changes in a
  world. This is what keeps clients in sync.
- **Game system** — the rules: stats, skills, dice formulas, resources.
  Declared by a manifest (`packs/systems/*/system.json`), not compiled into
  the engine. Extendable and shareable by design.
- **Scene** — one map or location: a background, a grid, tokens, walls,
  lights, interactive elements.
- **Actor** — a character or creature, with system-defined stats.
- **Token** — an actor's representation on a scene: position, kind, and an
  optional binding to an actor.
- **Actor–token binding** — the link that lets a token show an actor's
  information and an actor's stats affect the token.
- **Permissions** — who may do what, per world and per object.

## The roadmap

### [x] Phase 1 — User login

Username/password, OAuth, two-factor, sessions, admin bootstrap
(`src/server/src/auth/`, `src/server/src/users/`).

Spec 007 added deploy-time OAuth configuration by environment variable
(`OAUTH_<PROVIDER>_CLIENT_ID` and friends) alongside the admin panel; env vars
win for the fields they set. Providers are generic — any OAuth2/OIDC endpoint
set works — with Discord, Google, GitHub and Keycloak as built-in presets, and
multiple named instances of one provider type are supported (two Keycloak
realms, say). ADR-041.

### [x] Phase 2 — World creation

`createWorld` with `game_system_id`/`interface_pack_id`, plus listing and
deletion.

### [x] Phase 3 — Scene creation

Scene CRUD, `grid_size`/`grid_type`, background art by `.dd2vtt` import or
paste-to-canvas (spec 002, on RustFS asset storage). Spec 022 overhauled scene
management and made which scene a world is _on_ server state rather than a
per-client selection (ADR-046).

### [x] Phase 4 — Token creation

Tokens place, move and bind to actors. Spec 004 unified the backing store onto
the scene-scoped `tokens` table (ADR-040) and split player- from GM-initiated
moves (`moveOwnToken` vs `updateToken`). Spec 006 replaced a keyboard-shortcut
stand-in with real canvas-rendered resize and rotate handles.

**Closed since the last pass**: tokens now have a kind.
`thunderforge_canvas_core::token_kind::TokenKind` defines Character, NPC,
Vehicle and Object, parsed server-side (`parse_token_kind`) and rendered with a
per-kind palette in the engine (`token_kind_color`, `src/engine/src/lib.rs`).
The "no distinct token type or visual representation" gap this document
recorded is gone.

### [~] Phase 5 — Actor stats and customisation

Actors carry system-defined stats (`world_actors`,
`world_actor_system_data`), bind to tokens via `tokens.actor_id`, and have a
dedicated UI (spec 010): view/edit routes, a PC/NPC flag, an NPC catalog on
the staging page, and share-and-deep-copy of an actor into another of your own
worlds.

**Done — stats reach the canvas.** Spec 029 draws a token's resources as bars
above it (`src/engine/src/plugins/status_display.rs`) and shows the selected
token's in a viewer-chosen corner (`StatusPanel.tsx`). Which resources exist is
declared by `system.json`, never hard-coded — four real systems ship
declarations (Genie, D&D 5e, Pathfinder 2e, Blades in the Dark), and the engine
understands none of them, which is precisely what the four-system e2e exists to
prove. Coarsening for viewers not entitled to exact figures happens
server-side (`src/server/src/status_display.rs`), so a withheld value never
reaches the client. See `docs/status-displays.md`.

Capacity was measured, not assumed (SC-006): **3,200 tokens at 30fps with
displays on — the same as with none.** Before off-screen culling the same board
carried 16,003 sprites against 3,203 and ran at 20fps. Figures are generated
into `marketing/engine-status-capacity.json`, never transcribed.

**Still open — nothing acts on a computed stat.** `DerivedStats.movement_speed`
exists and is written, and `grep` finds no reader anywhere
(`src/engine/src/components.rs:172` is the only hit outside its own writes).
Movement is _modelled_ but not _enforced_: `movement_budget::cost_path` is
implemented and unit-tested, while `PlannedPath` still charges one cell per
step and no caller consults the cost. Gating movement on a computed speed
belongs to Phase 8.

### [x] Phase 6 — Walls and lighting

Walls, doors and light sources with 2D vision occlusion, and hand-drawn wall
and shape authoring on the canvas (specs 001, 002).

Spec 003 closed the round-trip trust gap: `map_import.rs` re-queries the
database after an import — and after hand edits on top of one — and asserts
exact field equality against the source across the richest real fixtures, so
nothing is silently lost or altered by a reload. It also surfaced previously
silent field categories as import `warnings` (freestanding portals, non-default
`ambient_light`, `objects_line_of_sight`).

Live cross-session sync works: `WorldPage.tsx` opens the `worldEventsCreated`
subscription once per mounted scene and feeds the wall, token, shape, light and
status appliers from it — one subscription, each applier filtering by its own
event code.

### [ ] Phase 7 — Scene levels

Upstairs and downstairs, each level with its own walls and token assignments.
No level concept exists in the schema or the engine. Not started.

### [~] Phase 8 — Game system integration

Shipped: the system package install and manifest-serving pipeline
(`src/server/src/systems.rs`), a registry with eight pack crates, and
server-side per-system validators that already enforce real rules — Pathfinder
2e rejects an ability value outside `-5..=10` because it stores modifiers where
D&D 5e stores raw scores.

**Attributes are system-declared, and the 5e formulas are gone.** The engine
once carried `TokenAbilities` — six fixed fields, strength through charisma —
with `derived_data.rs` computing on them using formulas its own comments
labelled D&D 5e (`10 + (dex - 10) / 2` for AC, and a `movement_speed` that
ignored both arguments and returned 30). That is one system's rules compiled
into a system-agnostic engine, which Principle I and spec 029's FR-001 both
forbid, and it did not survive contact with the systems already registered:
Blades in the Dark has no ability scores at all, just twelve action ratings on
a 0–3 scale. Attributes are now manifest-declared, resolved server-side against
the actor's sheet, and carried as `id -> value` pairs the engine does not
interpret (`TokenAttributes`, a map). The 5e formulas were removed rather than
fed. `apps/web/e2e/token-attributes.spec.ts` proves all four declaring systems
resolve their own sets under their own names.

**What is genuinely still open**, and it is narrower than "where do rules
execute":

1. **Engine-side rules.** `packs/systems/*/engine/` crates cannot be loaded
   into a running wasm engine the way server crates link into the binary.
2. **Validation is not enforcement.** Rejecting a bad ability score is not the
   same as gating movement on a computed speed. See Phase 5's open item.
3. ~~**A vestigial trait.**~~ Resolved by spec 032: `src/engine/src/systems/core.rs`
   is gone, and the one contract a system implements is
   `thunderforge_canvas_core::system_rules::SystemRules` — in the crate both the
   engine and the server already depend on, and the only one whose tests execute
   natively.

**Spec 032 closed the presentation half.** A system's sheet was a hand-written
React container per system, mounted from a `Record<string, ComponentType>` that
held exactly one entry, so six of the seven bundled packs had no character sheet
at all and were disabled in every picker as "(TBD)". A sheet is now what a
system *declares* — abilities, resources, movement, tracks, ladders, player-named
slots — laid out by whichever interface pack the world has chosen, with `forge`
as the generic default that renders any system. A pack gets a sheet by having a
manifest. `apps/web/e2e/world-appearance.spec.ts` opens 5e, Fate Core and Cypher
under the base pack alone and asserts the three sheets are not the same sheet.

The `interface_pack_id` field on a world is live: `packs/interface/` ships four
packs, three of them targeting a system, and the binding is validated before it
is stored and falls back to the base pack when the pack it names is not
installed.

**And the application stopped knowing which systems exist.** `/api/systems`
read the `game_systems` table, which has never held a row, so the client
carried two hand-kept literals naming all seven bundled systems and their
titles. The route now lists `packs/systems/` — the same directory listing
`/api/interface-packs` always used — and both literals are gone, along with an
unread `gameSystems` GraphQL query. **Adding a system pack that ships no code
is now one directory**, proved by creating one against a running stack and
watching it appear in the pickers. ADR-028 records why the table stays and
what it means now.

What is *not* done, and is recorded rather than glossed: a system pack cannot
yet contribute **behaviour**. World creation still branches on one system's
name to insert its session row, because that pack cannot own the table it
writes while 2,763 lines of that ruleset's GraphQL live in the server. ADR-063
sizes the move and research § F-5 finds the route — the server compiles as a
library, which is what lets a pack depend on it. A failing pack surface is
contained and names its pack, and `packs/systems/README.md` is the published
contract for authors.

### [x] Phase 9 — Multiplayer

Invite codes and shareable links, `joinWorld`, `world_members` and
`world_invites`. Spec 027 unified access links and deliberately gives every
dead link the _same_ message, whatever the cause — telling the holder which
cause applied is exactly what the server refuses to disclose (FR-011/SC-005).

**Closed since the last pass**: GM override of character selection, previously
recorded here as unaudited, is delivered by spec 017. A GM may un-claim any
character at any time under their existing Owner-level authority over every
actor in their world (spec 010's DM-always-full-control rule), and the
un-claimed player keeps their membership and returns to the "no character
selected" state. Covered by `apps/web/e2e/actor-claim.spec.ts`, including two
players racing to claim the same character with exactly one winner.

### [~] Phase 10 — Permissions model

A fixed three-tier role model (Owner/GM/Player) over `world_members`, with
`updateMemberRole` letting an Owner or GM change any member's role, including
promotion to Owner.

Spec 010 added a real per-object layer on top: an ownership block granting any
member Viewer/Editor/Owner on any actor, with the DM implicitly retaining full
control and a default-Viewer fallback. Enforced server-side
(`auth/actor_permissions.rs`, gating `updateActorSystemData` and
`moveOwnToken`). The same shape has since been extended to lore, items and
abilities (`world_*_permissions` tables).

**Missing**: this is per-object, not a general policy system. There is still no
world-wide "trusted player" or "assistant DM" role beyond the fixed three, and
no ownership block for scenes or maps. The `policies`/`permission_grants`
tables remain dead code. Same class of gap as `docs/SECURITY_RBAC.md` records.

## Interactive elements — spec 030

Things on a scene that respond: a prop that opens a lore entry, a lever that
switches lights, a door that opens, closes and locks, a region that fires when
a token crosses it, and a request the GM must approve. See
`docs/interactive-elements.md`.

The framing runs through the whole feature and ADR-051 records it: **this is
not a way to run a game without a Game Master.** Every trigger is something a
GM placed, configured and chose to allow, and anything consequential stays
gated on their approval.

**Delivered.** Doors gained a definition — open blocks neither, closed blocks
exactly what the wall's own profile says, and `locked` is a separate property
governing who may change the state rather than a third state. Props are tokens
of the existing Object kind with no actor, so there is no second placement
pipeline. All seven user stories are proven end to end
(`apps/web/e2e/interactive-*.spec.ts`), including the two rules most likely to
be implemented only in a UI: a player cannot open a locked door, and a request
never expires into approval.

**How it is built.** One plugin owns placing, triggering, permission and
dispatch, and owns no effect at all; every effect is contributed by the
subsystem that performs it. Adding a triggerable capability is a declaration, a
line in a list, and one plugin with one system — never an edit to the core.
`scripts/check-interaction-seam.mjs`, in `pnpm verify`, asserts that core names
none of "light", "door" or "sound", so the coupling this design forbids is
greppable rather than a matter of judgement. ADR-054.

**Deliberately not delivered:**

- **Sound.** No audio subsystem exists, so nothing declares a sound effect and
  none is offered. That is the contribution seam working, not a gap in it —
  when audio is built it contributes `audio.play` and nothing else changes.
- **Multi-scene navigation.** `nav.request_scene` raises a request, the GM
  approves or refuses, and the requester is told. Then nothing moves anybody,
  because there is nothing yet to move them with. The request and the decision
  are the parts this feature owns, and they work.
- **Party tokens.** A GM-controlled token the whole party sees and follows — a
  ship, a caravan — for world-map scenes. Named so the model does not preclude
  it; not built.

## Health of the test suite

Read `docs/e2e-suite-health.md` before reacting to a failure count; the number
is not self-explanatory, and most of what it reports has historically been the
suite rather than the product.

Two rules earned the hard way, and worth more than any individual fix:

1. **Read `apps/web/test-results/<slug>/error-context.md` before theorising.**
   It holds an accessibility snapshot of the page at the moment of failure and
   usually contains the answer. A long session was burned stepping through a
   store, its props and the DOM while the snapshot said plainly that the page
   was showing a different scene.
2. **Give every failure a verdict** — stale spec, real product bug, or
   intentionally removed behaviour. Weakening an assertion to get green is the
   one outcome worse than leaving it red.

Always `--workers=1` for anything touching the engine.

## Supported browsers

**Chromium-based browsers only, for now. Firefox is a later target.**

This is a real constraint rather than a preference, and it is worth stating
here because it decides who can use MVP 1 at all. The world cache is built on
OPFS, WebCrypto and IndexedDB (`thunderforge-cache-browser`), and a browser
missing any of the three cannot keep world content on the device.

What that costs elsewhere is small and deliberate: the crate degrades rather
than crashing, so an unsupported browser still *plays* — it re-downloads
content every session instead of keeping it. What it must never do is imply
nothing happened. A playtest found the diagnostics panel reporting "nothing
served, 0 B downloaded" against a world that genuinely had content to cache:
every figure true, the impression false. Spec 031 FR-042 fixed that, in two
places — a capability probe answering "could this work here?" before anything
is attempted, and the engine's own degradation reason answering "did it?".

The end-to-end suite runs Chromium alone, so **any claim about another browser
is untested by construction**, not merely unverified. Firefox support means
running the suite against it, not reasoning about feature tables.

Recorded in the constitution as a project-wide constraint; see also the
Post-MVP note on connections below, which is the other place MVP 1's operating
envelope is stated rather than assumed.

## Post-MVP

**Metered and constrained connections.** Explicitly not in MVP scope, and
planned. Nothing here is tested against a metered, capped or slow connection.
The client fetches world content ahead of need (spec 028's cache and its
background prefetch), which is the right trade on an unmetered link and an
untested one otherwise. There is no setting today to hold that back, and adding
one is a product decision rather than a switch.

- **Recommended baseline: an unmetered connection of at least 50 Mbps.** This
  figure is **UNTESTED** — a starting recommendation, not a measurement, and
  labelled that way deliberately, because this document already carried one
  number that drifted 16% before anyone noticed.
- What _has_ been measured bounds the guess from below: 4.92MB brotli for the
  engine bundle (2026-09-01, after spec 031; it was 4.15MB before), plus a
  world's art. Steady-state play is small — token moves
  and dice over one WebSocket.
- **If you play on something slower, on a metered link, or on mobile data, we
  want to hear about it.** That evidence is worth more than our estimate, and
  there is nowhere it can come from except people who actually did it:
  [the discussions](https://github.com/ThunderForgeVTT/ThunderForgeVTT/discussions).

**Sharing and federation.** Started, for actors only: share by link
(`/shared/actor/:code`), preview read-only, deep-copy into your own world with
cascaded ability, item and lore data. Not generalised to systems, scenes or
maps. No cross-instance federation exists.

**Marketplace.** Not started.

**Engine bundle size and load time.** Measured 2026-08-26. The dev-profile
bundle had reached 220,099,904 bytes — and 71.3% of that (157MB) was the wasm
`name` custom section, unmangled Rust and Bevy symbol names, not code. There
are no DWARF sections at all. That is why it gzipped 10:1, and why the number
was always more alarming than the program it described.

| Build                    |      Raw | gzip -9 | brotli -q11 |
| ------------------------ | -------: | ------: | ----------: |
| dev (was shipping)       | 220.1 MB | 21.1 MB |           — |
| release + `wasm-opt -O`  |  24.7 MB |  6.7 MB |     4.15 MB |
| release + `wasm-opt -Oz` |  21.0 MB |  6.6 MB |    4.152 MB |

8.9x smaller raw, 5.1x on the wire. `-Oz` is not worth it: 3.7MB less raw for
**1,861 bytes** less brotli, at ~6 extra minutes of optimiser time. Stay on
wasm-pack's default `-O`.

**Re-measured after spec 031 (2026-09-01): 29.83 MB raw, 7.91 MB gzip -9,
4.92 MB brotli -q11.** That is **+18.5% on the wire** against the row above —
0.77MB more brotli — and it is the whole spec, not one flag: `bevy_state` plus
six new engine plugins (authoring mode, placement, selection filter, scene
transition, interaction marker, context menu).

The `bevy_state`-only delta that spec 031's T001 was meant to record **cannot
now be recovered**. The engine uses `States`/`NextState` throughout, so a build
without the feature no longer compiles — the measurement had to be taken when
the flag landed and was not. Recorded here as the process lesson it is: a
"record the delta" step gets skipped unless the number has somewhere to live,
which is the same failure mode that let this document carry a figure that
drifted 16%.

Both figures above are measured, not estimated: `brotli -q11` via Python's
`brotli`, `gzip -9` via `gzip`, on the release build wasm-pack produced.

### The bundle budget

**The engine is expected to grow as it gains features. The threshold of concern
is 100MB after compression.** At 4.92MB brotli there is roughly 20x headroom,
so an 18.5% rise across one spec is growth, not a problem, and does not need
re-litigating each time a feature lands.

That budget is as generous as it is because of the downloader, not in spite of
it: spec 028 caches world content on the device and prefetches ahead of need,
so the engine bundle is a first-visit cost that is then kept, rather than a
per-session one. What the budget protects is the first visit — and 100MB
compressed is where that stops being reasonable on an ordinary connection.

The number to watch is therefore **brotli, not raw**. Raw size is mostly the
wasm `name` section (see the table above), which compresses roughly 6:1 and is
why the raw figure has always looked more alarming than the program it
describes. Track the compressed column here on each spec that touches the
engine; that is the one the budget is stated in.

- **Done** — `scripts/shared.mjs` selects the profile per caller: the dev loop
  keeps `--dev` (a 7-minute rebuild after every engine edit is not a dev loop),
  everything else defaults to `--release`. `ENGINE_PROFILE` overrides. The
  profile participates in the `pkg.sum` cache key, without which switching
  profiles silently serves whichever bundle was already on disk.
- **Done** — the server was serving the wasm _uncompressed_: `tower-http` had
  no compression feature and no `CompressionLayer`. That was a ~6x gap on
  first-load bytes on top of the release win. Now brotli and gzip.
- **Done, and not the same thing** — the load is no longer silent. Spec 028
  shipped byte-level progress, a loading state inside a second, and a real
  explanation with a working retry when the download or startup fails, pinned
  by `apps/web/e2e/engine-loading.spec.ts`. Worth stating plainly because the
  two are easy to conflate: **feedback is not size.** A 4.15MB first load that
  reports itself honestly is a different problem from a 4.15MB first load, and
  only the first is closed.
- **Open — per-pack lazy loading.** WASM has no dynamic code-splitting
  analogous to JS `import()`. A workspace split alone will not shrink the
  shipped `.wasm` unless each piece compiles to its own binary, fetched and
  instantiated over a stable JS-glue boundary. The natural seam is
  `packs/systems/*/engine` — a world needs only its active system. Real
  architectural work, and at 4.92MB brotli the case is still much weaker than
  it looked — though 18.5% closer than it was before spec 031.
- **Open — Bevy feature trim.** `bevy_ui`/`bevy_ui_render` are unused (the
  debug HUD that needed them was removed) but deliberately retained; `webp`
  appears unreferenced in the engine crate; `bevy_gizmos` is diagnostic-only and
  could be feature-gated. None measured, and behind the two items above.
