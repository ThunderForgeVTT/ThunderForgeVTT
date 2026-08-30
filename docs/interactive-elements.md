# Interactive elements

Things on a scene that respond. A book that opens a lore page, a lever that
turns the lights out, a door that opens and locks, a threshold that fires when
the party crosses it, and a request the Game Master has to approve. Spec 030.

## What problem this solves

A Game Master preparing a scene could place walls, lights, tokens and
drawings, and nothing among them could _do_ anything. Everything that happened
at the table happened because the GM narrated it and then made the change by
hand — open the door, dim the lights, tell them what the book says.

This does not replace that. It is not a way to run a game without a Game
Master, and nothing here decides anything a GM has not decided in advance.
Every trigger is something a GM placed, configured and chose to allow, and
anything consequential stays gated on their approval. It is a way for a
prepared scene to carry some of what the GM already intended, so the thing they
described actually happens when a player reaches for it.

## What open, closed and locked mean

The definition the spec asked for, and the shape it is worth understanding:

| State    | Blocks vision                  | Blocks movement                  |
| -------- | ------------------------------ | -------------------------------- |
| `open`   | No                             | No                               |
| `closed` | The wall's own `blocks_vision` | The wall's own `blocks_movement` |
| `none`   | The wall's own `blocks_vision` | The wall's own `blocks_movement` |

A closed door blocks exactly what the wall it is part of blocks, which is why
a closed window stays see-through and a closed stone door does not. Nothing
stores a second set of flags for the closed state, so there is nothing to keep
consistent — the rule is `Wall::blocking` in
`crates/thunderforge-canvas-core/src/wall.rs`.

A closed door is therefore indistinguishable from a plain wall in what it
blocks. That is correct: the difference is that it can be opened.

**`locked` is a separate property, not a third state.** It governs _who may
change the state_, never the state itself. As one enum it would make "open,
and players cannot close it" — a spiked-open portcullis — inexpressible, and
it would force a decision about what happens to the lock when a GM opens a
locked door that a separate flag simply never raises.

A locked door refuses a player and accepts the Game Master. It is theirs; it is
not a rule against them.

**`secret` is presentation only.** The geometry reaches every client and it
goes on blocking there; what differs is that a player's engine does not draw
it. A secret door that did not arrive would also stop blocking vision, so the
player's line of sight would run through a wall the GM can see — a far louder
tell than a hidden sprite. Somebody who inspects their own client and announces
a secret door has created a problem at their table, not found one here.

## How a subsystem becomes triggerable

The whole feature is a contribution seam. `InteractionPlugin` owns placement,
hit-testing, entry detection, `once` bookkeeping and dispatch, and owns **no
effect at all**. Every effect is contributed by the subsystem that performs it.

Adding one takes three things and touches nothing else:

1. **A declaration**, in the module that owns the subsystem, in
   `thunderforge-canvas-core`. It is data — id, label, description, which
   subjects it attaches to, and typed configuration fields. See
   `wall.rs::interaction_effects` or, for the smallest possible example,
   `seam_probe.rs`.
2. **One line** in `contributions()` in `src/server/src/interaction.rs`, plus a
   performer there if the effect changes anything that outlives the click.
3. **One Bevy plugin with one system** that reads `InteractionActivated` and
   handles the identifiers it declared, ignoring the rest. See
   `src/engine/src/plugins/lore_link.rs` — it is about forty lines including
   its documentation.

Nothing in the interaction core changes. If it ever has to,
`scripts/check-interaction-seam.mjs` should be failing: it asserts that plugin
names none of "light", "door" or "sound", which turns FR-039 from a judgement
call into something anybody can see in a diff.

The full contract is `specs/030-interactive-elements/contracts/effect-registry.md`,
and the decision behind it is
`docs/adrs/20260830-054-interaction_effect_contribution_seam.md`.

### Why declarations are code and not configuration

An effect is a capability of the build, not content that varies per world.
Expressed as data — a manifest, a table — a deployment could declare an effect
no code performs, and a Game Master would be offered something that silently
does nothing. That is the same failure spec 029 spent a whole user story
retiring at the engine's command boundary, and there is no reason to reintroduce
it one layer up.

The consequence worth stating: **an unbuilt subsystem contributes nothing.**
There is no audio yet, so nothing declares a sound effect and none is offered —
no greying out, no "coming soon", no dead option to maintain. When audio is
built it contributes `audio.play` and nothing else changes.

### What happens when a subsystem goes away

An interactive whose `effect_id` is not in the current registry is
**unavailable**. It is shown as such to the Game Master, is not dispatched, is
not deleted, and reaches players as nothing rather than as an error. Put the
subsystem back and it works again with nothing to restore.

Detection is a registry lookup, never "we dispatched and nothing happened" — a
Bevy message is fire-and-forget and cannot report that nobody listened.

## What runs where

- **Rules** live in `crates/thunderforge-canvas-core/src/interaction.rs`: the
  registry, authoring validation, region entry, and the activation truth table.
  They live there because the engine crate targets wasm32 with no test runner,
  so its tests compile and never execute — the same constraint that put spec
  029's resource model there.
- **Authority** lives at the GraphQL boundary
  (`src/server/src/graphql/mutations_interactives.rs`). Every refusal is
  decided from stored state. "A player cannot open a locked door" is the rule
  most likely to be implemented by not drawing the button, which passes every
  screen test and fails the moment anybody calls the mutation directly — so it
  is tested at the server with the mutation called directly.
- **Responsiveness** lives in the engine. It applies a permitted change locally
  so it is visible immediately rather than a round trip later. It is never a
  second authority on whether the change was allowed; a disagreement between
  the two is a client bug (Principle III).
- **Chrome** lives in React: the authoring panel, the door controls, the
  approval queue. The authoring form is built from `effectRegistry` rather than
  a list written there, which is what makes "a GM is offered exactly what
  exists" true rather than aspirational.

## Triggers

- **Click.** Somebody activated the subject. The one mutation a player calls is
  `activateInteractive`, and it answers with a tagged outcome — performed,
  requested, refused with a reason, unavailable, or no effect — because "it did
  not run" covers five different situations and a player told only "no" cannot
  tell a locked door from a broken product.
- **Enter.** A token crossed into a region. The engine detects it, because it
  is the only thing that knows both where the token was and where it is now,
  and it _reports_ rather than performs: whether a crossing is permitted is the
  same question a click raises, and answering it twice in two places is how the
  two answers drift.

Entry fires once per crossing, not continuously while inside. A region that
fired on every step reads at the table as the scene stuttering rather than as a
trigger misbehaving.

**Preparation fires nothing.** A GM arranging a scene and a GM running one make
the identical gesture, so the engine has to be _told_ which it is
(`set_scene_playing`). It defaults to preparation, because a trigger that went
off while nobody was looking has already spent itself and a table has no undo
for that.

## Approval

An interactive set to `requires_approval` raises a request and performs
nothing. The Game Master sees it — on any device, and after a refresh, which is
why requests are a table rather than memory — and decides.

Two rules matter more than the rest:

**Nothing expires into approval.** There is no timeout anywhere in this
feature: no countdown, no default action, no auto-approve. Silence is not
consent, and a queue that eventually says yes on the GM's behalf is a queue
that decides things they did not.

**Approval runs the effect with the permission it has now**, not the permission
it had when the player asked, and it runs as the _requester_ rather than as the
approving GM. A Game Master who locks a door and then approves a queued request
to open it has contradicted themselves, and the lock is the more recent
statement. Trusting the request's own moment would make approval a way to
perform something currently forbidden.

## What it costs

**Nothing measurable.** A board of 200 moving tokens with 50 region
interactives runs at the same frame time as the same board with none:

```
tokens=200 interactives=50
  absent   17.1ms / 58fps  (n=18)
  present  17.0ms / 59fps  (n=18)
  ratio    0.994
```

Both sides sit on the vsync floor, measured back to back in one session so the
comparison is between the conditions rather than between two runs on different
builds. `apps/web/e2e/engine-interaction-limits.spec.ts` re-measures it.

That is the expected answer, which is exactly why it is checked — an expected
result nobody verified is an assumption, and this one had a plausible way of
being wrong. Entry detection runs every frame over every token, and a naive
implementation would turn a populated board into a token-times-region sweep
that nobody notices until a real table hits it.

Two things make the measurement honest rather than flattering. The tokens are
_moved_ while sampling, because a still board never exercises entry detection
at all — the comparison returns immediately when position has not changed. And
the interactives are all regions rather than props, because a prop costs one
map entry and nothing per frame; fifty regions is the worst case fifty
interactives can be.

## What is deliberately not here

- **Sound.** No audio subsystem exists. Nothing declares a sound effect and
  none is offered, which is the seam working rather than a gap in it.
- **Multi-scene navigation.** `nav.request_scene` raises a request, the GM
  decides, the requester is told — and nothing moves anybody, because there is
  nothing yet to move them with. The request and the decision are the parts
  this feature owns and they work today.
- **Party tokens.** A token the GM controls and the whole party follows, for
  world maps rather than tactical play. Future work, named in the spec so the
  model does not accidentally preclude it.
- **Bulk authoring.** Fifty interactives is the scale. A bulk API would exist
  only to be misused.

## Where to look

| Concern                           | File                                                 |
| --------------------------------- | ---------------------------------------------------- |
| Rules, registry, activation table | `crates/thunderforge-canvas-core/src/interaction.rs` |
| Doors, and what they contribute   | `crates/thunderforge-canvas-core/src/wall.rs`        |
| Dispatch, triggers, hit-testing   | `src/engine/src/plugins/interaction.rs`              |
| The smallest possible contributor | `crates/thunderforge-canvas-core/src/seam_probe.rs`  |
| Authorization and persistence     | `src/server/src/graphql/mutations_interactives.rs`   |
| Registry assembly, requests       | `src/server/src/interaction.rs`                      |
| Authoring UI                      | `apps/web/src/components/InteractionAuthor/`         |
| The GM's queue                    | `apps/web/src/components/ApprovalQueue/`             |
| End-to-end proof                  | `apps/web/e2e/interactive-*.spec.ts`                 |
