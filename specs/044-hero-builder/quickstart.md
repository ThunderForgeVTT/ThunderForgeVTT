# Quickstart: Proving The Hero Builder

**Spec**: [spec.md](./spec.md) | **Contract**: [contracts/builder.md](./contracts/builder.md) | **Plan**: [plan.md](./plan.md)

How each phase is shown to work. There is no playtest for this feature: no
playtest scenario builds a hero, and writing one to satisfy a habit would prove
nothing a Playwright spec does not prove better. The proof is four e2e specs and
the package's own tests.

## Prerequisites

- `pnpm install`.
- **Phase (a) needs nothing else.** No Postgres, no RustFS, no server. That is
  the property the phase exists to keep (FR-012).
- **Phases (b) to (d)** need the usual e2e stack: Postgres and RustFS up, no
  other e2e or playtest running (`pgrep -f "[e]2e-parallel"` and
  `pgrep -f "[p]laytest"` both empty), and no `cargo test` beside either. A
  hand-started stack needs `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and
  Playwright at `--workers=1`.

## Phase (a): the builder itself

The catalogue's own gate, which `pnpm verify` already runs:

```bash
pnpm -F @thunderforge/heroes check
```

It must fail, not the builder, when a race's look names a choice or swatch
that does not exist, when an alias belongs to two races, when a roll for a race
breaks its look, or when `matchRace` misses a listed alias; and when a choice
has no label (US2 scenario 3), when
two choices draw alike (already true, `heroes.test.ts:59-71`), when `randomHero`
is not reproducible, when `minimalSpec` drops a field that changes the drawing,
or — from phase (d) — when `HERO_SPEC_SCHEMA` and the lists disagree.

The builder, with no backend:

```bash
pnpm -F @thunderforge/hero-builder-app test:e2e
```

Not through `scripts/e2e-parallel.mjs`: that harness knows two suites, both
rooted at `apps/web` (`scripts/e2e-parallel.mjs:185-193`), and each lane stands
up a database and a server this app does not use.

`apps/hero-builder/e2e/builder.spec.ts` takes its expectations from
`HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS` and `SIZES` **at run time**, never
from a count written in the spec file (SC-002). It checks:

- one control group per field, every choice labelled and selectable (FR-001,
  FR-002);
- both previews redraw on every change, with no reload (FR-003), and within
  100 ms of the change, measured in the page and logged per run (SC-006);
- every preset loads and matches what the CLI writes for it (US1 scenario 3);
- randomise is reproducible from its shown seed and leaves locked fields alone
  (FR-007);
- the race picker lists every key of `HERO_RACES` after "any"; for every race,
  rolling with a few seeds gives heroes that satisfy that race's look, derived
  from `HERO_RACES` at run time; a locked field beats the race; the exported
  spec carries no race (FR-007a, B5a);
- export then import reproduces the on-screen hero byte for byte, for every
  preset (SC-003);
- copy-as-preset produces an entry that pastes into `presets.ts` and passes the
  package's tests (FR-014);
- an invalid import is refused whole, by field, and changes nothing (FR-010);
- **no duplicated element id on the page, after loading every preset** (SC-005);
- the whole builder is walked by keyboard alone, and every choice and swatch has
  an accessible name (SC-004, FR-017 to FR-019);
- at 375 px wide both previews stay visible, nothing scrolls sideways, and every
  target is at least 44 px (FR-020).

**The US2 proof is done by hand, once.** On a throwaway branch, add a choice to
any list in `HERO_PARTS` with its label, run the two commands above unchanged,
and confirm the e2e finds, selects and draws it with no edit to
`packages/hero-builder` or to the spec file. Record the result in `tasks.md`;
discard the branch.

## Phase (b): a Game Master, an NPC

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-npc,world-compendium
```

then search the log for `✘` — the summary is not the proof.

`apps/web/e2e/hero-builder-npc.spec.ts`:

- a Game Master opens an NPC's edit page, presses "Build look", the builder
  opens as a full-screen dialog with no change of URL (FR-019), they change a
  choice, save, and both roles appear in the panel; the served bytes are WebP
  (`RIFF`/`WEBP`), the same assertion the P8 spec makes
  (`world-compendium.spec.ts:268`);
- the hero opens with the NPC's name already in it (FR-023);
- on the compendium list, a row's "Build look" opens the same dialog, saves both
  roles, and the GM is still on the list with the row's portrait updated
  (FR-028a); a Player sees no row build control;
- a dnd5e NPC whose sheet race is "High Elf" opens with the picker on elf and
  rolls pointed ears; a race of "Moonkin" and a Genie NPC both open on "any"
  (FR-007a, research R7);
- with the world paused, saving reports the refusal, stores nothing, and the
  dialog keeps the hero (spec Edge Cases);
- an NPC that already has art warns before saving that both roles will be
  replaced (FR-025);
- one role made to fail reports that role failed, the other as saved, and the
  failed role retries alone (FR-025, B4);
- a Viewer on the same page is offered no build or save control, **and** a
  direct `uploadActorImage` call is refused (FR-023, B5);
- Quick NPC: three NPCs from an empty compendium, each named, each with a
  portrait and a token, all three faces different; the Game Master stays in the
  compendium (US4);
- reroll changes the face; "Open in builder" carries the current hero across
  (FR-027);
- a Player is offered no Quick NPC (US4 scenario 5);
- the NPC survives a failed upload and is listed as lacking art (FR-028);
- the built NPC is placed twice and both tokens wear the face, because a copy
  reads its actor's art (`token_art.rs:41-58`, ADR-102);
- a hidden NPC's built portrait is refused to a player while its token art is
  served on a scene they can see — the rule at `assets_serve/actor.rs:155-165`,
  unchanged by a builder.

`world-compendium.spec.ts` runs beside it because the imagery panel is the file
this phase edits.

## Phase (c): a player, their character

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-player,actor-claims,world-actor-permissions
```

Server first — these are `cargo test` in `src/server/src/auth/actor_imagery_tests.rs`,
one per clause of B6:

- a claimant may write imagery; a non-claimant may not;
- a player who created their own character may, and stops when the claim ends;
- the grant covers `uploadActorImage` and `removeActorImage` and no other
  mutation on the actor;
- `allow_player_actor_art = false` refuses every actor in that world;
- `art_locked = true` refuses that actor whatever the world setting;
- a Game Master is refused by neither, and may still replace a player's art;
- the three refusals carry three different messages (contract §5).

`apps/web/e2e/hero-builder-player.spec.ts` then proves SC-010 through the
product: an invited player builds a hero for the character they hold, a second
player cannot change it, the Game Master turns the world setting off and the
player's controls and the mutation both refuse, the Game Master locks one
character and the player's other character is unaffected, and the Game Master
replaces the art regardless. Every refusal is attempted **by calling the
mutation directly as well as by looking for the button**.

## Phase (d): a saved hero

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-saved,collections-copy
```

`cargo test -p pack_system_spec` covers the new `appearance` block: dnd5e's
declaration loads, a source naming an undeclared field is refused, and an
absent block is accepted (phase b, run with phase b's gates).

`cargo test` for B7 in `src/server/src/heroes/spec_schema_tests.rs`: an array
refused, an over-cap spec refused, an unknown field refused, a valid spec
stored, and a refusal storing neither the spec nor the image.

`cargo test` for B9 beside the collection copy's existing tests
(`collections/copy_asset_and_link_tests.rs`): a copied actor's images arrive
with their specs, and the export carries them.

`apps/web/e2e/hero-builder-saved.spec.ts`:

- build, save, reload, re-open: every control shows the saved choice (SC-011);
- replace the token with an uploaded file, re-open: the builder starts from the
  portrait's spec and says the token is no longer a built hero (US6 scenario 2);
- an actor with no stored spec opens from its name, as in phase (b);
- a spec made invalid by hand opens with the problem named by field, and the
  stored images are untouched (US6 scenario 4);
- the NPC goes into a collection, the collection is copied into a second world,
  and the builder there opens the same hero and saves to the copy only
  (US7, FR-039).

## Before the final commit of each phase

```bash
pnpm verify                                   # includes packages/heroes' own check
pnpm -F @thunderforge/web exec tsc --noEmit   # verify does not type-check the app
make lint-wasm                                # the pre-push hook runs it; no engine change here
cargo test -p thunderforge-server --lib       # phases (c) and (d) only
```

And, for phases (b) to (d), the phase's e2e through the harness with the log
searched for `✘`.
