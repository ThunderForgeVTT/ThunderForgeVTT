# Implementation Plan: Tokens Talk

**Branch**: `103-tokens-talk` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/103-tokens-talk/spec.md`

## Summary

A chat line records which token it was said as; a roll from the dice tray
does too. Every board that receives a line or roll live hands it to the
engine with a `say` command. The engine draws the bubble as a child of the
token (as `nameplate.rs` draws names) when the viewer may see the token and it
sits inside the unobstructed view, and otherwise answers with a
`speech_placed { placement: "edge" }` event. The page draws only the
right-edge stack. Opening chat sends `clear_speech` and suppresses new
floats. The server validates the speaker the way it validates moving a token,
and withholds a hidden token's name from players.

## Technical Context

**Language/Version**: Rust 2024 (engine, wasm32; server, native), TypeScript 5 / React 19

**Primary Dependencies**: Bevy (engine), async-graphql + diesel (server), Vite + React (apps/web, apps/demo)

**Storage**: PostgreSQL. One migration adds speaker columns to
`world_chat_messages` and `world_roll_records`. The demo backend mirrors it in
memory.

**Testing**: `cargo test` (server speaker rules, engine placement), vitest
(demo backend, float queue), Playwright (two browser contexts in apps/web,
one switching person in apps/demo)

**Target Platform**: Browsers, desktop and phone (spec 102 phone board)

**Project Type**: Web app + wasm engine + GraphQL server

**Performance Goals**: A float shows within 2 s of send (SC-001). Bubbles add
no per-frame cost when none show; at most one bubble per token.

**Constraints**: WCAG 2.2 AA contrast on any map. Reduced motion is honoured,
and announcements are polite and never take focus. Nothing is pushed until
the owner's phone test and greenlight (SC-008).

**Scale/Scope**: A table of ~2–8 viewers. At most one bubble per token and at
most 4 edge floats per viewer.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. ECS owns simulation, React owns chrome**: PASS. Bubbles over tokens are
  canvas entities drawn by an engine plugin. The page never computes where a
  token is on screen. The right-edge stack is chrome off the map, placed by
  React from the engine's event.
- **II. Plugin-modular engine**: PASS. The new `plugins/speech.rs` (`SpeechPlugin`)
  owns its resource (`Speech`, the live bubbles) and systems. It reads token
  `Visibility` and the camera, never another plugin's internals. View insets
  are a resource it owns, set by a command.
- **III. Authorization at the data boundary**: PASS. `sendChatMessage` and
  `rollDice` validate `speakerTokenId` server-side
  (`controllers::may_move` for players, any token of the world for its Game
  Master). The stored label is the token's real name. The resolver withholds
  it from players when `player_may_read_token_name` says no. There is no new
  table; the provenance columns already exist.
- **IV. ADRs and specs**: This spec plus an ADR,
  `docs/adrs/20261016-117-speech_is_drawn_by_the_engine.md`, recording the choice of
  engine-drawn bubbles with an edge event over page-positioned bubbles
  (research R1). The ADR lands in the same change set.
- **V. Verify before done**: `cargo check --target wasm32-unknown-unknown` for
  the engine, native `cargo check` for the server, `tsc`/build for web and
  demo, and a running demo on the owner's phone.
- **VI. Every feature is proven by its own slice**: slice **`talk`**, run as
  `pnpm e2e:talk`.
  - Standalone `e2e:talk:standalone`: apps/demo `tokens-talk.spec.ts` (one
    person, GM and player views) plus demo vitest.
  - Integration `e2e:talk:integration`: `--slice=talk`, own prefix
    `tokens-talk-` in apps/web/e2e (two browser contexts).
  - Neighbours, by seam:
    - `chat-panel.spec.ts`: the composer gains the speaker choice, and the log
      a token name.
    - `rolls-everyone.spec.ts`: roll sync now also feeds floats.
    - `rolls-gm-only.spec.ts`, `rolls-gm-eyes.spec.ts` and
      `rolls-reveal.spec.ts`: who may see a roll, and so its float, and the
      float on reveal.
    - `dice-roll.spec.ts`: the tray passes the speaker.
    - `phone-board.spec.ts` (demo): the chat sheet clears floats, and edge
      floats sit above the bottom row.
  - **Cross-cutting note:** `apps/thunderforge/schema.graphql` is in
    `crossCutting`. By the owner's standing rule (never gate on e2e-parallel,
    even for schema changes), the proof is the slices `pnpm e2e:which --diff`
    names, not the full suite. The full suite stays a pre-release check.

No violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/103-tokens-talk/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── graphql.md       # chat + roll input/output changes
│   └── engine.md        # say / clear_speech / set_view_insets, speech_placed, speech_bubbles()
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/thunderforge-server/
├── migrations/2026-10-16-120000-0000_speaker_token/{up,down}.sql
├── src/schema.rs                         # speaker_token_id, speaker_label on both tables
├── src/models.rs
├── src/graphql/mutations_chat.rs         # speakerTokenId validated
├── src/graphql/mutations_roll.rs         # RollDiceInput.speakerTokenId
├── src/graphql/speaker.rs                # shared: resolve + validate a speaker token (new)
└── src/graphql/*chat*/roll query types   # speakerTokenId, speakerLabel (withheld)
apps/thunderforge/schema.graphql          # regenerated

crates/thunderforge-engine/src/
├── plugins/speech.rs                     # SpeechPlugin (new)
├── plugins/speech_tests.rs               # placement + replace + expiry (new)
├── plugins/mod.rs, lib.rs                # register; wasm `speech_bubbles()`
└── sdk.rs / world command dispatch       # say, clear_speech, set_view_insets

packages/encounter/src/engineEvents.ts    # speech_placed event + parser

apps/web/src/
├── components/world/TokenTalk/
│   ├── useTalk.ts                        # live lines+rolls → floats; chat-open suppression
│   ├── floatText.ts                      # roll → "Perception 12" (no formula)
│   ├── EdgeFloats.tsx                    # right-edge stack + polite live region
│   └── SpeakerPicker.tsx                 # composer's "Speaking as"
├── components/world/PlayDock/ChatPanel.tsx   # picker; log shows token name + typist
├── components/world/DiceRollerPanel/useDiceRoller.ts  # passes speakerTokenId
├── components/world/PhoneBoard/PhoneBoard.tsx        # insets, chat-open
└── pages/world/WorldPage.tsx             # mounts useTalk + EdgeFloats

apps/demo/src/backend/handlers.ts (+ chat.test.ts)  # mirror speaker rules
apps/demo/e2e/tokens-talk.spec.ts                    # standalone half
apps/web/e2e/tokens-talk-*.spec.ts                   # integration half
scripts/e2e/slices.json                              # `talk` entry
package.json                                         # e2e:talk scripts
docs/adrs/20261016-117-speech_is_drawn_by_the_engine.md
```

**Structure Decision**: The existing monorepo layout. The engine plugin
draws; the server validates and stores; apps/web holds the page hook and the
chrome; the demo mirrors the backend.

**Slice**: `scripts/e2e/slices.json` gains `talk` (own `tokens-talk-`,
neighbours above, `paths` covering the files in the tree). The root
`package.json` gains `e2e:talk`, `e2e:talk:standalone` and
`e2e:talk:integration`.

## Complexity Tracking

None.
