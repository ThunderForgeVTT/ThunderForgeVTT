# Feature Specification: Tokens Talk

**Feature Branch**: `103-tokens-talk`

**Created**: 2026-10-16

**Status**: Draft

**Input**: The owner, 2026-10-16, trying spec 102's phone board: "new messages
in chat float above the hot bar when a user says it and above the token who
said it showing the token name (player name or gm) same with rolls i wanna see
those floating above the tokens too like what type of roll and the results we
can hide the formula on the floats … if the token is off the view port have it
as a float from the right side in order of most recent and floats up before
dismissing after a few seconds unless the user opens the chat then all the
floats vanish so they can just read the whole log". Then: "the idea is we want
to see tokens "talking" bc the player is doing it lets spec that please".

## Why

A table talks through its characters. Today the board does not show it:

- A chat message records who typed it and nothing else. It cannot say which
  character spoke, so the board has nothing to show it on.
- Chat and rolls live in a log behind a tab (a sheet on the phone). Someone
  watching the map sees nothing happen when a character speaks or a sword
  swings; they find out by opening the log.
- A roll already knows the character it was made for, and its type and
  result, but only the log shows them.

The point is the table: a player speaks, and everyone sees their character
say it on the map.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A player's character says it on the map (Priority: P1)

A player types a line in chat. Everyone at the table sees it rise as a speech
bubble over the player's character on the map, with the character's name, and
fade after a few seconds. The log keeps the line as before.

**Why this priority**: This is the feature. A line over a token is what makes
the board a table and not a map beside a chat room.

**Independent Test**: Two browsers on one world and scene, a player and the
Game Master. The player sends a line; the Game Master sees a bubble with that
line and the character's name over the player's token, and it goes after a
few seconds.

**Acceptance Scenarios**:

1. **Given** a player controls a character with a token on the current scene,
   **When** they send a chat line, **Then** every viewer of that scene who may
   see the token sees a bubble over it holding the line and the character's
   name, within 2 seconds.
2. **Given** a bubble is showing, **When** about 6 seconds pass, **Then** it
   fades out; the line is still in the log.
3. **Given** a token speaks twice in quick succession, **When** the second
   line arrives, **Then** it replaces the first over that token, so a token
   never wears a pile of bubbles.
4. **Given** a player speaks as nobody (no character), **When** the line is
   sent, **Then** it floats from the right edge (User Story 3) under the
   player's name, since there is no token to put it over.
5. **Given** the viewer pans or zooms the map, **When** a bubble is showing,
   **Then** it stays over its token.

---

### User Story 2 - Rolls show over the character who rolled (Priority: P1)

A player rolls from their sheet, an attack or a skill. Over their character a
float shows what the roll was and what it came to ("Longsword attack 17",
"Perception 12"), without the formula. The log keeps the formula, the dice and
the detail.

**Why this priority**: Rolls are the other half of what a table watches; the
owner asked for both together.

**Independent Test**: The player rolls Perception from their sheet; the Game
Master's browser shows a float over the player's token with "Perception" and
the total, and no formula.

**Acceptance Scenarios**:

1. **Given** a roll made for a character with a token on the current scene,
   **When** it lands, **Then** viewers who may see the roll see a float over
   that token with the roll's name (or type when it has no name) and its
   result, and no formula.
2. **Given** a roll with no character (the dice tray), **When** it lands,
   **Then** it floats over the roller's own character if they speak as one,
   and from the right edge under their name otherwise.
3. **Given** a roll whose result is a word, not a number (a coin, a success),
   **When** it floats, **Then** it shows that word.
4. **Given** a roll only the Game Master sees, **When** it lands, **Then** it
   floats for the Game Master and for nobody else; **When** the Game Master
   later reveals it, **Then** it floats then for those who can now see it.

---

### User Story 3 - Someone speaks out of view (Priority: P1)

The speaking token is somewhere the viewer cannot see: panned away, on the
map behind an open panel, or hidden from them. The float comes in from the
right edge instead, stacked with the others, newest at the bottom nearest the
controls, older ones rising above it, each fading after a few seconds.

**Why this priority**: On a phone the view shows a small part of the map, and
the speaker is often out of it. Without this, most lines would show nowhere.

**Independent Test**: Pan the viewer's map so the player's token is out of
view; the player speaks; the viewer sees the float at the right edge with the
character's name, and it fades.

**Acceptance Scenarios**:

1. **Given** the speaking token is outside the viewer's visible map, **When**
   the line or roll arrives, **Then** a float enters from the right edge with
   the name, newest first, and rises and fades after about 6 seconds.
2. **Given** several floats arrive close together, **When** they show,
   **Then** they stack without overlapping one another or any control, and at
   most 4 show at once; a fifth pushes the oldest out.
3. **Given** a token the viewer may not see (hidden, or out of their
   character's sight), **When** it speaks or rolls, **Then** the float never
   appears over where that token is; it goes to the right-edge stack, so the
   line is heard without the token being found.
4. **Given** a right-edge float, **When** the viewer taps it, **Then** the
   view moves to that token if the viewer may see it, and the chat opens if
   not.

---

### User Story 4 - Opening chat quiets the board (Priority: P2)

Someone opens the chat to read. Every float goes at once; while chat is open,
nothing new floats, since they are reading it there. Closing chat lets new
lines float again; what arrived while it was open does not replay.

**Why this priority**: The owner asked for it; floats over a log that is
already open repeat it and cover the map.

**Independent Test**: With a bubble and a right-edge float showing, open chat:
both are gone. Send a line while open: no float. Close chat, send another:
it floats.

**Acceptance Scenarios**:

1. **Given** floats are showing, **When** the viewer opens chat, **Then** all
   of them go at once.
2. **Given** chat is open, **When** a line or roll arrives, **Then** it does
   not float, now or after chat closes.

---

### User Story 5 - Choosing who speaks (Priority: P2)

Next to the chat composer the speaker sees who they are speaking as, and can
change it. A player speaks as their own character by default and can pick
another token they control, or themselves. The Game Master speaks as "Game
Master" by default and can pick any token on the scene, so a goblin can
growl.

**Why this priority**: A default makes User Story 1 work with no choice at
all; picking is what a Game Master voicing a cast, or a player with a
companion, needs.

**Independent Test**: The Game Master picks a goblin token and sends a line;
the player sees the bubble over the goblin with the goblin's name.

**Acceptance Scenarios**:

1. **Given** a player with one character on the scene, **When** they open
   chat, **Then** it shows they speak as that character, with no choice made.
2. **Given** a player, **When** they open the speaker choice, **Then** it
   lists only tokens they control on the scene and themselves.
3. **Given** the Game Master, **When** they open the speaker choice, **Then**
   it lists "Game Master" and every token on the scene.
4. **Given** a speaker chose a token, **When** they return to the world later
   on the same device, **Then** the choice holds while that token is still on
   the scene, and falls back to the default when it is not.
5. **Given** a line said as a token, **When** it shows in the log, **Then**
   the log shows the token's name and who typed it.

---

### Edge Cases

- **A long line** is cut to fit the bubble with an ellipsis; the log holds it
  whole. A bubble never covers more than about a third of the map's width.
- **A token removed or moved to another scene** before its float ends: the
  float goes with it; a line for a token not on this scene goes to the
  right-edge stack.
- **Two tokens close together** both speaking: their bubbles stay readable
  (the later one sits above, never on top of, the earlier).
- **A viewer on another scene** sees no bubbles for this scene's tokens and no
  right-edge float for them; the log is unchanged.
- **The Game Master's whisper** (a GM-only line) floats only for the Game
  Master, as GM-only rolls do.
- **A reconnect** catching up on missed lines does not float them; only lines
  that arrive live float.
- **Reduced motion**: floats appear and disappear without rising or sliding.
- **The demo**, with one person switching between the Game Master's and a
  player's view, floats as the current view would.
- **A player with several characters on the scene** defaults to the one they
  selected last, or the first if none.

## Requirements *(mandatory)*

### Functional Requirements

**Who speaks**

- **FR-001**: A chat line MUST record, besides who typed it, which token it
  was said as, or none.
- **FR-002**: A player MUST be able to speak only as a token they control on
  the current scene, or as themselves; the world MUST refuse a line said as
  any other token.
- **FR-003**: The Game Master MUST be able to speak as any token on the
  current scene, or as "Game Master".
- **FR-004**: The default speaker MUST be the player's own character on the
  scene (the last selected, else the first), or themselves when they have
  none; for the Game Master, "Game Master".
- **FR-005**: The composer MUST show who the next line is said as, and let
  the speaker change it in one tap or click plus a pick.
- **FR-006**: The log MUST show a line said as a token under the token's name,
  with who typed it.

**What floats**

- **FR-007**: A chat line said as a token on the current scene MUST float over
  that token as a bubble holding the line and the token's name.
- **FR-008**: A line said as nobody MUST float at the right edge under the
  speaker's name ("Game Master" for the Game Master).
- **FR-009**: A roll for a character with a token on the current scene MUST
  float over that token with the roll's name or type and its result; the
  formula MUST NOT show on a float.
- **FR-010**: A roll with no character MUST float as the roller's current
  speaker would (FR-007, FR-008).
- **FR-011**: A float MUST be seen only by viewers allowed to see the line or
  roll; a GM-only line or roll floats for the Game Master only, and a revealed
  roll floats on its reveal for those who may now see it.
- **FR-012**: Only lines and rolls that arrive live MUST float; history loaded
  on opening the board or after a reconnect MUST NOT.

**Where it floats**

- **FR-013**: A float over a token MUST stay over it as the view pans, zooms
  or the token moves, until it fades.
- **FR-014**: When the token is outside the viewer's visible map (panned away,
  or under an open panel or sheet), the float MUST show in a stack at the
  right edge instead, newest nearest the bottom controls, at most 4 at once.
- **FR-015**: A token the viewer may not see (hidden, or out of their sight)
  MUST NOT be located by a float: its float goes to the right-edge stack.
- **FR-016**: A float MUST fade after about 6 seconds; a newer bubble over the
  same token replaces the older one.
- **FR-017**: Opening chat MUST clear every float, and no float MUST appear
  while chat is open; lines that arrived while it was open MUST NOT float later.
- **FR-018**: Tapping a right-edge float MUST move the view to its token when
  the viewer may see it, and open chat otherwise.
- **FR-019**: A float MUST NOT cover a control, and right-edge floats MUST NOT
  overlap each other.

**Access**

- **FR-020**: Floats MUST be announced to assistive technology politely,
  without taking focus or interrupting.
- **FR-021**: Float text MUST meet WCAG 2.2 AA contrast against its backing,
  whatever map is under it.
- **FR-022**: With reduced motion preferred, floats MUST appear and disappear
  without movement.
- **FR-023**: Floats MUST work on the desktop board and the phone board, in the
  app and in the demo.

### Key Entities

- **Chat line**: what was said, who typed it, whether only the Game Master sees
  it, and now which token it was said as (or none).
- **Speaker choice**: per viewer and world, who their next line is said as;
  remembered on the device.
- **Float**: a passing view of a line or roll for one viewer: its text (the
  line, or the roll's name and result), the name it shows, the token it
  belongs to if any, and whether it is over the token or at the right edge.
  Never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A line a player sends shows over their token in another
  viewer's board within 2 seconds, with the character's name, in 95% of
  sends on a local network.
- **SC-002**: A roll float shows its name and result and never a formula, in
  every roll type the e2e covers (check, save, skill, attack, tray roll,
  coin).
- **SC-003**: A token out of view always floats at the right edge, and a token
  the viewer may not see is never floated over its position: zero leaks
  across the e2e cases.
- **SC-004**: A player never sees a float for a GM-only line or roll.
- **SC-005**: Opening chat leaves zero floats on screen.
- **SC-006**: At most 4 right-edge floats show at once, none overlapping
  another or any control, at phone and desktop sizes.
- **SC-007**: The existing chat and roll e2e tests pass unchanged, apart from
  the log's new speaker name.
- **SC-008**: The owner plays the demo on their phone, sees their character
  talk and roll, and greenlights it before anything ships.

## Assumptions

- **Drawing**: the map and what sits on it are the engine's to draw (the
  project's first principle). Either the engine draws bubbles over tokens, or
  it reports where tokens are on screen and the page places them; the plan
  chooses. The right-edge stack is page chrome either way.
- **About 6 seconds** is how long a float lasts; the plan may tune it in the
  owner's phone test.
- **"Controls a token"** means what it already means for moving tokens: the
  token's character belongs to the player, or the Game Master gave them the
  token.
- **The right edge** sits above spec 102's bottom row on a phone, and above the
  dice tray on desktop, so floats never cover the controls.
- **No new rule about what a viewer may see**: floats follow the same
  visibility as the log, and the same token visibility as the map.
- **Lines already in the log** keep showing who typed them; they gain no
  speaker.

## Out of Scope

- Emotes, actions ("/me"), typing indicators, and voice.
- Speech bubbles kept on the map, or a transcript over the map; the log is
  the record.
- Rolls drawn as dice over tokens (the dice already throw on the board).
- Speaking as a token on another scene.
- Floats for the turn order, conditions, damage, or anything that is not a
  chat line or a roll.
