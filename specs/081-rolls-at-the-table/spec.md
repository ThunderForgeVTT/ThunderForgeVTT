# Feature Specification: Rolls at the Table

**Feature Branch**: `081-rolls-at-the-table`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "is it possible for us to make communication possible between browser tabs via a service worker so the users character sheet can trigger rolls on the play field in demo and in the real game?" Then: "everyone sees rolls, GM-hidden rolls filtered server-side; draft the spec also players can do gm eyes only in which case to other players we send \*\*\*\* for the roll and the gm gets the real number if the gm reveals to everyone it basically republishes the messages"

## Why

A roll is a moment the whole table shares. Today it is not shared at all.
The server rolls and records the dice, then answers only the person who
asked. The dice tumble on that person's board and nowhere else. Another
player sees nothing, and neither does the GM unless they open the roll
history.

That is also why a character sheet in a second tab cannot roll onto the
board. The tab that rolls is the only one that hears the answer. The
answer is not a channel between tabs. The roll should travel the way walls,
tokens and chat already do: the server decides it, records it, and tells
every member of the world. Every tab and every player's board then shows
it. A second tab is just one more listener.

Tables also keep secrets. A GM rolls behind the screen. A player rolls
something only the GM should know, such as a stealth check or an insight
check. So a roll has a visibility, and the server enforces it. Whatever a
viewer may not see never leaves the server.

The demo has to behave the same way, in the browser and without a server.

## What exists

Counted on 2026-10-07:

- **Rolling.** `rollDice` (`crates/thunderforge-server/src/graphql/mutations_roll.rs`)
  rolls on the server and writes `world_roll_records`. It records no world
  event, so nothing else hears about the roll. The full history query,
  `worldRollRecords`, is for the GM only.
- **Dice on the board.** `DiceRollerPanel.tsx` and `InPaneCharacterSheet.tsx`
  each call `triggerDiceRollAnimation` with the result they got back. Only
  the rolling tab animates.
- **Attacks** (spec 046, `combat/attack.rs`) already record
  `EVENT_CODE_ATTACK_MADE`. Checks (spec 067, `mutations_roll_check.rs`)
  write a roll record.
- **Chat** (`mutations_chat.rs`) is the pattern to follow:
  - The world event carries only `{ messageId }`, and each client fetches
    the message.
  - The fetch applies the `gm_only` filter on the server, so a hidden body
    never rides the bus.
  - Only the GM can send `gm_only` today.
- **The character sheet page** (`/world/:id/actor/:actorId/view`) has no
  roll buttons. The rolls a character can make (`characterRolls.ts`) are
  offered only by the in-pane sheet on the play view.
- **The demo.**
  - Each tab runs its own copy of the in-browser backend
    (`apps/demo/src/backend/state.ts`): one object, saved to
    `localStorage`.
  - Two demo tabs are two worlds. Neither hears the other's events, and
    each overwrites the other's saved state.
  - Its dice (`handlers/dice.ts`) mirror the server's.
- **Tabs talking.** The app already uses `BroadcastChannel` in
  `services/worldCache.ts` and `api/sessionExpiry.ts`. The service worker
  (`public/sw.js`) is deliberately a tombstone (spec 028), and stays one.

## Visibility

Every roll has one of three visibilities. The roller chooses it, and the
server enforces it.

| Visibility    | Who may choose it | The roller and the GM see | Other players see                            |
| ------------- | ----------------- | ------------------------- | -------------------------------------------- |
| **Everyone**  | anyone            | the whole roll            | the whole roll                               |
| **GM's eyes** | a player          | the whole roll            | that the player rolled for the GM, as `****` |
| **GM only**   | the GM            | the whole roll            | nothing: the roll does not exist for them    |

"The whole roll" means the formula, each die, the total and the label
(such as "Stealth"). `****` means the roller's name, the time and the words
"rolled for the GM". There is no formula, no die count, no label and no
number.

**Revealing.** The GM may reveal a GM's eyes roll or a GM only roll to
everyone. The roll is then published again, as if it had just been rolled
in the open:

- every board animates it;
- every feed shows the real result;
- the entry says the GM revealed it.

A reveal cannot be taken back. A roll that is already visible to everyone
cannot be revealed.

## User Scenarios & Testing

### User Story 1 - Everyone sees a roll (Priority: P1)

A player rolls a d20 from the dice roller. On every board in the world, the
player's own included, the dice tumble and land on the server's numbers.
Every feed shows who rolled, the formula, the dice and the total.

**Why this priority**: This is the shared moment, and everything else here
builds on it.

**Independent Test**: Two members in two browser contexts. One rolls. Both
see the same dice values animate and the same entry in the feed.

**Acceptance Scenarios**:

1. **Given** a GM and a player on the play view,
   **When** the player rolls `1d20+3`,
   **Then** both boards animate dice that land on the server's values, and
   both feeds show the same entry.
2. **Given** a member who joins after the roll,
   **When** they open the play view,
   **Then** the roll is in their feed's backscroll, and no dice animate for
   it.
3. **Given** a roll the server refuses (a bad formula, or a world where the
   caller is not a member),
   **When** it is attempted,
   **Then** no board animates anything, and the roller sees the reason.
4. **Given** the rolling tab is also showing the board,
   **When** it rolls,
   **Then** it animates once, from the server's event. The local answer
   alone never plays it.

---

### User Story 2 - A sheet in another tab rolls onto the board (Priority: P1)

A player keeps their character sheet open in a second tab and the board in
the first. They click Stealth on the sheet. The dice land on the board in
the first tab, and on everyone else's.

**Why this priority**: This is the owner's question. It is answered by US1
plus a sheet page that can roll.

**Independent Test**: One player, two tabs: the play view and the sheet
page. Roll from the sheet, and see the play view's board animate it.

**Acceptance Scenarios**:

1. **Given** the sheet page of a character the player controls,
   **When** it loads,
   **Then** it offers the same rolls the in-pane sheet offers, from the same
   list (`characterRolls.ts`).
2. **Given** the sheet in tab B and the board in tab A,
   **When** the player rolls Stealth in tab B,
   **Then** tab A's board animates it, and tab B shows the result too.
3. **Given** an attack offered on the sheet page,
   **When** it is chosen,
   **Then** it goes through the attack flow (spec 046), which needs a
   target on the board. The sheet page says to pick the target on the
   board and does not roll a bare formula.

---

### User Story 3 - A player rolls for the GM's eyes (Priority: P1)

A player wants to know how sneaky they were without the other players
knowing. They choose "GM's eyes" and roll. They and the GM see the number.
Everyone else sees that the player rolled for the GM, as `****`.

**Why this priority**: This is the owner's explicit ask, and it is the
case where a leak would matter.

**Independent Test**: Three members, GM and two players. Player 1 rolls
for the GM's eyes. Player 2's client never receives the formula or the
number, in its rendered feed or in any network answer.

**Acceptance Scenarios**:

1. **Given** player 1 rolls `1d20+7` for the GM's eyes,
   **When** the roll lands,
   **Then**:
   - player 1 and the GM see the dice and the total;
   - player 2's feed shows "Player 1 rolled for the GM: \*\*\*\*";
   - no dice animate on player 2's board.
2. **Given** player 2's browser,
   **When** every request and subscription message it received is
   inspected,
   **Then** none contains the formula, a die value, the total or the label.
3. **Given** a player who tries to roll as **GM only**,
   **When** the server receives it,
   **Then** it is refused. Only the GM may roll unseen.

---

### User Story 4 - The GM rolls behind the screen (Priority: P2)

The GM rolls a monster's attack or a random encounter as **GM only**.
Players see nothing at all, not even that a roll happened.

**Why this priority**: GMs expect it. It costs little once US3 exists.

**Independent Test**: The GM rolls GM only. A player's feed, board and
network traffic show no trace of it.

**Acceptance Scenarios**:

1. **Given** the GM rolls GM only,
   **When** it lands,
   **Then** the GM's board animates it and the GM's feed shows it marked as
   hidden. A player's feed has no entry, and their board animates nothing.
2. **Given** a player's client,
   **When** it fetches the feed's backscroll,
   **Then** the GM only roll is not in the answer.

---

### User Story 5 - The GM reveals a hidden roll (Priority: P2)

The rogue's stealth check mattered after all. The GM clicks Reveal on it.
Everyone's board rolls it out in the open, and everyone's feed now shows
the real number, marked as revealed by the GM.

**Why this priority**: The owner's ask, and it completes US3 and US4.

**Independent Test**: Reveal a GM's eyes roll. Player 2's masked entry
becomes the real roll, and their board animates the original dice.

**Acceptance Scenarios**:

1. **Given** a GM's eyes roll that player 2 sees as `****`,
   **When** the GM reveals it,
   **Then** player 2's entry shows the formula, the dice and the total, and
   their board animates those dice.
2. **Given** a GM only roll,
   **When** the GM reveals it,
   **Then** it appears in every player's feed at its original time, with
   its dice animated now.
3. **Given** a player,
   **When** they try to reveal anything, their own GM's eyes roll included,
   **Then** the server refuses.
4. **Given** a roll already visible to everyone,
   **When** a reveal is asked for,
   **Then** nothing changes, and nothing is published again.

---

### User Story 6 - The demo does all of this, across tabs (Priority: P2)

In the demo, a visitor opens the sheet in a second tab and rolls. The board
in the first tab animates it, exactly as in a real game.

**Why this priority**: The demo is how people meet the product (spec 074),
and the owner named it.

**Independent Test**: The demo in two tabs of one browser. Roll from the
sheet in tab B and see tab A's board animate it. Reload both tabs and see
one world, not two.

**Acceptance Scenarios**:

1. **Given** the demo open in two tabs,
   **When** a token is moved or a roll is made in either,
   **Then** the other shows it, as two tabs of a real world would.
2. **Given** two demo tabs,
   **When** both are reloaded,
   **Then** both show the same world. Neither tab's changes are lost to
   the other's save.
3. **Given** the tab that is holding the demo world closes,
   **When** the other tab acts next,
   **Then** it takes the world over, and nothing that was saved is lost.
4. **Given** the demo's viewer is switched to a player,
   **When** they look at the GM's GM's eyes and GM only rolls,
   **Then** they see what a real player would: `****`, or nothing.

---

### Edge Cases

- **Dice that land late.** A member whose connection lags receives the
  event after others. They still animate it, unless the roll is older than
  a short window (a few seconds), so a reconnect does not replay a
  minute's worth of dice. The feed shows them all either way.
- **Catch-up after a drop.** Rolls missed during a drop arrive through the
  existing catch-up (`eventsSince`), and go into the feed without dice.
- **A member who becomes GM.** Visibility is checked when the roll is
  fetched, not when it was rolled. A player promoted to GM then sees
  hidden rolls; a GM demoted to player stops seeing them.
- **A member removed from the world** receives nothing more, as for every
  other event.
- **The roller leaves.** A GM's eyes roll keeps its visibility. The GM can
  still reveal it, and the feed shows the roller's name as recorded.
- **Two reveals at once** (two GM tabs). The first wins. The second
  changes nothing, and nothing is published twice.
- **Attacks and checks.** These stay visible to everyone in this spec. An
  attack's result changes hit points that every player can see, so hiding
  its dice would hide nothing. The visibility choice is offered on free
  rolls and on the sheet's non-attack rolls.
- **Many rolls at once** (a GM rolling ten initiative dice). Each is one
  roll record and one event. The board may run them together.
- **A browser without `BroadcastChannel` or Web Locks** (demo only). The
  demo works in one tab as it does today. A second tab says the demo is
  already open elsewhere, instead of silently forking the world.

## Requirements

### Functional Requirements

**Publishing a roll**

- **FR-001**: Every roll the server records MUST record a world event in
  the same transaction: `rollDice`, a sheet roll, a check, and the roll
  inside an attack. The attack's existing event MAY carry the roll's id
  instead of a second event.
- **FR-002**: The event MUST carry only the roll's id and its visibility,
  never the formula, the dice, the total or the label. This is chat's
  rule (`mutations_chat.rs`) for the same reason: every member's
  subscription sees every event on the world.
- **FR-003**: A client MUST learn a roll's content by fetching it. The
  fetch MUST apply visibility on the server for the caller:
  - the whole roll for the roller, the GM and an admin;
  - for another player, a GM's eyes roll comes back masked, and a GM only
    roll does not come back at all.
- **FR-004**: A masked roll MUST carry only the roll's id, the roller's
  display name, the time, the visibility and the fact that it is masked.
  The type that carries it MUST have no field that could hold the
  formula, a die value, the total or the label, so that a mistake in the
  resolver cannot leak them.
- **FR-005**: The feed MUST read through one query that pages backwards
  with the same visibility rules, open to every member, not only the GM.
  The GM's full `worldRollRecords` history stays as it is.
- **FR-005a**: A GM only roll MUST be invisible to a player at every
  level:
  - the event is not delivered to that player's subscription, or is
    delivered as a "nothing to fetch" the client ignores;
  - planning picks which, as long as a player's client cannot tell from it
    that a roll happened;
  - the fetch answers as if the id did not exist.

**Visibility**

- **FR-006**: A roll MUST carry a visibility: `everyone`, `gm_eyes` or
  `gm_only`. When none is given, it is `everyone`.
- **FR-007**: Only a player may roll `gm_eyes`, and only the GM may roll
  `gm_only`. The server MUST refuse the other combinations with a reason.
  A GM rolling `gm_eyes` would mean nothing, so it is refused, not
  quietly turned into `gm_only`.
- **FR-008**: The dice roller and the sheet (in the pane and on the sheet
  page) MUST offer the choice. A player sees "Everyone" and "GM's eyes";
  the GM sees "Everyone" and "GM only". The choice stays as last used, per
  device.

**Revealing**

- **FR-009**: The GM (or an admin) MUST be able to reveal a `gm_eyes` or
  `gm_only` roll. A reveal:
  - records who revealed it and when;
  - makes the roll visible to everyone from then on;
  - records a new world event that tells every client to fetch it again
    and to animate it.
- **FR-010**: Revealing MUST be idempotent. Revealing a roll that is
  `everyone` or already revealed changes nothing and records no event.
- **FR-011**: Revealing MUST NOT change the roll's dice, total or time.
  The feed keeps the roll at its original place and marks it "revealed by
  the GM".

**The board**

- **FR-012**: Every board MUST animate a roll from the roll event (after
  fetching it), never from a mutation's answer. A board animates:
  - a roll it may see whole, when it arrives live;
  - a revealed roll, when the reveal arrives live.
    It never animates a masked roll, a roll from catch-up, or a roll older
    than the replay window.
- **FR-013**: The rolling tab MUST still show its own result in the panel
  that rolled, from the mutation's answer. It MUST NOT animate it a second
  time.

**The sheet page**

- **FR-014**: The character sheet page MUST offer the same rolls as the
  in-pane sheet, from the same `characterRolls.ts` list, to anyone who may
  roll for that character. Attacks MUST say to pick the target on the
  board (US2 scenario 3).

**The demo**

- **FR-015**: The demo MUST hold one world per browser, shared by its
  tabs:
  - one tab holds it, chosen with Web Locks (`navigator.locks`);
  - the others send their requests to that tab;
  - every tab receives the world's events, over `BroadcastChannel`.
- **FR-016**: When the holding tab closes, another tab MUST take the world
  over from what was saved, without losing a saved change.
- **FR-017**: The demo's dice handler MUST apply the same visibility,
  masking and reveal rules for its current viewer. They MUST be tested
  against the same cases as the server's.
- **FR-018**: No service worker is involved. `public/sw.js` stays the
  tombstone spec 028 made it.

### Key Entities

- **Roll record** (`world_roll_records`), existing, gains:
  - `visibility` (`everyone` | `gm_eyes` | `gm_only`, not null, default
    `everyone`);
  - `label` (nullable: "Stealth", "Longsword");
  - `revealed_at` and `revealed_by` (nullable).
- **Roll event**: two world event codes. One says a roll was made, one
  says a roll was revealed. Each carries `{ rollId, visibility }`.
- **Masked roll**: the shape a player gets for someone else's `gm_eyes`
  roll. It has its own type, with no field for hidden content (FR-004).

## Success Criteria

### Measurable Outcomes

- **SC-001**: A roll made by one member animates on every other member's
  board within 1 s on a local stack. The values are the server's on every
  board.
- **SC-002**: A roll made from the sheet page in one tab animates on the
  play view in another tab of the same browser, in the real game and in
  the demo.
- **SC-003**: Across the e2e run for US3 and US4, a non-GM player's
  browser receives zero bytes of a hidden roll's formula, dice, total or
  label. This is checked on every subscription message and every response
  the page received, not only on what it rendered.
- **SC-004**: A reveal reaches every member's feed and board within 1 s,
  and a second reveal of the same roll publishes nothing.
- **SC-005**: Two demo tabs, after any sequence of edits in either and a
  reload of both, show one identical world.

### Proof

- **Server tests** cover:
  - each visibility, fetched as the roller, the GM, another player and an
    admin;
  - the masked type's shape;
  - the refused combinations (a player rolling `gm_only`, the GM rolling
    `gm_eyes`, a player revealing);
  - idempotent reveal;
  - the event's payload carrying no content.
- **Demo tests** run the same visibility cases against `handlers/dice.ts`,
  and cover the takeover when the holding tab closes.
- **An e2e slice, `pnpm e2e:rolls`**:
  - two and three members across browser contexts: US1, US3, US4, US5;
  - one member in two tabs, sheet to board: US2;
  - a network capture on the other player for SC-003.
- **The demo's e2e** covers two tabs of one browser for US6.

## Assumptions

- **Feed placement.** Rolls appear in the play view's chat panel, between
  messages in time order, as roll entries. A separate log is not added.
- **The roller sees their own GM's eyes roll.** "GM's eyes" means hidden
  from the other players, not from the roller. A blind roll, hidden from
  the roller too, is out of scope.
- **Replay window.** A few seconds. It is tuned in planning against the
  dice's settle time (`SETTLE_DURATION_SECS` in `plugins/dice_roll.rs`).
- **Scope.** Attacks and checks stay public (see Edge Cases). Hiding them
  is a later spec if a game system needs it.
- **Retention.** Roll records are kept as they are today; this spec adds
  no expiry.
- **Hiding the dice count.** A masked roll does not say how many dice were
  rolled. Players learn only that a roll for the GM happened, and when.
