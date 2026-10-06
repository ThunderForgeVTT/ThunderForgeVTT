# Feature Specification: Sight

**Feature Branch**: `076-sight`
**Created**: 2026-10-05
**Status**: Implemented 2026-10-06 (engine, web and demo e2e); open questions below still stand
**Input**: The owner, running the demo on 2026-10-05: "walls dont obscure vision like expected" (with Foundry's vision screenshots as the picture of expected), then "both" views, "if a gm clicks on a token the view should be as the token", and "if the gm hits esc it should take em back to gm view".

## Why

Spec 045 built vision around light. A scene is bright, dim or dark; walls
stop light; darkness is drawn where no light reaches; and a player's own
token decides which *tokens* they are shown (FR-030). In a bright scene
nothing is shaded at all: the player sees the whole map, every wall, every
room they have never entered, and only the monsters behind walls are
withheld. On the demo's outdoor maps, which are bright, that looks like
walls doing nothing — because to the map they do nothing.

The table expects what every other tabletop draws: a token sees what is in
its line of sight, and the rest of the map is not shown, in daylight as
much as in the dark. Light decides how far and how well a token sees; walls
decide where it can see at all. Both apply, always.

The Game Master's half is the same want from the other chair. Their board
shows everything, and marks what the party cannot see (FR-033). That is
right for running the table and useless for the question a Game Master
asks constantly: *what does this one see from there?* Today the only way
to find out is to sign in as the player. The owner's answer is the usual
one: select a token and the board becomes that token's view; press Escape
and it is the Game Master's again.

## What exists

Counted on 2026-10-05:

- `systems/lighting_vision.rs`: `ViewerToken`, set by the page from the
  viewer's own token (`WorldPage.tsx`, `setViewerToken`), and the pass that
  decides what that token can see, through walls and doors, every frame.
- `plugins/lighting_overlay.rs`: draws darkness where no light reaches in
  a dim or dark scene. In a bright scene it draws nothing.
- `plugins/exploration.rs`: remembers the cells the viewer's token has seen
  (spec 045 US7), drawn faintly into `CanvasLayer::Fog`, when the Game
  Master turns it on for the scene.
- Token culling (`token_culling_tests.rs`): hides tokens the viewer cannot
  see, and marks them for a Game Master.
- The keyboard already follows the single selected token and falls back to
  the viewer's own (`120284e1`, 2026-10-05), by the same reconcile pattern
  this spec reuses for sight.
- Escape does **not** clear a token selection today. In the engine it
  cancels a planned route (`token_move.rs`) and a shape, wall or placement
  in progress; a token stays selected until empty board is clicked. FR-008
  is new behaviour.

## Decisions already made

- **Sight bounds the map, not only the tokens.** Outside the viewing
  token's line of sight the map is not shown, in every ambient light. Light
  still governs how far and how well: in a dim or dark scene the lit area
  within line of sight is what is seen, as today.
- **Unseen is dark; remembered is faint.** Where the viewer has never seen is
  drawn as darkness. Where exploration is on and the viewer has seen is drawn
  as the exploration layer draws it today (faint, under the live view). A
  scene with exploration off remembers nothing, as today.
- **A Game Master's board follows their selection.** With exactly one token
  selected, the board is drawn as that token sees it: its line of sight, its
  light, the tokens it can see. With none selected, or a stack, the board is
  the Game Master's as it is today, everything shown and the party's blind
  spots marked.
- **Escape is the way back.** Escape clears the token selection (which it
  does not do today), and the board is the Game Master's again on the next
  frame. No second control. A planned route is cancelled by the same press,
  as it is now; the two do not conflict.
- **A player's view is unchanged in kind.** A player sees through their own
  token (FR-001) and selection does not change that; a player who selects
  another token may move it if the server allows, and still sees from their
  own.
- **The rules stay the engine's.** Nothing outside the ECS decides what is
  visible (spec 045, Principle I). The page names the viewer and the
  selection; the engine draws the consequence.

## Requirements

### Sight on the map

- **FR-001** For a viewer with a viewing token, the map outside that
  token's line of sight (walls that block vision, closed doors) MUST NOT be
  shown, in a bright scene as in a dim or dark one. Spec 045 FR-030, FR-031
  and FR-032 continue to govern which tokens are drawn within it.
- **FR-002** Where the viewer has never seen MUST be drawn as darkness.
  Where exploration is on and the viewer has seen MUST be drawn as the
  exploration layer draws it today. The two MUST be distinguishable at a
  glance, by the same rule `REMEMBERED_ALPHA` states.
- **FR-003** A light the viewer's token cannot see (a lantern in a sealed
  room) MUST NOT light the viewer's map, and a lit area the token can see
  MUST be shown as lit; a brightly lit room glimpsed through an open door is
  seen as far as the door's opening lets it.
- **FR-004** FR-001 and FR-002 MUST hold within one second of any change
  spec 045 FR-034 names, with no reload.
- **FR-005** A player with no token in the scene MUST continue to see the
  board as it is lit (spec 045 FR-035).

### A Game Master looks through a token

- **FR-006** When exactly one token is selected on a Game Master's board, the
  board MUST be drawn as that token sees it under FR-001 to FR-003 and spec
  045 FR-030 to FR-032, using the token's own sight (a game system's
  darkvision and range, spec 045 US5) where the system defines it.
- **FR-007** With no token selected, or more than one, the board MUST be the
  Game Master's as spec 045 FR-033 defines it.
- **FR-008** Escape MUST clear the selection and so return the board to
  FR-007. Any other way the selection is cleared (clicking empty board,
  switching tools) MUST do the same, because the rule is the selection, not
  the key.
- **FR-009** The authoring aids a Game Master is drawn above the darkness
  today (wall and light markers, spec 045) MUST stay drawn while looking
  through a token, so a Game Master checking a wall's effect can still see
  the wall.
- **FR-010** The marks of FR-033 MUST NOT be drawn while looking through a
  token; they answer a different question.
- **FR-011** The keyboard's token (`120284e1`) and the sight token MUST be
  the same token whenever one is selected, so a Game Master walking a
  monster sees as the monster walks.

### Proof

- **FR-012** Engine tests: given one wall and one viewer, a cell behind the
  wall is unseen and a cell beside it is seen, in a bright scene; the same
  with the viewer named by selection rather than `ViewerToken`; and clearing
  the selection restores the all-seeing state in one frame.
- **FR-013** A web e2e on the full stack: the Game Master draws a wall,
  selects the hero on one side, and a probe reports the cell on the far side
  unseen; Escape, and the probe reports it seen. The same scene, as the
  player, with no selection.
- **FR-014** A demo e2e: on Grassy Path Ambush, as the player, a wall drawn
  by the Game Master across the road hides the road beyond it; as the Game
  Master, selecting the fighter does the same and Escape undoes it.

## Success Criteria

- **SC-001** On a bright scene with one wall, a player's screenshot shows
  darkness on the far side of the wall and the map on the near side.
- **SC-002** A Game Master can answer "what does the goblin see" by one
  click and return by one key, without leaving the table or signing in as
  anyone.
- **SC-003** Frame time with sight on the map is within the budget spec 028
  measures today; the sight polygon is already computed every frame, so the
  cost is the overlay, not the geometry.
- **SC-004** `pnpm playtest --only=dungeon-crawl` and the full web e2e suite
  stay green; the crawl's vision steps are rewritten where they asserted
  that a bright scene is fully visible.

## How it was built (2026-10-06)

- The darkness sheet (`plugins/darkness.rs`, `darkness.wgsl`) carries one
  more shadow-map row, `SIGHT_ROW`, for the viewer's own line of sight; the
  shader draws a fragment outside it in the scene's darkness tint at no less
  than `UNSEEN_STRENGTH` (0.92) before it consults any light, so a light the
  viewer cannot see lights nothing (FR-003). The sheet is spawned in a bright
  scene whenever someone looks through a token.
- One resource, `Eyes` (`systems/lighting_vision.rs`), names the viewing
  token for illumination and darkness alike: a Game Master's single selected
  token, else `ViewerToken`. Exploration still keys on `ViewerToken` only, so
  a Game Master's glance through a monster remembers nothing for anyone.
- Escape clears the token selection (`clear_token_selection_on_escape`) and
  tells the page so; the keyboard's token and the sight token are the same
  resource (FR-011).
- The proof reads the row the shader reads: `sight_probe(x, y)` (
  `plugins/darkness_probe.rs`) answers with the shader's own bin and unpack
  arithmetic, and `window.__engineProbe.sight` exposes it. The demo is only
  ever a production build, so its e2e builds with `VITE_ENGINE_PROBE=1`; the
  image builds the demo itself and never carries the probe.
- The sight row has `SHADOW_BINS` (512) directions, so at the far reach of a
  large view a wall's edge is a little coarse angularly. Accepted; raise the
  bins if it is ever seen at the table.

## Open questions for the owner

- Darkness for the unseen: pure black, or the same colour as a dark scene's
  darkness (which has a hint of blue in it today)? Black is the usual
  tabletop answer and reads as "not there"; one colour everywhere is one
  less thing to explain.
- Should a Game Master looking through a token also *hear* as it — see the
  chat and rolls a player would — or is this strictly the board? The
  decision above is strictly the board.

## What this spec does not do

- It does not add fog of war that persists for the whole party, or shared
  party vision; exploration stays per viewer as spec 045 US7 built it.
- It does not change movement, walls, doors or lights themselves.
- It does not change what a player may select or move.
