# Quickstart: Validating In-Engine Status Displays

How to prove this feature works end to end, and how to catch the one failure
that matters most. Contracts are in [contracts/](./contracts/); entity rules
are in [data-model.md](./data-model.md).

---

## Prerequisites

```bash
make dev          # seeds admin/admin, user1/user1 (GM), user2/user2 (player)
```

`make dev` seeds a world ready to play with a GM and a player account, which
is what the disclosure scenarios need — most of them require two viewers of
the same token at once.

**Restart the stack after any engine or server change.** `dist/engine` and
`target/debug/thunderforge` are what the browser actually gets, and a stale
artifact has repeatedly produced failures that looked like logic bugs.

---

## Layered checks, cheapest first

### 1. Rules — no database, no browser

```bash
cargo test -p thunderforge_canvas_core resource_display
```

Covers entry ordering, depletion consuming the topmost entry first, spent
entries remaining in the list, quarter-band boundaries (exactly 25%, exactly
zero, a spent top entry), and rejection of a second entry where stacking is
forbidden.

These execute in milliseconds. **The engine crate's own tests compile and
never run**, so any rule that needs real coverage lives here — that is why the
model is in `canvas-core` and not beside the renderer.

### 2. Generated types match their source

```bash
cargo test -p thunderforge_canvas_core export_bindings
git diff --exit-code apps/web/src/engine/sdk/
```

A non-empty diff means the committed TypeScript has fallen behind the Rust.
Generation alone does not prevent drift; it moves where drift can occur, and
this check is what makes the commitment real.

### 3. The engine compiles for the target it actually runs on

```bash
cargo check --target wasm32-unknown-unknown -p thunderforge_engine
```

A native `cargo check` on this crate always fails and is not a signal
(Constitution V).

### 4. Server resolution and authorization

```bash
cargo test -p thunderforge status_display
```

Covers: a GM sees `VISIBLE` regardless of stored state; a player gets the
stored state or the world default; an explicit override beats the derived
one; an unrecognised stored state falls back rather than guessing; a
resource the actor stores nothing for is not displayed.

> This filter used to read `disclosure`, which matches no test in this crate
> and so passed by selecting nothing. Running the quickstart is what found
> it. A filter that selects zero tests reports success, which is the worst
> possible failure mode for a check whose whole job is to be trusted — prefer
> a module name you can see in the source over a word that describes the
> subject.

### 5. The wire, which is where the real assertion lives

```bash
cargo test -p thunderforge no_coarse_resolution_carries_the_exact_figure
cd apps/web && npx playwright test e2e/status-disclosure.spec.ts --workers=1
```

**These are the tests that matter.** The first asserts it at the resolver;
the second asserts it on the wire, by intercepting the actual GraphQL
response in a player's browser. Both are needed: the resolver test says the
rule is right, and only the browser test says it survives the whole way out. For every state other than `VISIBLE`,
assert the exact figure is **absent from the payload** reaching a non-GM
client:

| State        | Payload must contain    | Payload must NOT contain         |
| ------------ | ----------------------- | -------------------------------- |
| `GREYED`     | the resource's presence | any value, maximum or proportion |
| `PERCENTAGE` | a proportion            | any maximum or absolute value    |
| `CHUNKED`    | a quarter index         | any proportion or absolute value |

Asserting against the rendered UI instead would pass against a client that
received the value and chose not to draw it — which is the exact bug class
this feature is guarding, and one this project has shipped before in a
different place.

### 6. End to end, two viewers

```bash
cd apps/web && npx playwright test e2e/status-display.spec.ts
```

```bash
# and the rest of the suite this feature added
npx playwright test e2e/status-disclosure.spec.ts e2e/status-many-tokens.spec.ts \
  e2e/status-gm-control.spec.ts e2e/status-systems.spec.ts \
  e2e/status-placement.spec.ts e2e/status-sdk.spec.ts \
  e2e/status-appearance.spec.ts --workers=1
```

- A player opens a world; their character's token carries a bar; the corner
  panel shows every declared resource.
- A GM changes the value from another session; both surfaces update **without
  a reload**.
- A boss token set to `CHUNKED` shows a quarter band to the player and the
  true value to the GM, simultaneously.
- Moving the panel to another corner survives a reload.

### 7. Capacity, because status furniture multiplies

```bash
cd apps/web && npx playwright test e2e/engine-limits.spec.ts --workers=1
```

Compare against the documented baseline of 3,200 sprites at 60fps. SC-006
requires the cost to be a **stated number**, not an assumption — a measured
reduction is an acceptable outcome; an unmeasured one is not.

Use `--workers=1`: parallel headless engines contend badly and have produced
phantom failures before.

---

## Manual walkthrough

1. Sign in as **user1** (GM), open the seeded world, place two tokens.
2. Give one a health resource with two entries — a base pool and a shield —
   and confirm both segments draw, with depletion taking the shield first.
3. Set the second token to `CHUNKED`.
4. In another browser profile, sign in as **user2** (player) and open the same
   scene.
5. Confirm: the player sees a quarter band on the second token, and the GM
   sees the exact figure on the same token at the same time.
6. Set it to `GREYED`. Confirm the player can tell the resource _exists_ and
   nothing about its value — and that this looks different from a resource at
   zero.

Step 6 is the one worth doing by eye. "Nothing is known" and "the value is
zero" are different facts, and a design that renders them alike will pass
every automated check while misleading every player.

---

## Known traps

- **A stale wasm bundle or backend binary** is the most common cause of a
  "broken" result. Restart the stack after engine or server changes.
- **`--workers=1`** for anything involving the engine.
- **The engine's `#[cfg(test)]` modules never execute.** A green
  `cargo check` on that crate says it compiles, not that it works.
- **Do not assert disclosure from the screen.** Assert it from the payload.
