# Phase 0 Research: Interactive Elements

Decisions that had to be settled before the design could be drawn, each with
what was rejected and why. Two were already answered in the spec itself
(secrets are a table concern; effects are contributed rather than enumerated)
and are not re-argued here.

---

## 1. Where the effect vocabulary lives

**Decision**: Effect _declarations_ live in `thunderforge-canvas-core`, one
set contributed per subsystem module, assembled into a registry. Effect
_handlers_ live in the Bevy plugin that owns the subsystem. The server
validates against the declarations; the web app generates its authoring UI
from them via ts-rs.

**Rationale**: Three surfaces must agree on what effects exist — the engine
dispatches them, the server validates and persists them, and the web app
offers them in a form. Any design where each holds its own list guarantees
drift, and drift here is silent: a GM authors an effect the engine does not
handle and nothing happens at the table. That is precisely the failure spec
029 spent a whole user story retiring.

`canvas-core` is the one crate the server already compiles and from which the
web app's types are already generated. One definition, three consumers, and a
generation check already wired into `pnpm verify` to catch drift.

**Alternatives considered**:

- _Engine owns the registry, server stores opaque blobs._ Rejected: the
  server could not then validate an authored effect, and Principle III
  requires authorization and validation at the data boundary. It would also
  make the authoring UI depend on a loaded wasm engine to know what a GM may
  choose.
- _Server manifest declares effects, like game systems declare resources._
  Rejected: system manifests describe _content_ that varies per ruleset.
  Effects are capabilities of the build, and expressing them as data would
  let a manifest declare an effect no code can perform — reintroducing the
  dead-option problem from the other direction.
- _A Rust enum of effect kinds._ Rejected outright: an enum lives in one
  crate, so every subsystem gaining a triggerable capability would require
  editing the core. That is exactly the coupling Principle II forbids, and
  FR-039 exists to prevent.

---

## 2. How dispatch crosses the plugin boundary

**Decision**: `InteractionPlugin` writes a Bevy event carrying the effect
identifier and its configuration. Each contributing plugin adds a system that
reads those events and filters for the identifiers it declared. No direct
calls in either direction.

**Rationale**: Constitution II names this exactly — "cross-plugin
communication happens through Bevy events or shared resources, never through
direct calls into another plugin's private systems." It is also the only shape
that lets the interaction plugin compile with every contributor removed, which
FR-039 requires and US7 tests.

**Alternatives considered**:

- _A trait object registry of handlers_ (`Box<dyn EffectHandler>`). Rejected:
  handlers need `Commands` and arbitrary `Query` access to do their work, which
  a boxed trait cannot express without a service-locator shape that fights the
  ECS. Bevy's answer to "several systems care about a thing happening" is an
  event.
- _A shared mutable command queue._ Rejected: it is an event channel with
  worse ordering guarantees and no scheduling integration.

**Consequence worth stating**: an event is fire-and-forget, so "no contributor
handled this effect" is not detectable at the call site. FR-041 (an interactive
whose subsystem is absent must be reported to the GM) is therefore satisfied
_before_ dispatch, by comparing the stored effect identifier against the
registry, not by observing that nothing happened.

---

## 3. What open, closed and locked actually change

**Decision**: A door is a wall whose blocking is conditional on its state.
Open blocks neither vision nor movement. Closed blocks exactly what the wall's
own `blocks_vision` and `blocks_movement` say. `locked` is a separate boolean
governing who may change the state.

**Rationale**: The wall already carries a blocking profile, and deriving the
closed state from it means a closed window stays see-through and a closed stone
door does not, with no second set of fields to keep consistent. Making locked a
third state instead of a separate property would make "open, and players cannot
close it" inexpressible — a spiked-open portcullis is a real thing a GM
prepares.

**Alternatives considered**:

- _Open/Closed/Locked as one enum._ Rejected as above; it also forces a
  decision about what happens to the lock when a GM opens a locked door, which
  a separate flag simply does not raise.
- _Doors as their own geometry._ Rejected: `walls` already has `door_state`
  with `none`/`open`/`closed`. Two geometries for one line on the map would
  need reconciling on every edit.

---

## 4. Where regions live

**Decision**: A region is geometry carried on the interactive itself, not a
row in `shapes`.

**Rationale**: `shapes` is authored annotation — drawings the table sees, with
style and text. A region is invisible to players and exists only to be crossed.
Storing them together would mean every shape query filters out regions and
every region query filters out drawings, and `visible_to_players` would be
doing two unrelated jobs.

**Alternatives considered**:

- _Reuse `shapes` with `kind = "region"`._ Rejected for the reason above,
  though it is the cheaper migration.
- _A separate `regions` table._ Rejected as a table that would only ever be
  joined 1:1 with `interactives`; the geometry belongs to the interactive.

---

## 5. What a link effect may point at

**Decision**: A link effect references in-world content by identifier — a lore
entry id — not a free-text URL.

**Rationale**: This settles the spec's edge case about hostile destinations
without a URL allowlist, a warning interstitial, or a moderation surface. A GM
cannot point an interactive at an arbitrary address because the field does not
accept one. It also keeps the constitution's content guardrail untouched:
nothing here makes one world's content reachable from another, because the
reference is resolved within the world it belongs to.

**Alternatives considered**:

- _Arbitrary URLs with an allowlist._ Rejected: an allowlist is a moderation
  surface nobody has agreed to own, and the failure mode is a player being sent
  somewhere hostile from a table they trusted.
- _Arbitrary URLs with a confirmation prompt._ Rejected: it moves the judgement
  onto the player, who has the least context about where the link came from.

**Deferred, deliberately**: linking to a handout, image or journal that is not
a lore entry. The reference is typed, so adding a kind later is additive.

---

## 6. Where region entry is detected

**Decision**: In the engine, on token movement, comparing previous and current
containment. Only for movement the engine considers to be play — not for a GM
arranging a scene (FR-032).

**Rationale**: The engine already owns token position and movement, and is the
only place that knows both the previous and the current position within a
frame. Detecting entry server-side would mean the server inferring crossings
from position updates, which it receives as endpoints rather than as motion.

**Consequence**: "was this movement play or preparation" needs an explicit
signal rather than a guess. A GM dragging a token during preparation and a GM
dragging a token during play are the same gesture; the distinction is the
scene's mode, which the engine must be told.

---

## 7. Whether approval requests are persisted

**Decision**: A table, pruned rather than retained as history.

**Rationale**: The spec calls them transient, and in-memory would be the
lighter answer — but a GM who refreshes mid-session must not lose a pending
request, and a GM on a different device must see it. Presence was moved to
memory earlier in this project for a load reason that does not apply here:
requests are a handful per session, not a heartbeat per client.

**Alternatives considered**:

- _In-memory only._ Rejected for the refresh case above.
- _Retained as an audit log._ Rejected: nobody has asked what they would use
  it for, and retaining who asked to go where is a privacy surface without a
  purpose.

---

## 8. What a prop is

**Decision**: A prop is a row in `tokens` with the existing `object` token
kind and no actor. No new table, no new placement pipeline.

**Rationale**: Token placement, artwork, movement, ordering and live sync all
exist and work. A parallel "props" concept would duplicate every one of them,
and the token-kind palette already renders `object` in a deliberately
recessive slate so scenery does not compete with anything that acts.

**Consequence to watch**: anything that assumes a token has an actor must
already tolerate one that does not. Spec 029 established this — `tokenStatus`
skips actorless tokens, calling them markers rather than creatures — so the
precedent holds, but it needs checking wherever else tokens are consumed.
