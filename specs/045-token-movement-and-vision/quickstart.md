# Quickstart: Proving Token Movement and Vision

The playtest is the acceptance test. Each phase turns named FINDINGs into
passes; when a phase lands, its soft check becomes a hard one in the scenario.

## Prerequisites

A dev stack's dependencies: Docker (Postgres, RustFS), the engine built, and
`pnpm install` done. Nothing else — `pnpm playtest` builds and starts
everything it needs on its own stack.

## The one command

```bash
pnpm playtest --only=dungeon-crawl
```

Watch it in `apps/web/playtest-report` (`pnpm exec playwright show-report
playtest-report` from `apps/web`): every step carries a screenshot of the Game
Master's and both players' screens, and each player's browser is recorded.

Before this feature, that run records these findings, and they are what each
phase clears:

| Finding | Cleared by |
|---|---|
| `the door Aria opened should be open on her own board` | Phase 1 |
| `the door Aria opened should open on the Game Master's board` | Phase 1 |
| `the door Aria opened should open on Brom's board` | Phase 1 |
| `in daylight the open door should show Aria the goblin` | Phase 1 |
| `a brazier by the goblin should let Aria see it through the open door` | Phase 1 |
| `D (east) should walk Aria's own token one cell` | Phase 2 |
| `a wall that blocks movement should stop Aria at it` | Phase 3 |
| `heroes went through walls that block movement` (the wander) | Phase 3 |

## Per phase

**Phase 1 — a door reaches every board.** After the change, the door steps pass
and the two light steps that rested on them pass with them. Check by hand once:
two browsers on the same scene, the Game Master designates a wall a door and
opens it, and the other browser's board shows it open without a reload.

```bash
cargo check -p thunderforge && pnpm playtest --only=dungeon-crawl
```

**Phase 2 — the keyboard moves a player's own token.** The first step of the
crawl passes: a press of D moves Aria one cell, and the Game Master, Brom and
the server all agree where she is. On a gridless scene the same press persists
too, which the crawl does not cover — check it by hand, or add it.

```bash
cargo check --target wasm32-unknown-unknown -p thunderforge_engine
pnpm -F @thunderforge/web exec tsc --noEmit
pnpm playtest --only=dungeon-crawl
```

**Phase 3 — a wall stops a hero.** The "walks into the wall" step passes, and
the seeded wander records no crossings. Then prove the server does not depend
on the engine's good manners:

```bash
pnpm -F @thunderforge/web exec playwright test e2e/token-movement-walls.spec.ts
```

That e2e sends a crossing move straight to the server from a player's session
and expects a refusal naming a wall.

**Phase 4 — the vision rules.** The crawl's sight steps already pass; this
phase makes them hard rather than assumed, and adds the cases the spec names
that the crawl does not cover (a player with no token; a token hidden in the
dark with no wall involved).

**Phase 5 — a game system's sight.** A new playtest step: a D&D 5e character
with darkvision sees a token in darkness within its range, dimly, and a
character without it sees nothing there.

```bash
pnpm playtest --only=combat-5e
```

**Phase 6 — a map that remembers.** A new playtest step: with exploration on, a
player walks through two rooms, reloads, and sees both rooms faded and nothing
of the third; the Game Master resets, and the fog goes.

## What "done" means for the feature

```bash
pnpm playtest          # both scenarios, no FINDING in the movement/vision set
pnpm verify            # the repository's own gate
pnpm -F @thunderforge/web exec tsc --noEmit   # verify does not type-check the web app
```

The last line is not redundant: `pnpm verify`'s web checks are formatting and
lint, and a type error has reached a commit here before.
