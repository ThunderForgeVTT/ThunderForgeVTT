# ADR-117: Speech Is Drawn by the Engine; the Page Owns Only the Edge

**Date:** 2026-10-16
**Status:** Proposed
**Participants:** ThunderForgeVTT Team
**Related:** spec 103 (tokens talk), spec 102 (mobile play board)

---

## Problem Statement

A line said in chat, or a roll, should float over the token that said it, so
the table sees characters talking rather than a log. The token lives on the
canvas; the chat lives in React. Who draws the bubble, and who decides when
a token is out of view and the line comes in from the edge instead?

## Decision

1. **The engine draws bubbles over tokens.** `plugins/speech.rs` spawns each
   bubble as a child of the token entity (a backing sprite and a `Text2d`),
   above the nameplate. It follows pans, zooms and moves with no per-frame
   positions crossing to the page. One bubble per token: a new line replaces
   the old.
2. **Placement is judged once, when the line arrives.** A token is "over"
   when its `Visibility` is not `Hidden` for the viewer and its anchor lies
   inside the camera view shrunk by the page's insets
   (`set_view_insets`). Otherwise the engine emits
   `speech_placed { placement: "edge" }` and draws nothing. A bubble stays
   with its token until it fades, even if the token walks out of view.
3. **The page owns only the right-edge stack.** It is chrome beside page
   controls: DOM, at most four floats, a polite live region, never taking
   focus. It is also the only place a hidden token's line can appear, and it
   names no position.
4. **The page sends, the engine draws.** `say`, `clear_speech` and
   `set_view_insets` are world commands; `speech_bubbles()` mirrors what is
   drawn so e2e can assert it.

## Alternatives

- **The engine exports token screen positions each frame; React places DOM
  bubbles.** Rejected: a second source of canvas truth, a wasm↔JS call per
  frame, and the page would hold hidden tokens' positions, which makes a
  fog leak possible.
- **The engine draws the edge stack too.** Rejected: the stack must sit
  beside the phone's bottom row and the dice tray without covering them, and
  needs DOM for the live region.
- **Re-judge placement every frame.** Rejected: a float would jump between
  the map and the edge as the camera moves, mid-read.

## Consequences

- Fog and vision decide what a float may reveal in the one place that knows
  them; the page never learns where a hidden token is.
- The page must report how much of the canvas its sheets and dock cover, or
  lines said behind them float over a token nobody can see.
- Bubble look (size, wrap, colours) is engine work and is tuned in Rust.
