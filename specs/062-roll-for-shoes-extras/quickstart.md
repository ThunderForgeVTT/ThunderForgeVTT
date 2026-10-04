# Quickstart: proving Roll for Shoes Extras

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md)

How to run this feature's proof, and what each layer is actually claiming. The
contracts are in [contracts/](./contracts/) and the entities in
[data-model.md](./data-model.md); nothing is repeated here.

## Prerequisites

- A PostgreSQL dev database with migrations applied (`make migrate`).
- `pnpm install`, once.

## The three layers, fastest first

### 1. The rules — no browser, no stack

```bash
pnpm --filter @thunderforge/pack-roll-for-shoes test
```

Runs `packs/systems/roll_for_shoes/web/src/game.test.ts` under `node --test`.
This is where the arithmetic is proven: band dice and band targets, the resolved
roll's field order, status modifiers summing, slot caps and costs, and the XP
ledger staying honest across both ways of spending.

**The test that matters most**: `resolve()` given `DEFAULT_SETTINGS` produces
exactly what spec 061's `verdict()` produced, for every combination of total and
opposition. Everything else this feature adds is optional; that one is the claim
that the core game did not change.

Expected: all tests pass, in under a second.

### 2. The stored shape and the network surface

```bash
cargo test -p roll-for-shoes-server
cargo test -p thunderforge-server play_pause_surface
```

The first proves the new validator rules — status ids unique, names non-empty,
bought-slot keys and values integers — and, just as importantly, that a
character holding more skills at a level than the caps allow is still **storable**
(FR-036).

The second is the one that fails if the new mutation is not classified. A pack
contributing a root field must submit a `PackSurface` entry with a request
document, or `play_pause_surface_tests` refuses the build. Expect it to fail
first and pass once `settings/play_pause_surface.rs` exists.

### 3. The proof — the slice

```bash
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:game-systems
```

Per constitution Principle VI this is the gate. It runs
`apps/web/e2e/system-roll-for-shoes-extras.spec.ts` (new) alongside
`system-roll-for-shoes.spec.ts` (unchanged) and the slice's other specs.

The rate-limit bypass is not optional on the external stack — without it the
repeated registrations return 429 and read as flaky tests.

Read the harness's `Totals:` line and nothing else. Expected: every spec passes,
including spec 061's unmodified. The slice last measured 176s for 18 specs; when
this feature is done, re-record it:

```bash
pnpm e2e:game-systems --record-durations
```

and commit the changed line in `scripts/e2e/slice-durations.json`.

## What the e2e has to work around

**There is no dice seed.** Rolls are server-rolled and random, so every
assertion is built from something true of *every* roll, the way spec 061's are:
a single d6 cannot beat 6, four d6 cannot reach 25, a status of −100 cannot
produce a positive total. Each scenario below is constructible that way.

**A player needs Editor, not just a claim.** The play-dock path needs a second
account that claims the character **and** is granted Editor on it — claiming
grants no write access today (FR-045). That is a host gap recorded separately;
until it closes, the test grants Editor explicitly. Spec 063 closes it: a claim
grants Editor, and the explicit grant comes out of the test.

**Assert `rfs-error` is absent** wherever a write is expected to land. The sheet
reports a refusal in a badge rather than throwing, so a lost write otherwise
looks like a disagreement about a number.

## Scenarios, and what each one settles

| Scenario | Settles |
| --- | --- |
| A world that touches no setting plays the core game | FR-001, FR-004 — the whole feature is opt-in |
| Turn on the tie rule; roll one die against a target of 6 | FR-015–018 — a tie succeeds and awards no XP |
| Add a −100 status, roll, read `rfs-total` | FR-021, FR-022 — flat, once, on the total |
| Same roll, confirm the advancement prompt behaves as the faces dictate | FR-023 — a status cannot create or destroy an advancement |
| Set difficulty to `rolled`, pick Very Hard, compare `rfs-gm-die-*` to `rfs-die-*` | FR-013, FR-014 — Game Master dice are a separate roll |
| Fill level 2, roll an advancement | FR-030 — "no room", not a silent denial |
| Buy a level-2 slot, watch `rfs-xp` | FR-031, FR-035 — 4 XP, one honest ledger |
| Define starting skills, then open a character made before | FR-039 — a stored character is unchanged |
| A player opens the settings panel | FR-007 — read-only, and the server refuses the write |

## The check that is easy to forget

```bash
node scripts/check-system-registry.mjs
```

Fails the build if shared server or web source names a system id. `KNOWN` is
empty and must stay empty (FR-006). Note what it cannot catch: a **column** named
for a ruleset is neither a quoted id nor a filename, which is exactly why the
settings went into a pack-owned table rather than onto `worlds` (research D1).
