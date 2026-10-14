# MVP 1 Roadmap

The ten phases to Minimum Viable Product 1, and where each actually stands.

**Statuses are verified against the codebase, never assumed.** This document
once sat for months with every box unchecked while half the phases shipped,
and it has since carried a size figure that drifted 16% before anyone noticed.
So each phase below says what was checked and when. `[x] Done`,
`[~] Partial` (with what is missing), `[ ] Not started`.

**Last verified: 2026-10-13**, against `main` at `da60b0fa`, by reading the
code, the migrations, the e2e files and the spec ledgers — nothing was run.
Phases 7 and 8 moved; the rest kept their marker and had their text brought
up to date where it had drifted.

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
(`crates/thunderforge-server/src/auth/`, `crates/thunderforge-server/src/users/`).

Spec 007 added deploy-time OAuth configuration by environment variable
(`OAUTH_<PROVIDER>_CLIENT_ID` and friends) alongside the admin panel; env vars
win for the fields they set. Providers are generic — any OAuth2/OIDC endpoint
set works — with Discord, Google, GitHub and Keycloak as built-in presets, and
multiple named instances of one provider type are supported (two Keycloak
realms, say). ADR-041.

**Sessions changed on 2026-09-07 (spec 036, ADR-073).** Signing in used to
revoke every live session for the account — one line in `create_session`,
deliberate, "to reduce session replay risk" — so one account could be signed
in in exactly one place and a second browser signed the first one out. That is
gone. An account now holds up to ten concurrent sessions, the eleventh ends
the least recently used, and a person can list their own sessions and end one
or all of them (`mySessions`, `endSession`, `endAllSessions`). Exactly one
client is at the table; the rest are companion surfaces.

**Two-factor is a whole lifecycle, as of 2026-09-09 (spec 041).** It was
listed as shipped for a long time on the strength of a verifier and a policy
switch, which was true of nothing a person could reach. What exists now:

- **Three entrances, one flow** — `/settings/security`, first-run setup, and
  the sign-in itself, where the login challenge doubles as the enrolment
  ticket so a policy cannot lock out an account that never enrolled by hand
  (FR-019, ADR-082).
- **Ten recovery codes**, Argon2id-hashed and shown exactly once, because the
  store cannot produce them again (ADR-083).
- **A deliberate way off** — `POST /authentication/2fa/disable`, costing
  password **and** possession, which is what adding one cost. Starting an
  enrolment is not a way off: a pending secret lives beside the live factor
  rather than on top of it (ADR-081).
- **Replay closed** (FR-016). A code is bound to the step it matched, claimed
  by a conditional `UPDATE`, so the same six digits cannot be used twice
  inside their own ninety-second window.
- **Bounded guessing** (FR-017), per account rather than per address, applied
  inside the shared verification helper so it covers every route that checks
  the credential rather than the one it was written for.
- **An administrator must hold one**, computed from the role rather than
  stored, so it cannot drift (FR-027, ADR-094). First-run setup does not
  complete without one.
- **A defined path for somebody who has lost everything** — an operator reset,
  recorded with the operator's own id against it, which replaces a `psql` edit
  against a live table (FR-024). It cannot enrol and cannot issue codes.
- **A record and a notice.** Every change is written in the transaction that
  made it; the account holder is told by mail where an instance has any, and
  by their own security page where it has none — which is most small
  self-hosted ones (FR-015).
- **One refusal.** Every credential refusal on the verification path says the
  same thing, because "that code was right but already used" confirms an
  intercepted code was genuine (FR-018).

**First run now sets an instance up, and says what it did not.** As of
2026-09-07 an empty database is walked through one pass — administrator
account, operator identity, the contact for copyright notices, the support
address, mail, a second factor, review — with the steps derived from the
settings registry rather than hard-coded, so a declaration added later
appears without the wizard being edited. A setting the environment has
already fixed is shown as fixed and not asked for, which takes a
fully-configured container to the review screen in two steps. Setup no longer
ends when the administrator's account is created (ADR-093); it ends at
`/complete`, which is what makes the second half of the wizard reachable at
all.

Proven end to end against a migrated-but-unseeded database — a lane that had
to be built, because every other e2e stack is cloned from a seeded template
where setup is already finished and first run is therefore unobservable.

**Everything collected at setup is changeable afterwards, from a screen.** As
of 2026-09-08 the administration area carries Instance, Readiness and Mail
sections. Instance lists every declared setting in its registry group with
where the value came from and its change history; a value the environment has
fixed shows the variable that fixed it rather than a disabled box with no
explanation. Readiness says what this instance can and cannot do, derived on
every read, with the registry's own sentence for each gap. Mail sends a test
message through the real outbox and shows that outbox, with no message body
anywhere on the screen. The precedence rule — environment, then instance row,
then declared default — is the same for every setting in the product, resolved
per request, so a change takes effect without a restart. See
[docs/INSTANCE_CONFIGURATION.md](docs/INSTANCE_CONFIGURATION.md).

**Mail is proven end to end, against a real SMTP server.** Configure it in the
admin screens, send a test, receive it in a per-shard Mailpit; break the port
and the failure names what to fix without ever printing the password; clear the
settings and the next message is *held* rather than discarded; reconfigure and
it leaves on its own, because the operator fixed the settings and that should
be all they have to do.

**An instance with nobody to serve a copyright notice on cannot publish beyond
a world** — share links are refused, naming what is missing, while playing,
editing and sharing *inside* a world are unaffected. A link minted while the
instance was configured keeps resolving afterwards: removing a setting must not
break links already given out. This is spec 039's FR-053, built here.

**Terms disputes and privacy requests have a channel.** The terms and privacy
pages take a form and publish no address; submissions need no account, because
somebody objecting to the terms is often objecting to the terms they were asked
to accept. The copyright page keeps its published designated agent, because 17
U.S.C. § 512(c)(2) conditions the safe harbour on it being publicly available —
the form is first and the designation sits at the foot of the page.

**Feedback is how MVP finds out what is wrong with it.** As of 2026-09-09 a
person reports a problem from any screen, reviews exactly what will be sent —
message, context, a redacted log bundle, optionally a screenshot they took
themselves — and the instance keeps it. Redaction happens in the client before
the review, from one rule list the server also reads; a payload still carrying
a secret is **refused** and never quietly rewritten, because a server that
cleaned up a submission would be delivering something other than what was
approved.

The instance is the record. Delivery to a tracker is a separate step that can
fail, be abandoned, or never be configured, and none of that unsends the
report — which is why the author is shown *received* and never a delivery
failure. Both sides now have a screen: the author's at Settings → Feedback, the
operator's undelivered queue under Admin → Legal, and the operator's shows no
message and no attachment because the server does not send them.

Delivery runs end to end against a stub the server reaches through
`GITHUB_API_BASE` — the same configuration GitHub Enterprise uses — so there
is no test branch in the delivery path. What that proves that unit tests could
not: an attachment is committed **before** the issue referencing it, and an
ambiguous failure produces exactly one issue because the retry searches for the
delivery key rather than creating a second. See
[docs/FEEDBACK.md](docs/FEEDBACK.md).

**Not built**, re-checked 2026-10-13: the by-hand quickstart passes (037 T078,
040 T111–T112, 041 T078 are the open rows in their ledgers). The
credential-resolution screens this document listed as missing are built —
spec 037 US4, `apps/web/e2e/feedback-credentials.spec.ts`. Attachments are
kept for 30 days; the sweep (`feedback/schedule.rs`) is unit-tested
(`the_sweep_takes_expired_evidence_and_leaves_the_report` in
`feedback/deliver_tests.rs`) and has never been watched end to end.

**The password-change path exists.** An earlier revision of this section said
there was none; spec 036 built one — `POST /authentication/password`
(`auth/password_change.rs`), which ends every other session for the account,
proven in `apps/web/e2e/concurrent-sessions.spec.ts`. The stale claim still
lives on in a comment in `auth/session_registry.rs`.

**Admission is a policy, since 2026-09-06.** `instance_access_settings` holds
`open`, `invite_only` or `closed` (spec 035, ADR-072; fresh installs start
invite-only), one gate covers local registration and OAuth sign-in
(`auth/instance_access.rs`), and since `e54c6893` the environment can pin the
policy (`THUNDERFORGE_INSTANCE_ACCESS_POLICY`, unknown values fail shut to
closed). Spec 064 made first run ask only what that mode needs — a private
instance is not asked for copyright-notice details
(`instance-first-run-private.spec.ts`, `instance-first-run-public.spec.ts`;
52/52 tasks, lane green 2026-10-12). Keycloak-style providers take an issuer
URL (`2026-10-02-090000-0000_oauth_provider_issuer_url`, spec 064 FR-032): the
endpoints are built from it by a known path pattern, which is not OIDC
discovery.

### [x] Phase 2 — World creation

`createWorld` with `game_system_id`/`interface_pack_id`, plus listing and
deletion. Since spec 088 (109/109) it also takes a `baseMapId`, so a new world
can open on one of the example maps instead of a blank scene
(`apps/web/e2e/world-base-map.spec.ts`; the maps are MBRound18's own CC BY-SA
4.0 work, credited in `examples/maps/credit.json` and shown by
`MapCredit.tsx`).

**Grown around it, verified 2026-10-13.** A world now has a book list
(`2026-09-13-100000-0000_world_book_list`, spec 050): which books on the
owner's shelf it has switched on, nothing copied, and a trigger that refuses
anyone else's shelf. The shelf itself comes from spec 049 (105/106, the open
row is the by-hand quickstart on a real book; `book-import-*.spec.ts`, slice
`book-import`), and a collection is a `compendiums` row of origin `Authored`
with its earlier versions kept
(`2026-09-16-220000-0000_collections_on_the_shelf`,
`2026-09-16-230000-0000_collection_versions`; slice `collections`). Spec 074
is a world to try: the demo under `apps/demo`, which runs the visitor as Game
Master entirely in the browser — nothing leaves it, and it is not a shared
table. Its ledger stands at 7/12.

### [x] Phase 3 — Scene creation

Scene CRUD, `grid_size`/`grid_type`, background art by `.dd2vtt` import or
paste-to-canvas (spec 002, on RustFS asset storage). Spec 022 overhauled scene
management and made which scene a world is _on_ server state rather than a
per-client selection (ADR-046).

**Since the last pass, verified 2026-10-13.** A scene has an ambient light of
bright, dim or dark (`2026-09-10-140000-0000_scene_ambient_light`) and an
exploration switch (`2026-09-12-090000-0000_scene_exploration`, spec 045 US7):
the server holds only on/off and a reset epoch, what a player has seen stays
in their own browser. A scene made from an example map carries that map's
credit (`2026-10-10-110000-0000_base_map_assets`, spec 088). Walls and lights
snap to the grid or to wall ends, toggled with S (spec 077, implemented
2026-10-06, `apps/web/e2e/wall-snapping.spec.ts`), and a wall that ends on
another joins it (spec 078 FR-001 to FR-004). The engine and large scene
assets download in resumable parts (spec 080, 36/36,
`resumable-downloads-*.spec.ts`).

Not built: a map that arrives in pieces (spec 059, ADR-104 still proposed —
the import is one request in `map_import/mod.rs`) and a map in tiles (spec 075;
`MAX_CANVAS_TEXTURE_DIMENSION` in `storage/transcode.rs` still caps a
background at 4096). Both are drafts with no tasks.

### [x] Phase 4 — Token creation

Tokens place, move and bind to actors. Spec 004 unified the backing store onto
the scene-scoped `tokens` table (ADR-040) and split player- from GM-initiated
moves (`moveOwnToken` vs `updateToken`). Spec 006 replaced a keyboard-shortcut
stand-in with real canvas-rendered resize and rotate handles.

**Closed since the last pass**: tokens now have a kind.
`thunderforge_canvas_core::token_kind::TokenKind` defines Character, NPC,
Vehicle and Object, parsed server-side (`parse_token_kind`) and rendered with a
per-kind palette in the engine (`token_kind_color`, now in
`crates/thunderforge-engine/src/payloads.rs` since `ea104c76`).
The "no distinct token type or visual representation" gap this document
recorded is gone.

**Since the last pass, verified 2026-10-13.** A token is linked to its actor
or a detached copy with its own `system_data` (`2026-09-14-210000-0000_token_links`,
spec 046), and `tokens.health`/`max_health` are retired into
`retired_token_health` because HP lives on the sheet
(`2026-09-14-210100-0000_retire_token_health`, ADR-102). The old
`world_tokens` table is dropped outright
(`2026-09-15-090000-0000_drop_world_tokens`); only a comment and a test
module still say the name. A token's name can be hidden from players
(`token_name_visibility`), an NPC is invisible to players by default
(`npc_visible_to_players`), and a world decides whether players may set
their own actor's art (`player_actor_art`). Since `5191e738` (2026-10-12)
token creates and deletes go through the world store with a client-minted
id, the way moves already did (`apps/web/e2e/canvas-context-menu.spec.ts`).
The board takes touch (spec 069, `canvas-touch.spec.ts`) and says when its
engine has stopped (spec 070, `canvas-engine-stopped.spec.ts`). The hero
builder (spec 044, 103/103) makes an actor and its art from a form
(`hero-builder-*.spec.ts`, slice `hero-builder`). Spec 056 was never built as
its own spec; 071 and 073 cover its right-click asks.

### [~] Phase 5 — Actor stats and customisation

Actors carry system-defined stats (`world_actors`,
`world_actor_system_data`), bind to tokens via `tokens.actor_id`, and have a
dedicated UI (spec 010): view/edit routes, a PC/NPC flag, an NPC catalog on
the staging page, and share-and-deep-copy of an actor into another of your own
worlds.

**Done — stats reach the canvas.** Spec 029 draws a token's resources as bars
above it (`crates/thunderforge-engine/src/plugins/status_display.rs`) and shows the selected
token's in a viewer-chosen corner (`StatusPanel.tsx`). Which resources exist is
declared by `system.json`, never hard-coded — four real systems ship
declarations (Genie, D&D 5e, Pathfinder 2e, Blades in the Dark), and the engine
understands none of them, which is precisely what the four-system e2e exists to
prove. Coarsening for viewers not entitled to exact figures happens
server-side (`crates/thunderforge-server/src/status_display.rs`), so a withheld value never
reaches the client. See `docs/status-displays.md`.

Capacity was measured, not assumed (SC-006): **3,200 tokens at 30fps with
displays on — the same as with none.** Before off-screen culling the same board
carried 16,003 sprites against 3,203 and ran at 20fps. Figures are generated
into `marketing/engine-status-capacity.json`, never transcribed.

**Re-verified 2026-10-13: a computed stat is now read, charged and shown —
and still never enforced.** The previous pass found `movement_budget::cost_path`
implemented with no caller. Spec 046 gave it one: every committed move —
`moveOwnToken`, a GM's `updateToken` drag, and offline replay
(`graphql/mutations_reconcile.rs`) — passes through
`crates/thunderforge-server/src/combat/budget.rs::spend_for_move`, which
resolves the actor's walking speed from its sheet through the system's
`movement` block in `system.json` (5e's `walk` reads `speed_walk`), prices the
route with `thunderforge_combat::budget::move_cost` → `cost_path`, and writes
the spend to `world_combatant_budgets` (migration
`2026-09-14-230000-0000_combatant_budgets`). The dock shows what is left
(`apps/web/src/components/world/PlayDock/CombatantBudget.tsx`), and
`apps/web/e2e/combat-economy.spec.ts` proves it in a browser.

Three limits, all deliberate and all recorded in the code rather than
discovered here:

- **Charged only while a combat is running in that scene.** Outside a fight
  `spend_for_move` returns before it prices anything. There is no
  out-of-combat speed.
- **Shown, never refused.** The module header says so in those words, and
  spec 046's decision 2 is why: a spend past the allowance is a *debt*, shown
  as `overspent`, never a rejected move. The same holds for reach and range
  (`combat/reach.rs`): flagged on the attack record, never refusing it. So
  "gating movement on a computed speed", which this document once assigned to
  Phase 8, is now a decision to overturn rather than a feature to add.
- **The engine knows none of it.** `PlannedPath::cost_in_cells` still counts
  one cell per step (`crates/thunderforge-canvas-core/src/movement.rs`), the
  engine has no `cost_path` caller, and `DerivedStats.movement_speed`
  (`crates/thunderforge-engine/src/components.rs:166`, now an `Option` that
  nothing sets) is dead. Difficult terrain has no source: every step is priced
  as open ground.

What *does* refuse a player's move is the turn check (`turn::turn_check`) and
walls: `judge_against_walls` in `graphql/mutations_tokens.rs` answers "A wall
is in the way", and since migration `2026-09-16-090000-0000_walls_block_movement`
a wall blocks movement by default (spec 045, `apps/web/e2e/token-movement-walls.spec.ts`).
A GM's drag is not judged, by FR-017.

**Also closed by specs 045 and 046.** Keyboard movement — WASD and arrows,
Shift to extend a route, Space to commit — is wired in
`crates/thunderforge-engine/src/systems/token_move.rs` and re-sent from the
window by `apps/web/src/engine/canvasKeyboard.ts` so keys do not die when
focus leaves the canvas (`apps/web/e2e/token-keyboard-move.spec.ts`). Hit
points, temporary hit points and zero-means-out live on the actor, with a
token either *being* its actor or a copy of it (`2026-09-14-210000-0000_token_links`,
`…_retire_token_health`, ADR-102). Attacks resolve on the server against
armour class with an offer the GM takes or declines
(`combat-attack.spec.ts`, `combat-hit-points.spec.ts`). Conditions exist
(`2026-10-04-120000-0000_world_actor_conditions`, spec 067) and are drawn;
nothing in `thunderforge-combat` reads them, so they are display only.

**Characters arrive from their sheets (spec 048, 98/98 tasks).** A D&D Beyond
PDF or a Roll for Shoes sheet is read in the browser and again on the server
by the pack's own `sheet/` crate (ADR-115), reviewed before it lands, re-read
on re-import for only what changed, and rolled back by the GM
(`apps/web/e2e/sheet-import-*.spec.ts`, the `sheet-import` slice). The
`playtest/bring-a-character.playtest.ts` run of 2026-10-10 carries one soft
finding: a spell that comes off a sheet with no dice cannot be cast until the
GM gives it an effect.

### [x] Phase 6 — Walls and lighting

Walls, doors and light sources with 2D vision occlusion, and hand-drawn wall
and shape authoring on the canvas (specs 001, 002).

Spec 003 closed the round-trip trust gap: the map importer (now the directory
`crates/thunderforge-server/src/map_import/`, split in `6f8ed43f`; the tests
are `map_import/tests.rs`) re-queries the
database after an import — and after hand edits on top of one — and asserts
exact field equality against the source across the richest real fixtures, so
nothing is silently lost or altered by a reload. It also surfaced previously
silent field categories as import `warnings` (freestanding portals, non-default
`ambient_light`, `objects_line_of_sight`).

Live cross-session sync works: `apps/web/src/pages/world/useSceneLiveSync.ts`
opens the `worldEventsCreated` subscription once per mounted scene and feeds
the wall, token, shape, light, status, interactive, vision, grid and
scene-lighting appliers from it, each filtering by its own event code.
`WorldPage.tsx` only calls the hook now. A second, world-scoped subscription
in `useWorldEventStreams.ts` carries scene selection, pause and rolls; its
comment says two open subscriptions is the accepted trade.

**Since the last pass, verified 2026-10-13.** A light has a bright reach and
a dim reach (`2026-09-15-140000-0000_light_bright_radius`); walls block
movement by default (`2026-09-16-090000-0000_walls_block_movement`, spec 045,
see Phase 5). Spec 076 (implemented 2026-10-06) reworked sight: line of sight
hides the map even in a bright scene, never-seen areas are black, a light the
viewer cannot see lights nothing, and a GM who selects a token sees what it
sees (`systems/lighting_vision.rs`, `plugins/darkness.rs`;
`scene-lighting.spec.ts`, slice `lighting`). Doors, lights and drawings have
right-click menus (specs 071, 073; `interactive-door-menu.spec.ts`,
`canvas-light-and-drawing-menu.spec.ts`). Players draw their own shapes
(spec 082, 46/46, see Phase 10). Hidden walls use the existing `walls.secret`
and the board has box select (spec 085, 37/37, `canvas-box-select.spec.ts`).

### [~] Phase 7 — Scene levels

Upstairs and downstairs, each level with its own walls and token assignments.

**Built on 2026-10-04, in two commits** (`0ef38b66` the server, `ad98425b`
the board), without a spec folder of its own — which is why this document
still said "not started" six weeks later. What exists, verified against the
code:

- **Schema.** `crates/thunderforge-server/migrations/2026-10-04-090000-0000_scene_levels`
  adds `scene_levels` — a name, an order, one entry level per scene, and a
  board of its own (background, size, ambient light) — and a `level_id` on
  `tokens`, `walls`, `light_sources`, `shapes`, `interactives` and
  `fog_masks`. Every existing scene was backfilled with a "Ground" entry
  level, and a trigger puts an insert that names no level onto the entry
  level, so nothing that existed before levels had to learn about them. Fog is
  now per level.
- **Server.** `graphql/mutations_levels.rs` (create, update, reorder, delete,
  `moveTokensToLevel`; up to twelve per scene), `level_travel.rs` for stairs,
  and `auth/level_visibility.rs`: a GM reads every level, a player reads only
  the levels where they control a token — or the entry level if none — and a
  level they may not read is answered as if it did not exist. Event codes 33
  (`SCENE_LEVEL_CHANGED`) and 34 (`TOKEN_TRAVELLED`, whose payload
  deliberately names no level).
- **Travel.** A `nav.travel` interactive with a partner: walking into a stair
  region travels inside `moveOwnToken`; a door or prop travels through
  `activateInteractive`. A GM's drag never travels. Arrival counts as a
  placement, so a token cannot bounce straight back down.
- **Board.** `apps/web/src/components/world/LevelTabs/` gives the GM a tab per
  level with a token count and a manager (rename, reorder, make entry, set
  the background, move tokens, delete); `api/viewedLevel.ts` makes every
  per-scene read level-aware; `engine/world/sync/levels.ts` applies events 33
  and 34. The engine itself has no level concept and needs none: switching
  level reloads the board the way a scene switch does
  (`pages/world/useSceneLevels.ts`), so vision and lighting are per level
  because each level's data is loaded alone.
- **Proven.** `apps/web/e2e/scene-levels.spec.ts` (the `scenes` slice): a
  player who walks onto the stairs is taken upstairs and *only they are* — the
  other player stops seeing them and cannot read the upper floor, the GM's
  tab count moves, a reload keeps them upstairs, and a GM drag across the
  stairs moves nobody.

**Not done**, and the reason this is `[~]`:

- Clicking a door or prop to travel: the server accepts a `tokenId`, the web
  never sends one (`CanvasContextMenu/doorActions.ts`,
  `engine/world/sync/interactives.ts`), so the server answers "Choose which
  token travels". Only stair *regions* travel from the board today.
- Drawing a stair region: `components/InteractionAuthor/RegionDraw.tsx`
  exists and nothing imports it. The e2e creates its regions over GraphQL.
- Combat does not check that two tokens share a level.
- `.dd2vtt` import writes only to the entry level
  (`map_import/scene_write.rs`).
- No spec or task ledger owns the feature; spec 088 US6 (edge walls for a
  level's background) is the only spec that mentions it.

### [~] Phase 8 — Game system integration

Shipped: the system package install and manifest-serving pipeline
(`crates/thunderforge-server/src/systems.rs`), a registry with eight server
pack crates plus two `sheet/` reader crates (root `Cargo.toml`,
`apps/thunderforge/src/system_packs.rs`, whose test asserts each one is
discovered), and server-side per-system validators that already enforce real
rules — Pathfinder 2e rejects an ability value outside `-5..=10` because it
stores modifiers where D&D 5e stores raw scores.

**Where each system stands, re-verified 2026-10-13.** The owner's bar is that
D&D 5e and Roll for Shoes are field-test ready and the rest keep working.
Against the manifest contract in `packs/systems/README.md` (the key list is
enforced by `scripts/check-packs.mjs`):

| System | Declares | Code beyond validators | Proof |
| --- | --- | --- | --- |
| `dnd5e` | abilities, resources, movement, vision, combat, conditions, checks (30), settings, skills, damage types, item properties, ability vocabulary, sheet import | rules, roll facets (spec 084, 72/72), stat blocks, content refine, a `sheet/` reader (spec 048), a web sheet and `statRolls` | 61 e2e files: `dnd5e-sheet`, `rolls-facets-*`, `rolls-5e-stats`, `roll-check`, `combat-*`, `sheet-import-*`, `system-world-settings` |
| `roll_for_shoes` | resources, sheet, starting skills, sheet import | adjudicate (rolls judged on the server, XP paid in the same transaction), its own settings and table-difficulty tables (ADR-063), a `sheet/` reader, a web sheet and three panels; specs 061 (35/35) and 062 (57/57) | `system-roll-for-shoes.spec.ts`, `-extras`, `-table`, `sheet-import-second-system`, `system-world-create` |
| `genie` | abilities, facets, combat, conditions, movement, wish points | rules, a session domain owning six tables and its GraphQL, a web sheet and four panels | about thirty `genie-*` and `status-*` files |
| `pathfinder2e`, `blades_in_the_dark` | abilities, resources, movement, sheet | validators only | `status-systems`, `token-attributes`, `abilities-vocabulary`; Pathfinder also in `world-appearance` |
| `fate_core`, `cypher_system` | sheet (tracks, ladders, slots), groups | validators only | `world-appearance.spec.ts` renders each through `forge`; Fate also in `sheet-import-refusals` |
| `year_zero_engine` | abilities, resources, sheet | validators only | **no e2e at all** — a registration test and the build-time `check-packs` are its whole evidence |
| `basic-game-system` | `template: true` | — | not offered in any picker, by design |

So 5e and Roll for Shoes meet the bar on the code and the e2e that exists;
Year Zero is the system with the least behind it. Spec 049's SC-002 and SC-004
(a real source book, by hand) are the one 5e item left open.

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

1. **Engine-side rules.** A pack cannot load code into a running wasm engine
   the way its server crate links into the binary. ADR-062 decided packs extend
   the engine with data, and spec 066 removed the `engine/` crates that nothing
   ever loaded.
2. **Validation is not enforcement — and enforcement is now a decision, not a
   gap.** A computed speed is read and charged on every move in combat, and
   never refuses one, by spec 046's decision 2. See Phase 5.
3. ~~**A vestigial trait.**~~ Resolved by spec 032: `crates/thunderforge-engine/src/systems/core.rs`
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

**Spec 033 gave a system the words for its abilities.** The compendium was a
flat list with a "Type" column you had to read down to find your spells; it is
now a tab per type, in the system's plural word for it and the system's order,
each carrying its own count. A 5e GM has Spells and Feats, a Genie GM has
Scrolls and Knacks, and the umbrella term replaces "Abilities" everywhere a
person reads it.

The set of types is no longer fixed. `world_abilities.classification` carried a
CHECK constraint listing four values; it is dropped, the GraphQL enum and the
TypeScript union with it, and validity is now what the world's assembled
vocabulary says — enforced at the mutation, where the world's system is known
and a table-wide constraint could never see it. `packs/systems/dnd5e` declares
an item-bound Enchantment that no shared file mentions, which
`scripts/check-ability-vocabulary.mjs` enforces.

A type declares what it binds to and whether it is graded, and both are refused
server-side. An ability whose type the active system stops recognising is not
lost, renamed or re-typed: it moves to a marked tab under the identity it was
authored with, and returns to its own tab when that system is active again.

**Changing a world's system is no longer one click.** It counts what is at
stake, says in red that the content becomes hidden rather than destroyed, and
asks twice — with the server refusing an unacknowledged or stale request, so
the guard holds against a direct call.

**A pack now contributes behaviour — closed by ADR-063 on the day it was
accepted (2026-09-03).** The previous pass recorded that world creation
branched on one system's name to insert its session row, because that pack
could not own the table it wrote while 2,763 lines of its GraphQL lived in the
server. Those lines were Genie's, and they are in the pack: the server
compiles as a library, `packs/systems/genie/server/src/session/` owns its six
tables and their GraphQL (`apps/thunderforge/src/schema_roots.rs` merges
them), and `createWorld` calls `world_hooks::run_world_created`
(`crates/thunderforge-server/src/world_hooks.rs`), which dispatches through
`inventory` to whichever pack registered a `WorldCreatedHook`.
`scripts/check-system-registry.mjs` carries an empty exemption list. What a
server crate may contribute is one `SystemContribution`
(`crates/thunderforge-canvas-core/src/system_contribution.rs`): slot
validators, rules, `refine_content`, `world_setting`, `adjudicate`,
`roll_facets` and `sheet_import`. A failing pack surface is contained and
names its pack, and `packs/systems/README.md` is the published contract.

Since then: per-world system settings are declared rows
(`2026-10-04-100000-0000_world_system_settings`, spec 067, ADR-112 — 5e's
`inspiration` is the first), a pack's `sheet/` crate reads a character sheet
(spec 048, ADR-115), and roll facets — advantage, inspiration, rerolls, item
properties — are a pack contribution (spec 084, migration
`2026-10-07-120000-0000_roll_facets`). The one ADR-063 step still open is
moving a pack's migrations into the pack; they live under the server's
`migrations/` today.

### [x] Phase 9 — Multiplayer

Invite codes and shareable links, `joinWorld`, `world_members` and
`world_invites`. Spec 027 unified access links and deliberately gives every
dead link the _same_ message, whatever the cause — telling the holder which
cause applied is exactly what the server refuses to disclose (FR-011/SC-005).

**Closed since the last pass**: one account can now be signed in on several
clients at once (spec 036, ADR-073), which was the defect that interrupted a
demo — a second browser used to sign the first one out. Exactly one of those
clients holds the play field; the others are companion surfaces, and peer
reachability belongs to the play field alone (ADR-075), so a companion cannot
become a second peer endpoint for one person.

**Also closed by spec 036**: an account can see and end its own sessions
(`/settings/security`), each named coarsely by the client that opened it —
"Firefox on Linux", never an address. Ending one now closes the live streams
that session holds rather than leaving a revoked client receiving world events
until its socket dropped. And the product has a **password-change path** for
the first time: `password_hash` was previously written at registration, admin
bootstrap and OAuth provisioning and never again, so somebody who believed
their password had been seen had no remedy. Changing it ends every other
session for that account.

**Turn structure is now proven in a browser.** The shared initiative tracker
had fourteen Rust tests and nothing that pressed a button;
`apps/web/e2e/combat-panel.spec.ts` drives the panel for real — start, roster,
initiative, advance, wrap into a round, end — with a player following the
round and the active combatant without reloading, refused when they try to
advance it, and a Game Master's *own second window* both following the fight
and able to drive it. That last case could not be written before ADR-073: a
second window used to sign the first one out.

**Also closed**: GM override of character selection, previously
recorded here as unaudited, is delivered by spec 017. A GM may un-claim any
character at any time under their existing Owner-level authority over every
actor in their world (spec 010's DM-always-full-control rule), and the
un-claimed player keeps their membership and returns to the "no character
selected" state. Covered by `apps/web/e2e/actor-claim.spec.ts`, including two
players racing to claim the same character with exactly one winner.

**Since the last pass, verified 2026-10-13.** Every seat hears a roll: spec
081 (47/47) sends rolls to the whole table, with GM-only and GM-eyes rolls and
a reveal (`apps/web/e2e/rolls-everyone.spec.ts`, `rolls-gm-only`,
`rolls-gm-eyes`, `rolls-reveal`), and spec 083 (68/68) draws the dice in the
engine landing on the server's result (`rolls-dice-on-screen.spec.ts`,
`rolls-dice-frame-rate.spec.ts`). Damage reaches every seat's bars inside one
second, asserted rather than annotated (spec 046, `e2e/fixtures/seatTiming.ts`).
An operator can pause a world's play and lift it, and the table is told
(spec 051, 74/74, migration `2026-09-13-170000-0000_pausing_a_worlds_play`,
ADR-100; the five `play-pause*.spec.ts` files and
`journeys/operator-pauses-through-the-portal.journey.spec.ts`). An invite's
use limit is optional (`2026-10-10-100000-0000_world_invites_optional_limit`,
spec 088 FR-012): the old default of 0 was a limit the CHECK constraint
refused.

Still exactly one play field per account: `play_field.rs` keeps one claim,
there is no release mutation, and `peer_signaling/admission.rs` refuses a peer
connection from a companion.

### [~] Phase 10 — Permissions model

**Four ranked roles, not three, as of 2026-09-13.** `world_members.role` is
Owner, GM, TrustedPlayer or Player (migration
`2026-09-13-110000-0000_trusted_player_role`, ADR-099), ranked in
`crates/thunderforge-authz/src/role.rs` and backed by a capability matrix in
`crates/thunderforge-authz/src/capability.rs`. A Trusted Player differs from a
Player in exactly one capability, `ManageBookMaterial` — switching books on
the world's list, editing the world's changes to their entries (spec 050), and
adopting what players bring in (spec 048) — and the matrix's own test pins
that difference. GM is already the "assistant GM" this document used to ask
for: full content authority, cannot delete or transfer the world. Proven by
`apps/web/e2e/library-deltas-roles.spec.ts`, `library-book-list.spec.ts` and
`sheet-import-adopt.spec.ts`.

`updateMemberRole` now has limits (`crates/thunderforge-core/src/models/invites.rs`):
nobody hands out a role above their own, so only an Owner promotes to Owner,
and a GM manages only members ranked below GM.

Spec 010 added a real per-object layer: an ownership block granting any member
Viewer/Editor/Owner on any actor, with the DM implicitly retaining full
control and a default-Viewer fallback. Enforced server-side
(`auth/actor_permissions.rs`, gating `updateActorSystemData` and
`moveOwnToken`). The same shape covers lore, items and abilities
(`world_*_permissions` tables), and since spec 063 (ADR-110, migration
`2026-10-03-090000-0000_claim_grants_editor`) claiming a character grants
Editor on it and releasing the claim takes back only what the claim granted
(`apps/web/e2e/actor-claim.spec.ts`).

**Players hold authoring tools per member.** Spec 031 FR-046 added
`world_authoring_tool_grants` — a GM may grant any of select, walls, lights,
shapes, tokens, interactions (`auth/authoring_tools.rs`,
`setAuthoringToolGrant`, `AuthoringToolGrantsCard.tsx`) — and spec 082 turned
select and shapes *on by default* with a revocations table to take them away
(`2026-10-07-110000-0000_authoring_tool_revocations`,
`apps/web/e2e/canvas-shapes-by-players.spec.ts`). Granting walls or lights to a
player by name has only the server's surface table behind it
(`play_pause_surface_tables.rs`), no e2e.

**Still missing**, checked by grep on 2026-10-13:

- No ownership or visibility block for scenes or maps: nothing named
  `scene_permissions` exists; scene writes go through `is_dm_of_scene`.
- No general policy system. `permission_grants` turns out never to have been
  a table at all — the name survives in `docs/SECURITY_RBAC.md` as history.
  `policies` *is* still migrated (`2026-05-02-022200-0000_create_policies`)
  and declared in `schema.rs`, and its only reader is the admin dashboard's
  row count (`admin.rs`); every other reference is commented out or stubbed.
- `createWorld` still inserts no Owner row in `world_members`
  (`graphql/mutations_worlds.rs` says so in a comment); membership checks fall
  back to `worlds.created_by`.
- `docs/SECURITY_RBAC.md` is dated 2026-08-20 and knows none of the above —
  it still says scene-object writes are scoped to `scenes.owner_id`, which
  they no longer are.

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
  Unchanged on 2026-10-13: no `bevy_audio` in any Cargo file, and spec 038
  (scene audio) is still a specification only. Spec 058, "a table that hears
  what happened", is not audio — it is a proposal, nothing built, about more
  kinds of action reaching the table's feed.
- **Multi-scene navigation.** `nav.request_scene` raises a request, the GM
  approves or refuses, and the requester is told. Then nothing moves anybody,
  because there is nothing yet to move them with. The request and the decision
  are the parts this feature owns, and they work. (Still true on 2026-10-13:
  `crates/thunderforge-canvas-core/src/navigation.rs` says so in its header.
  The one effect that *does* move a token is `nav.travel`, between the levels
  of one scene — see Phase 7 — and `mutations_interactives_support.rs` moves a
  token on approval only when `level_travel::is_travel` says so.)
- **Party tokens.** A GM-controlled token the whole party sees and follows — a
  ship, a caravan — for world-map scenes. Named so the model does not preclude
  it; not built.

## Health of the test suite

**The unit of proof is a slice, not the suite** (ADR-107, constitution
Principle VI, `docs/CONTRIBUTING.md` "Proving a change"). A change is proven
by `pnpm e2e:<slice>` for the slices `pnpm e2e:which --diff` names;
`scripts/check-e2e-slices.mjs` runs in `pnpm verify` so a spec file with no
owner fails the check. `scripts/e2e/slices.json` declares 32 slices over the
249 `*.spec.ts` files under `apps/web/e2e`. The full `e2e-parallel` run is on
the order of four hours and nothing gates on it.

`docs/e2e-suite-health.md` is from the spec 030 era (last touched
2026-08-30, 202 tests and 42 failures) and reads as history now; no
`.e2e-history.jsonl` exists. What the files record instead, checked
2026-10-13:

- `scripts/e2e/slice-durations.json` (`9e46385a`, measured 2026-09-22/23):
  27 slices, 204 spec files, **541 passed, 0 failed, 0 flaky, 1 skipped**,
  149.7 minutes of wall time summed. Five slices had no duration then
  (landing, resumable-downloads, rolls, sheet-import, telemetry).
- Spec 048 T098 (2026-10-10) ran 19 slices one at a time, every one 0
  failed — sheet-import 46, accounts 69, actors 30, book-import 17, canvas 44,
  collections 31, combat 26, companion 13, compendium 26, engine-other 9,
  game-systems 33, genie 21, instance 42, lore 13 (+1 skip), moderation 14,
  scenes 19, tokens 24, worlds 21 — except `resumable-downloads`, 7 passed
  and 1 failed (`resumable-downloads-scene`: its 16 KB part override against
  a 274 MB dev-profile engine, not caused by 048 and not fixed since).
- `4799da64` (2026-10-12): `e2e:actors` 32 passed in 7.8 minutes; `combat`
  26 of 27, the odd one passing alone.
- `abb9f45f` (2026-10-12): the nine journeys in `apps/web/e2e/journeys/`,
  16 passed, 0 failed, 0 skipped.
- Spec 089 (36/36, 2026-10-11): `cargo test` 2,205 passed on each of two
  worktrees at once, each on its own stack (`make e2e-stacks`, two
  concurrent slots); `book-import` and `feedback` ran side by side.
- Spec 091: `pnpm e2e:landing` 127 passed, 34 skipped, 0 failed, twice; a
  later rerun had 2 failures. The demo's own suite
  (`pnpm -F @thunderforge/demo e2e`) 51 passed.
- `pnpm playtest` (`apps/web/playtest/*.playtest.ts`, one worker,
  Chromium) on 2026-10-10: combat-5e and dungeon-crawl green;
  bring-a-character red on one soft finding (see Phase 5).

No file records a full-suite run after 2026-09-23. That is by design, and it
also means the honest full-suite number is "unknown since then".

Two rules earned the hard way, and worth more than any individual fix:

1. **Read `apps/web/test-results/<slug>/error-context.md` before theorising.**
   It holds an accessibility snapshot of the page at the moment of failure and
   usually contains the answer. A long session was burned stepping through a
   store, its props and the DOM while the snapshot said plainly that the page
   was showing a different scene.
2. **Give every failure a verdict** — stale spec, real product bug, or
   intentionally removed behaviour. Weakening an assertion to get green is the
   one outcome worse than leaving it red.

Always `--workers=1` for anything touching the engine (the playtest config
has it; `e2e-parallel` gets its parallelism from shards, not workers). On an
external stack, `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` too, or 429s dress
up as flaky tests.

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

Re-checked 2026-10-13: `apps/web/playwright.config.ts` has two projects,
`chromium` and `first-run`, both `Desktop Chrome`; the demo, hero-builder and
landing configs are the same. `crates/thunderforge-cache-browser` still rests
on OPFS, WebCrypto and IndexedDB. The only Firefox-family commits since the
last pass (`641e2a7d`, `edd9e46e`, 2026-10-05) fix a LibreWolf WebGL panic on
the demo's engine window and tell the person where that browser keeps its
WebGL switch; nothing added a Firefox project or spec.

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
- What _has_ been measured bounds the guess from below: 5.19MB brotli for the
  engine bundle (2026-10-09, on Bevy 0.20; 4.92MB on 2026-09-01 after spec
  031, 4.15MB before that), plus a world's art. Steady-state play is small — token moves
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

**Converting a character between game systems.** Wanted, not started, and not
for the first field tests. Today a character is always on its world's system:
the server refuses to create one on any other, because a 5e sheet at a Roll for
Shoes table is two rulesets with nothing on screen saying so. Conversion is the
deliberate way across that line — bring a character from one system to another
and say what carries over (name, portrait, notes) and what has to be rebuilt
(everything the rules define). It needs its own spec; the refusal in
`create_actor_impl` is marked `TODO(system-conversion)` where it would hook in.

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

**Re-measured for Bevy 0.20 (spec 087, 2026-10-09).** The release build was
30,424,557 bytes raw / 5,069,241 brotli -q11 on Bevy 0.19.1 and is
**31,939,040 raw / 5,186,179 brotli** on 0.20 — +2.31% on the wire for the
engine upgrade (`specs/087-bevy-0-20/spec.md`, `research.md`). Spec 091's
landing build recorded the same engine at 5,192,368 bytes on the wire
(`specs/091-landing-encounter/tasks.md`). No gzip figure has been recorded
since 2026-09-01, and `marketing/*.json` holds no bundle size at all — the
three files there (generated 2026-08-29 and 2026-08-30) are frame rates and
load tiers. The demo build writes an `engine.json` with raw, brotli and gzip
bytes (`apps/demo/src/build/engineManifest.ts`, `apps/landing/scripts/compress.mjs`),
and none has been committed, so the spec files above are the record.
`scripts/shared.mjs` and ADR-062 still quote 4.92 MB; the engine's
`Cargo.toml` comment says ~5.19 MB, which is the current one.

The dev profile is a different animal and is not the shipped figure: 0.19.1
dev was 274,300,485 bytes raw, and spec 087 settled 0.20 dev on
`line-tables-only` debug info at 335,575,245 raw / 14,610,549 brotli rather
than full debug info's 352,608,993 / 15,866,613 — `scripts/shared.mjs` sets
`CARGO_PROFILE_DEV_DEBUG` accordingly and folds it into the `pkg.sum` key.

### The bundle budget

**The engine is expected to grow as it gains features. The threshold of concern
is 100MB after compression.** At 5.19MB brotli (spec 087, 2026-10-09) there is
roughly 19x headroom, so an 18.5% rise across one spec, or 2.3% for an engine
upgrade, is growth, not a problem, and does not need re-litigating each time a
feature lands.

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
  first-load bytes on top of the release win. Now brotli and gzip
  (`CompressionLayer::new().br(true).gzip(true)`, which has moved to
  `apps/thunderforge/src/main.rs`).
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
  instantiated over a stable JS-glue boundary. The natural seam would be
  per system — a world needs only its active system — but no pack carries
  engine code today (ADR-062, spec 066). Real
  architectural work, and at 5.19MB brotli the case is still much weaker than
  it looked.
- **Open — Bevy feature trim.** `bevy_ui`/`bevy_ui_render` are unused (the
  debug HUD that needed them was removed) but deliberately retained — the
  Cargo comment says the win is small; `bevy_gizmos` is diagnostic-only and
  could be feature-gated. `webp` is *not* unreferenced, as this list once
  said: `crates/thunderforge-engine/Cargo.toml` records it as the format of
  DungeonDraft map art that the map importer accepts. None measured, and behind
  the two items above. The engine is on Bevy 0.20 (spec 087, 42/42).
