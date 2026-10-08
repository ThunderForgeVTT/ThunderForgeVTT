# Rolls at the table

Every roll made in a world reaches everyone at the table. It plays on every
board, and it is listed in every chat. That holds wherever the roll was
made: the dice roller, the character sheet in the dock, an attack, or the
sheet open in another tab. A roll can also be kept behind the screen.

## Who sees a roll

Pick who sees the roll in **Roll for**, next to the roll button. Your
browser remembers your choice for your next roll.

| Roll for      | Who can pick it | The roller | The Game Master | The other players            |
| ------------- | --------------- | ---------- | --------------- | ---------------------------- |
| **Everyone**  | Anybody         | The roll   | The roll        | The roll                     |
| **GM's eyes** | A player        | The roll   | The roll        | That a roll was made, `****` |
| **GM only**   | The Game Master | The roll   | The roll        | Nothing at all               |

A roll kept from the table does not play on the boards of the players it is
kept from. A reload lists past rolls again, but it never replays them.

## As a player

- **Everyone** is the default.
- **GM's eyes** rolls for the Game Master alone. The others see that you
  rolled for the Game Master, and nothing more: no label, no dice, and a
  total of `****`.
- The Game Master decides whether to reveal your roll. You cannot reveal it
  yourself.

## As the Game Master

- **GM only** rolls behind the screen. No player sees it, or knows that it
  was made.
- A roll you kept, or one a player rolled for your eyes, shows a **GM only**
  or **GM's eyes** mark in your chat. Each has a **Reveal** button.
- **Reveal** shows the roll to the whole table. It plays on every board and
  shows its numbers in every chat, marked "revealed by" you. A reveal cannot
  be taken back.

## Dice on the board

A roll is thrown as real dice. They tumble up from the bottom of the board
and come to rest in the lower third of your screen, each showing the face
the server rolled. Every board at the table throws the same dice the same
way, and they stay put on your screen while you pan or zoom the map.

A d4, d6, d8, d10, d12 and d20 are drawn as those solids. A d100 is two
ten-sided dice, tens and units. A d3 and a Fate die are cubes, a coin and a
d2 are discs, and any other size is a disc showing its number.

Under the dice, a line says who rolled, what for, and how the total was
reached: `Ayla: Stealth   13 + 3 = 16`. A bonus from a character sheet
shows as its number, never as its name. A roll that counts successes reads
`3 successes`. If more than 20 dice were rolled, 20 are drawn and a chip
says how many more there were, such as `+20 more`. The total is always the
server's.

- **A dropped die**, such as the lower one of a roll with advantage, is
  dimmed. A die that failed in a roll counting successes is dimmed too.
- **A rerolled die** tumbles again. The value it replaced is struck
  through above it.
- **An exploding die** brings in a new die of the same kind, which tumbles
  in beside it.
- **A clamped die**, held to a minimum or maximum, changes its number in
  place. The value it replaced is struck through above it.

The dice hold for a moment, then fade. If several rolls arrive at once,
they play one after another. When more than four are waiting, the oldest
waiting one is skipped on the board so the board keeps up with the table.
A skipped roll is still in the chat.

If your device is set to reduce motion, the dice appear already landed,
after a short fade, with no tumble. The board follows that setting as you
change it.

A **GM's eyes** or **GM only** roll never plays on the board of a player it
is hidden from. Once the Game Master reveals it, it plays there too.

## The sheet in another tab

A character's sheet page has the same rolls as the sheet in the dock. Open
the sheet in a second tab beside the play view and roll from it: the roll
plays on the board in the first tab, and on everyone else's. Nothing needs
to be connected between the tabs.

An attack needs a target, and you can only pick one on the board. So an
attack on the sheet page tells you to make it from the play view.

## Facets and rerolls

In a D&D 5e world, the rules that change how a die is rolled are applied by
the server when it rolls. You never edit a formula to roll with advantage.

### Advantage

Ability checks, saving throws and attacks have an **advantage** choice next
to the roll button: Normal, Advantage or Disadvantage. With advantage, two
d20s are rolled and the higher is kept. With disadvantage, the lower is
kept. The other die is shown dimmed. The choice goes back to Normal after
each roll. Damage is never rolled with advantage.

### What the sheet's facets do

The character sheet has a **Roll facets** section. Tick the ones the
character has, and every roll made for that character uses them:

- **Halfling Luck**: a natural 1 on a check, save or attack is rerolled
  once, and the new roll stands.
- **Great Weapon Fighting**: the damage of a two-handed weapon, in melee,
  treats a 1 or 2 on each die as a 3. It does not apply at range or with a
  thrown weapon.
- **Lucky**: the character has Luck Points to spend on rerolls. The sheet
  shows how many are left, and **Reset** gives them back after a long rest.

A roll that a facet changed says so in the chat, for example "Advantage" or
"Great Weapon Fighting".

### Rerolls

When a character has **Heroic Inspiration** ticked on their sheet, or Luck
Points left, their check, save or attack in the chat shows a **Reroll**
button, such as **Reroll (Heroic Inspiration)**. Only the person who made
the roll sees it, and only while they can still act for that character.

- The button is there for **two minutes** after the roll. After that, the
  roll stands.
- Rerolling spends the Inspiration or the Luck Point. The first roll is
  struck through, and the new one says what was spent.
- A roll is replaced once. Each spend is used once per roll: a roll
  rerolled with Heroic Inspiration can be rerolled again with a Luck Point,
  but not with Inspiration a second time.
- A missed attack can be rerolled. It is judged against the same Armor
  Class it missed, and a hit then rolls its damage as usual. A hit cannot be
  rerolled.

A reroll keeps the first roll's **Roll for**. A roll kept behind the screen
is rerolled behind the screen. When the Game Master reveals it, the table
sees the whole chain: the first roll, struck through, and the reroll.

### Two-handed weapons

Great Weapon Fighting needs to know which weapons are two-handed. On an
item's page, **As an attack** has a **Properties** list with the properties
the world's game system knows about, such as Two-Handed, Heavy and
Versatile. The Game Master, or whoever can edit the item, ticks the ones
the weapon has and saves. Only items have properties, not abilities.

## In the demo

The demo runs in your browser, so its tabs share one world. Roll in one tab
and the board in another plays it. **View as player** changes only the tab
you click it in, so you can keep the Game Master in one tab and a player in
another, and watch a hidden roll stay hidden.
