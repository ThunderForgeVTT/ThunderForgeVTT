# Feature Specification: A Token Is Down

**Feature Branch**: `096-token-defeated`
**Created**: 2026-10-15
**Status**: Draft
**Input**: The landing's walk-along (spec 098): "so when you scroll the
monsters get little skull and bones over them when you scroll past
simulating combat so that's token death state in the engine." Spec 046
FR-020 already decides on the server when a combatant is out of the fight;
the board has never shown it.

## Overview

The engine draws a token the same way whether its creature is fighting or
lying on the floor. `DerivedStats.is_dead` was taken out long ago
(`components.rs`), and nothing replaced it on the board. Spec 046 knows a
combatant is out of the fight (`thunderforge-combat`, `order.rs` and
`hit_points.rs`), but a player reading the table has to ask.

This spec gives a token one more piece of state: **down**. A token that is
down is drawn greyed, with a skull and crossed bones over its middle. It
still stands where it fell, can still be selected and moved by whoever
could before, and comes back the moment the host says it is up.

The engine knows no game system (ADR-062). "Down" is not "dead", "dying",
"unconscious" or "0 HP"; those are a system's words. The host decides when
a token is down and tells the engine. The engine only draws it.

## User Stories

### US1: The host marks a token down (P1)

A host sends `upsert_token` with `defeated: true`. The token's art greys
and a skull and crossed bones appears over it. Sending `defeated: false`
takes both away. A payload with no `defeated` field leaves the state
alone, the way `conditions` and `scale` already work.

**Acceptance**:
1. A token placed with `defeated: true` is drawn down from its first frame.
2. A down token updated with only a new position is still down.
3. `defeated: false` brings the art back to full colour and removes the mark.

### US2: The mark reads at any zoom and size (P1)

The mark scales with the token's grid footprint, like condition badges, so
a Large token's skull is bigger than a Small one's and stays legible
zoomed out.

### US3: Tests can see it (P2)

A test export reports which tokens the engine draws as down, so an e2e
spec can prove the mark rather than compare pixels.

## Requirements

- **FR-001**: `WorldTokenPayload` gains an optional `defeated: bool`.
  Absent means unchanged, `true` means down, `false` means up.
- **FR-002**: A down token's art is drawn at reduced brightness and
  saturation. The original tint returns exactly when it is up again.
- **FR-003**: A down token carries a skull-and-crossed-bones mark centred
  on it, above the art and condition badges and below the nameplate. It is
  sized at about 60% of the token's side.
- **FR-004**: The mark follows the token's culling: a culled token builds
  no mark, and one coming back into view builds whatever its state is then.
- **FR-005**: Down changes nothing else. Selection, movement, vision,
  lights and ownership work as they did.
- **FR-006**: `token_defeated()` (wasm export) returns the ids the engine
  draws as down, as a JSON array, following `token_conditions()`.
- **FR-007**: The engine never decides on its own that a token is down.

## Success Criteria

- **SC-001**: Engine unit tests cover placing down, staying down across a
  position-only update, getting up, and culling.
- **SC-002**: The landing's goblins (spec 098) are shown down in an e2e
  spec through `token_defeated()`.
- **SC-003**: No change to frame time with no down tokens: the system
  skips tokens with nothing drawn and nothing to draw, as condition
  markers do.

## Out of scope

- The web client wiring 046's out-of-the-fight to `defeated`. That is a
  follow-up (F001) once this lands.
- Animating the fall.
