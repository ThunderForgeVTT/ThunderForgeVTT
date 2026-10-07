# Quickstart: 5e Roll Facets

## Real game

1. `make dev`. Sign in as a GM and create a 5e world. Invite a second
   account and join as that player in another browser. Give the player a
   character, and place its token and a goblin's token on a scene.
2. **Advantage.** On the player's in-pane sheet, set the picker beside the
   checks to **Advantage** and roll Stealth. Both chats show
   `2d20kh1 + …`, two d20s with the lower struck, and an **Advantage** tag.
   The picker is back on **Normal**.
3. **Halfling Luck.** On the sheet, tick **Halfling Luck** under Roll
   facets and roll any check. The formula reads `1d20r1 + …` and the entry
   is tagged **Halfling Luck**. On a natural 1, the 1 shows struck beside the
   die that counted.
4. **Heroic Inspiration.** The GM ticks the character's **Heroic
   Inspiration**. The player rolls Athletics. Their own chat entry shows
   **Reroll (Heroic Inspiration)**, and the GM's does not. The player clicks
   it:
   - the first entry is struck through in both chats, with the new roll
     beneath it, marked "Heroic Inspiration";
   - the sheet's Inspiration is off;
   - the button is gone.
5. **Lucky.** Tick **Lucky**. The sheet shows Luck Points left, equal to the
   proficiency bonus. Reroll a check with **Reroll (Luck Point)**. The new
   roll has one more d20 and keeps the highest, and the points left drop by
   one. Roll a check at **Disadvantage**: no Luck button is offered.
   **Reset** on the sheet sets the points back.
6. **A missed attack.** Set the goblin's AC to 99 on its sheet. Select the
   character's token, target the goblin and attack with a weapon. It
   misses. Grant Inspiration and click **Reroll** on the to-hit. The reroll
   is judged against 99 and misses too, and no damage is offered.
7. **Great Weapon Fighting.** Open the greatsword's attack fields as the GM
   and tick **Two-Handed**. Tick **Great Weapon Fighting** on the sheet.
   Lower the goblin's AC to 5 and attack in melee. The damage reads
   `2d6min3 + …`, and any 1 or 2 shows beside a 3.
8. **Two minutes.** Wait two minutes after a roll. Its Reroll buttons are
   gone, and the server refuses a reroll.

## Demo

1. `pnpm -F @thunderforge/demo dev`, and open the demo's play view.
2. Roll a check with **Advantage** from the in-pane sheet, then reroll it
   with Inspiration once the sheet has it. Both work as in the real game,
   with no server.

## Proof

```sh
cargo test -p thunderforge-dice
cargo test -p thunderforge-system-dnd5e
make test-rust ARGS="-p thunderforge-server"
pnpm -F @thunderforge/demo test
pnpm -F @thunderforge/web test
make lint
node scripts/check-graphql-contract.mjs --schema --fix
pnpm e2e:rolls
node ./scripts/e2e-parallel.mjs   # the gate before merge: schema and migration are cross-cutting
```
