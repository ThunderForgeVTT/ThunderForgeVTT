# Quickstart: Hidden Walls and Box Select

This assumes spec 082 is merged, so players have Select and shapes carry
`createdBy`.

## Real game

```sh
make dev
```

1. **The GM hides a wall.** Sign in as a GM, open a scene with walls and
   invite a player in a second browser.
   - **A plain wall.** Arm **Walls**, select a plain wall and tick **Hidden
     from the table**. The GM's board draws it faint. The player's board
     stops drawing it.
   - **The player's token.** As the player, drag your token across where
     the wall was. It goes back. Stand beside it: sight stops at the line
     you cannot see.
   - **A door.** As the GM, click the hidden wall into a door. It stays
     hidden and becomes a secret door.
2. **The GM boxes several things.**
   - **The box.** Arm **Select** and drag from empty board around three
     tokens and a wall. All four are selected, and the Select bar says
     "3 tokens, 1 wall".
   - **The move.** Drag one of the tokens. All four move together. Reload,
     and they are where you dropped them.
   - **Shift.** Shift-drag a box over one of them: it leaves the group.
     Shift-click it: it comes back.
   - **The filter.** Open the selection filter and untick walls. Box the
     same area: the wall is not taken.
   - **Hiding a group.** With walls in the group, **Hidden from the table**
     in the Select bar hides every one.
   - **Delete.** It removes the whole group.
3. **A player boxes what is theirs.** Give the player two tokens.
   - **The box.** As the player, arm **Select** and box both, plus an NPC,
     a wall, your own shape and the GM's shape. Only your two tokens and
     your shape are selected.
   - **The move.** Drag them so that one path crosses a wall. That token
     goes back, the other lands, and one notice says "1 of 2 could not be
     moved."

## Demo

```sh
pnpm -F @thunderforge/demo dev
```

Steps 1 and 2 work as above, with no backend change. For step 3, use
**View as player** in a second tab.

## Proof

```sh
cargo test -p thunderforge-canvas-core box_select shape_geometry
cargo test -p thunderforge-engine box_select group_move door_click token_owner
pnpm -F @thunderforge/web test -- groupMoves walls lights store
make lint                       # host and wasm32
pnpm e2e:canvas                 # includes apps/web/e2e/canvas-box-select.spec.ts
pnpm e2e:which --diff           # run each slice it names
```
