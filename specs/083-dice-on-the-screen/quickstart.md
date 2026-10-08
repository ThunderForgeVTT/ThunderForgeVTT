# Quickstart: Dice on the Screen

## Tuning the look, without the stack

```sh
ENGINE_PROFILE=dev node scripts/build.mjs --only-wasm   # from the repo root
pnpm -F @thunderforge/engine-sandbox dev
```

The sandbox has a **Roll** button with a formula field. It rolls with the
dice crate's wasm façade under a fixed seed, then sends `trigger_dice_roll`.
Try these formulas:

- `1d20 + 5`
- `2d6 + 1d8 + 3`
- `1d100`
- `4dF`
- `1d7`
- `2d20kh1 + 4`
- `2d6r<3`
- `3d6x`
- `6d10cs>=8`
- `40d6`

Press it five times quickly to watch the queue. Toggle reduced motion in the
OS, or in DevTools under Rendering, then "Emulate CSS prefers-reduced-motion".

## The real game

```sh
make dev
```

1. Open a world's play view as the GM, and a player's in a second browser.
2. Roll `1d20 + 5` from the dice roller. A d20 tumbles onto both boards,
   lands on the chat's number, and the readout shows `n + 5 = total`.
3. Pan far from the origin and zoom out. The next roll still lands in the
   lower third, at the same size.
4. Roll Stealth from the sheet. The readout shows the modifier's value, not
   `MODIFIER`.
5. As the player, roll for **GM's eyes**. It plays on the player's board and
   the GM's, and on no one else's.

In the console:

```js
__engineProbe.diceLanded().at(-1);
__engineProbe.diceEntities(); // 0 once the fade ends
```

## The demo

```sh
pnpm -F @thunderforge/demo dev
```

Open the play view in two tabs, and switch one to **View as player**. A roll
in either tab plays the same throw on both boards.

## Proof

```sh
cargo test -p thunderforge-dice
cargo test -p thunderforge-canvas-core dice_throw
make test-rust ARGS="-p thunderforge-server --lib roll"
make lint
node scripts/check-graphql-contract.mjs --schema
pnpm -F @thunderforge/web exec vitest run src/engine/bevy src/engine/world/sync src/components/world/DiceRollerPanel
pnpm -F @thunderforge/demo exec vitest run src/backend/handlers/dice.test.ts
node scripts/build.mjs --only-wasm          # release engine and dice wasm
pnpm e2e:rolls                              # demo two-tab e2e, then the rolls slice
pnpm e2e:which --diff                       # then run every slice it names
# Slices are the gate (owner decision 2026-10-07): no full-suite run.
```

The bundle is measured in brotli, before and after (research R13):

```sh
node -e "const z=require('zlib'),f=require('fs');console.log(z.brotliCompressSync(f.readFileSync('dist/engine/engine_bg.wasm')).length)"
```
