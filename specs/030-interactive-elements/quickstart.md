# Quickstart: Validating Interactive Elements

How to prove this feature works, cheapest checks first, and which failure each
one is actually guarding against. Contracts are in
[contracts/](./contracts/); entity rules are in
[data-model.md](./data-model.md).

---

## Prerequisites

```bash
make dev          # seeds admin/admin, user1/user1 (GM), user2/user2 (player)
```

Two accounts matter here more than in most features: almost every claim is
comparative — what a _player_ may do against what a _Game Master_ may do — and
a single-account test cannot tell the difference.

**Restart the stack after any engine or server change.** `dist/engine` and
`target/debug/thunderforge` are what the browser actually gets, and a stale
artifact has repeatedly produced failures that looked like logic bugs.

---

## Layered checks

### 1. Rules — no database, no browser

```bash
cargo test -p thunderforge_canvas_core interaction
cargo test -p thunderforge_canvas_core wall
```

Covers the blocking rule (open blocks nothing; closed blocks what the wall
blocks), lock semantics, registry assembly, collision detection, and region
entry logic.

These execute in milliseconds. **The engine crate's own tests compile and never
run** (Constitution V), so any rule needing real coverage lives here — which is
why the registry and the door semantics are in `canvas-core` and not beside the
renderer.

### 2. The seam, without a table

```bash
cargo test -p thunderforge_canvas_core registry_
```

Two contributors declaring the same identifier must fail at assembly, not at
first use. A collision found when a Game Master happens to author one of them
is a collision found at the table.

### 3. The engine compiles for the target it runs on

```bash
cargo check --target wasm32-unknown-unknown -p thunderforge_engine
```

A native `cargo check` on this crate always fails and is not a signal.

### 4. Authorization — the part that must not live in the UI

```bash
cargo test -p thunderforge interactive
```

**This is the layer that matters most.** Every one of these is a rule a
developer could plausibly implement by hiding a button:

- A player cannot create, edit or delete an interactive.
- A player cannot open a locked door.
- A player cannot activate a `gm_only` interactive.
- A `requires_approval` activation performs nothing until approved.
- A request cannot be approved by its own requester.
- Approval re-checks permission at decision time, so a door locked after the
  request was raised stays locked.

A test asserting the button is absent would pass against a server that happily
performs the mutation when asked directly.

### 5. End to end, two browsers

```bash
cd apps/web && npx playwright test e2e/interactive-*.spec.ts --workers=1
```

- **US1** — a prop opens a lore entry, and the scene is undisturbed.
- **US2** — a player opens and closes a door; vision and movement change for
  everybody without a reload; the GM locks it and the player's click stops
  working.
- **US3** — a switch toggles lights for every viewer.
- **US4** — a secret door is not presented to players, and becomes a normal
  door once revealed, and is still revealed after a reload.
- **US5** — a region fires once on entry, not continuously, and not while the
  GM is arranging the scene.
- **US6** — a request reaches the GM, refusing changes nothing, approving runs
  the effect, and doing nothing leaves it pending for ever.
- **US7** — the seam.

Use `--workers=1`: parallel headless engines contend badly and have produced
phantom failures before.

### 6. The seam, end to end

```bash
cd apps/web && npx playwright test e2e/interactive-contribution.spec.ts --workers=1
```

The test with no table-visible value and the most architectural value. It
proves three things that only fail once a second subsystem exists — by which
point they are expensive to fix:

1. A newly contributed effect becomes authorable with no edit to the
   interaction feature.
2. With a contributor absent, its effects are not offered and everything else
   works.
3. A scene authored against an absent contributor shows the GM an unavailable
   interactive, and loses no authored data.

### 7. Capacity

```bash
cd apps/web && npx playwright test e2e/engine-limits.spec.ts --workers=1
```

SC-007 asks that 50 interactives cost nothing measurable against the documented
baseline. Unlike status displays — which are per-token and were measured
costing 4 sprites each — interactives are event-driven and rare, so the expected
answer is "no measurable change". **Measure it anyway**: an expected result that
was never checked is an assumption, and spec 029's capacity work only found its
real cost because somebody ran it.

---

## Manual walkthrough

1. As **user1** (GM), open the seeded world and place a prop — a book.
2. Attach a lore entry. Confirm the authoring panel offers only effects this
   build can perform, and that **no sound effect is offered** — nothing
   contributes one.
3. Draw a wall across a doorway and designate it a door. Confirm it is drawn
   distinguishably and starts closed.
4. As **user2** (player) in another browser profile, click the door. It opens
   for both of you, and vision changes.
5. As the GM, right-click the door and lock it. Confirm the player's click no
   longer opens it and that they are _told it is locked_ rather than ignored.
6. Confirm the GM can still open it.
7. Place a second door, mark it secret. Confirm the player is not shown a door.
8. Wire a prop to reveal it. Have the player activate the prop; confirm the
   passage becomes a normal door for both.

Steps 5 and 8 are the ones worth doing by eye. A locked door that silently does
nothing is indistinguishable from a broken product, and a reveal that works
only for the person who triggered it looks correct to whoever is testing alone.

---

## Known traps

- **A stale wasm bundle or backend binary** is the most common cause of a
  "broken" result. Restart the stack after engine or server changes.
- **`--workers=1`** for anything involving the engine.
- **The engine's `#[cfg(test)]` modules never execute.** A green
  `cargo check` on that crate says it compiles, not that it works.
- **Do not assert permission from the screen.** A hidden button is not a
  refused mutation. Assert at the server.
- **Do not let doors become special.** They are a contributor like any other.
  If the interaction plugin ever needs to know what a door is, FR-039 has been
  violated and US7 should be failing.
