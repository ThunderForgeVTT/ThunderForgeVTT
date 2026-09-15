# Quickstart: Proving A Fight That Resolves

**Spec**: [spec.md](./spec.md) | **Contract**: [contracts/fight.md](./contracts/fight.md)

How to show each phase works. The playtest is the acceptance proof; e2e specs
pin what the playtest reaches by hand; server tests pin the rules.

## Prerequisites

- `pnpm install` (the pre-flight fails fast if you forget).
- Postgres and RustFS up, as for any e2e run.
- **Nothing else running an e2e or playtest** (`pgrep -f "[e]2e-parallel"` and
  `pgrep -f "[p]laytest"` both empty), and no `cargo test` during either.
- If you use a reused dev stack rather than the harness, start it with
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and run Playwright at
  `--workers=1`.

## The acceptance proof

```bash
pnpm playtest --only=combat-5e
```

Each phase turns FINDINGs in `apps/web/playtest/combat-5e.playtest.ts` into
hard checks. Read the report, not only the exit code.

| Phase | FINDINGs it turns into passes (line on 2026-09-14) |
|---|---|
| 1 | 522 bar doesn't move on an HP write; 548 zero HP doesn't mark down |
| 2 | 606 a player moved out of turn |
| 3 | (new check) two goblin copies take damage separately |
| 4 | 263 roll not shown to the table; 416 no armour class or target; 439 longsword not rollable from the sheet |
| 5 | 481 no size or reach |
| 6 | 380 no turn economy |
| 7 | (new check) three legendary actions spent across turns, refilled |

## Per-phase checks

Run targeted e2e through the harness:

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=<spec names, comma-separated>
```

then search the log for `✘`.

### Phase 1: damage lands and bars move

- **e2e** `combat-hit-points.spec.ts`: the Game Master damages a goblin with 7
  hit points by 5 from the tracker. Within one second, both player boards'
  bars read 2 with no reload. Damage 2 more: the row is marked out and
  `advanceTurn` skips it. Heal 3: it is back in the order. A manual Down,
  then heal: it stays down.
- **server tests**: temporary hit points absorb first; current never goes
  below 0; healing never exceeds max; two concurrent changes both land.

### Phase 2: turn order

- **e2e** `combat-turn-order.spec.ts`: on the ogre's turn, a player's move is
  refused with "It is Ogre's turn", and their token does not move on any
  board. With the ogre's name hidden, the text reads "It is Unknown's turn".
  The Game Master moves the ogre freely.
- **server tests**: `moveOwnToken` and the offline replay path both refuse;
  a token that is not a combatant moves freely.

### Phase 3: linked and unlinked

- **e2e** `token-links.spec.ts`: place Aria (linked) and two goblins
  (copies) from one NPC. Damage goblin A: goblin B and the NPC sheet are
  unchanged. Damage Aria's token: her sheet changes. Mark "Boblin" unique and
  place him: the token is linked. Relink goblin A: its hit points become the
  NPC's.
- **e2e / measure**: `engine-status-limits.spec.ts` gains a 200-copy level.
  No new actors are created, and the frame rate and load-time targets in
  [research.md R17](./research.md#r17-performance) hold.

### Phase 4: an attack aimed at something

- **e2e** `combat-attack.spec.ts`:
  - Aria attacks the goblin with her longsword. Every seat sees attacker,
    target, total, and hit or miss against AC within one second (asserted:
    `e2e/fixtures/seatTiming.ts` times the slowest seat and allows one
    re-measurement of the step, never a rerun of the test).
  - On a hit, the Game Master receives an offer, takes it, and the bars move
    everywhere.
  - The ogre hits Aria. Aria's player receives the offer. With her player
    offline, the offer is still pending on reconnect. The Game Master takes it
    on her behalf, and the table sees "resolved by Game Master".
  - Auto-apply on for this encounter: a hit on a goblin applies with no offer.
    A hit on Aria is still an offer.
  - A hidden ogre attacks: the player's view reads "Unknown", and the
    player's recorded network traffic contains neither the ogre's token id nor
    its name.
- **server tests**: C1–C10 in the contract, one test each.

### Phase 5: size, reach, range, line of sight

- **e2e** `combat-reach.spec.ts`: an ogre with `size: large` fills two by two
  on every board, and snapping, hit-testing and the keyboard all treat it so.
  A hero one square away swings with no flag. From four squares away, the
  hero is warned before rolling, the attack is still made, and the table sees
  "out of reach". A shortbow beyond normal range is flagged long range. An
  attack through a closed door is flagged no line of sight, and is not
  auto-applied.
- **canvas-core tests**: `footprint_distance` for 1×1, 2×2 and 4×4 on square;
  centre distance on hex; gridless.

### Phase 6: the economy of a round

- **e2e** `combat-economy.spec.ts`: on Aria's turn, all four lines are unspent
  on every seat. She attacks: the action is spent everywhere. She attacks
  again: the action shows as overspent, not refused. She moves 20 ft: movement
  shows 10 ft left. The turn passes and comes back: her budget is fresh.

### Phase 7: legendary and lair

- **e2e** `combat-legendary.spec.ts`: a creature with three legendary actions
  spends one at the end of each player's turn, showing 2, then 1, then 0, and
  3 again at the start of its own turn. A lair added by the Game Master sits
  at initiative 20 and loses ties.

## Before the final commit of each phase

```bash
pnpm verify
```

```bash
pnpm -F @thunderforge/web exec tsc --noEmit
```

```bash
make lint-wasm
```

`make lint-wasm` is only needed when the engine changed. Host clippy on the
engine is expected to fail, and is not a signal.
