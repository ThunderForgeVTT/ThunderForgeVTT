# Quickstart: Rolls at the Table

## Real game

1. Start the dev stack, open a world as the GM in one browser
   and as a player in another.
2. Player: open the play view, roll `1d20` from the dice roller. Both
   boards show the dice; both chat panels list the roll.
3. Player: open the character sheet page in a second tab
   (`/world/:id/actor/:actorId/view`), roll Stealth. The play view tab
   animates it.
4. Player: pick "GM's eyes", roll. The GM sees the number; another
   player's panel shows `****`.
5. GM: pick "GM only", roll. Players see nothing at all.
6. GM: press "Reveal" on either. Every board animates it; the feed marks
   it "revealed by the GM".

## Demo

1. `pnpm -F @thunderforge/demo dev`, open two tabs.
2. Switch one tab to player. Roll in either; both boards animate.
3. Close the first tab; keep rolling in the second. Reload; nothing lost.

## Proof

- `cargo test -p thunderforge-server rolls`
- `pnpm -F @thunderforge/demo test`
- `pnpm e2e:rolls`
