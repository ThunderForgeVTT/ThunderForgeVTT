# Quickstart: Players Draw Shapes

## Real game

1. `make dev`. Sign in as a GM, create a world, invite a second account and
   join as that player in another browser. Do not touch the world's tool
   grants.
2. Both open the play view. The player's rail shows **Select** and
   **Shapes**, and nothing else.
3. The player draws a rectangle. It appears on the GM's board too.
4. The GM draws a circle. The player clicks it with Select: nothing is
   selected. The player drags their own rectangle: it moves on both boards.
5. The GM opens **Shapes** and chooses **Clear a player's shapes…**, ticks
   the player and confirms. The rectangle goes; the circle stays.
6. The GM chooses **Clear all shapes** and confirms. The board is empty.
7. In the world's settings, the GM unticks **Shapes** for the player. The
   player's rail drops it without a reload. Ticking it again brings it back.

## Demo

1. `pnpm -F @thunderforge/demo dev`, open the demo.
2. **View as player**: the rail shows Select and Shapes. Draw a shape, then
   try to select one of the GM's: nothing happens.
3. Switch back to the GM in the same tab (or keep the GM in another tab).
   **Clear a player's shapes…** lists the player; clearing removes only
   their shape.

## Proof

```sh
make test-rust ARGS="-p thunderforge-server"
cargo test -p thunderforge-engine shape
pnpm -F @thunderforge/web test -- authoringTools shapes
pnpm -F @thunderforge/demo test -- shapes
pnpm e2e:canvas
pnpm e2e:which --diff           # run every slice it names
make lint
```
