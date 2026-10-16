# Research: Tokens Talk

## R1. Who draws a bubble over a token

- **Decision**: The engine. A new `SpeechPlugin` spawns a bubble (a `Text2d`
  on a backing sprite) as a child of the token entity, kept upright and above
  the nameplate, as `plugins/nameplate.rs` does for names. When the bubble
  cannot go over the token, the engine emits `speech_placed` with
  `placement: "edge"`, and the page puts it in the right-edge stack.
- **Rationale**:
  - Constitution I: what sits on the map is the engine's. As a child entity,
    the bubble follows pans, zooms and moves for free (FR-013), with no
    per-frame positions crossing to the page.
  - The engine already knows whether the viewer may see the token
    (`Visibility::Hidden` from lighting/vision) and where the view is
    (`ViewportCull`), so the leak rule (FR-015) is decided where the truth is.
- **Alternatives considered**:
  - Have the engine export token screen positions each frame and let React
    place DOM bubbles. Rejected: it makes a second source of canvas truth,
    costs a wasm↔JS call per frame, and makes leaks possible (the page would
    hold hidden tokens' positions).
  - Draw everything, edge included, in the engine. Rejected: the edge stack is
    chrome beside page controls (spec 102 bottom row, dice tray), needs DOM
    for tap targets and the live region, and must not cover controls.

## R2. When a token is "in view"

- **Decision**: Judged once, when the line arrives. The token is over-able
  when its `Visibility` is not `Hidden` for the current viewer and its bubble
  anchor lies inside the camera view shrunk by the page-reported insets
  (`set_view_insets { top, right, bottom, left }` in CSS px). Otherwise →
  edge. A bubble already over a token stays with it until it fades, even if
  the token walks out of view.
- **Rationale**:
  - Open phone sheets and the desktop dock cover the canvas. Only the page
    knows how much, so it reports the size and the engine decides.
  - Judging once keeps a float from jumping between map and edge mid-read.
- **Alternatives considered**: Re-judging every frame. Rejected because the
  float would flicker between the map and the edge as the camera moves.

## R3. Recording who spoke

- **Decision**: `world_chat_messages` and `world_roll_records` gain
  `speaker_token_id UUID NULL REFERENCES tokens(id) ON DELETE SET NULL` and
  `speaker_label TEXT NULL`. Both are set by the server, the label from the
  token's name at send time. Sheet rolls keep `actor_id` and leave the speaker
  null; the float maps actor → token. Tray rolls pass the composer's speaker.
- **Rationale**:
  - FR-001 and FR-010. A tray roll has no actor, so without the speaker no
    other viewer could place it.
  - The label survives the token's deletion for the log (FR-006).
- **Alternatives considered**:
  - Store only the token id and resolve the name at read time. Rejected: the
    log would lose the name when the token goes.
  - Map the tray roll to the speaker's actor. Rejected: an NPC actor has many
    tokens (goblins), so the float would land on the wrong one.

## R4. Who may speak as which token

- **Decision**: In a shared `graphql/speaker.rs`:
  - Load the token, then its scene, and require the scene's world to be the
    message's world. If the message names a `sceneId`, the token must be on
    it.
  - The world's Game Master may speak as any such token.
  - A player may speak as a token only if `controllers::may_move(conn, user,
    token)` is true (owner, owner grant on the actor, the same rule as moving).
  - Anything else is refused with `FORBIDDEN`, not downgraded, matching how
    `gmOnly` is refused.
- **Rationale**: Spec assumption ("controls" is what it means for moving). It
  reuses the audited rule, Constitution III.

## R5. A hidden token's name

- **Decision**: The stored `speaker_label` is the real name. The GraphQL
  field `speakerLabel` returns it to the Game Master, and to a player only
  when `npc_visibility::player_may_read_token_name` holds. Otherwise players
  get `"Someone"`. On a player's board the engine already shows that token
  hidden or nameless; the bubble uses the label the page received.
- **Rationale**: The same rule as nameplates, so a float can never name what
  the map does not.

## R6. Which arrivals float

- **Decision**: One page hook, `useTalk`, listens where the log already
  listens:
  - chat: `startPlayPanelEventSync({ onChatMessage })`, which fires per live
    world event and is followed by a refetch
  - rolls: the `startRollSync` upsert
  It keeps a set of ids seen at mount, plus a 10-second freshness window on
  `createdAt` (or `revealedAt` for a reveal). Only new, fresh, visible-to-me
  items float. A reconnect's catch-up refetch adds stale ids, which do not
  float (FR-012).
- **Rationale**: No second subscription. Catch-up and reveal are told apart
  by timestamps the records already carry.
- **Alternative**: A dedicated "speech" event on the bus. Rejected: the chat
  event carries only the message id, by design so GM-only text never leaks,
  and the refetch already filters by viewer.

## R7. Chat open

- **Decision**: Chat counts as open when the phone sheet id is `"chat"` or the
  desktop dock's chat section is open. On open the page sends `clear_speech`
  and empties the edge stack; while open, `useTalk` marks arrivals seen
  without floating them (FR-017).

## R8. Speaker choice

- **Decision**:
  - Kept in `localStorage` under `tf.speaker.<worldId>`, wrapped in try/catch.
  - Defaults:
    - a player: the last-selected owned token on the scene, else the first,
      else "self"
    - the Game Master: "Game Master"
  - The picker lists the scene's tokens filtered by the same client-side
    control check the move UI uses; the server is the authority.

## R9. Contrast and motion

- **Decision**:
  - Bubbles and edge floats use a fixed dark backing (≥90% opaque) with light
    text, so contrast is ≥ 7:1 on any map.
  - The name line is set in the accent colour, checked ≥ 4.5:1 on the backing.
  - Under `prefers-reduced-motion`, the page passes `motion: false` in `say`
    and the edge stack uses no transitions.
  - The edge stack is `aria-live="polite"`, holds no focusable element except
    the float buttons, and never calls `focus()`.

## R10. Lifetime

- **Decision**: 6000 ms by default, sent per `say` (`ms`) so the owner's phone
  test can tune it in one place (`TALK_FLOAT_MS`). One bubble per token: a new
  `say` for the token despawns the old bubble. At most 4 edge floats; a fifth
  drops the oldest.
