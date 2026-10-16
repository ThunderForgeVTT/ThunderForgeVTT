# Contract: Engine (SpeechPlugin)

All commands go through `apply_world_command(json)`. Events come back
through the existing event callback and are parsed in
`packages/encounter/src/engineEvents.ts`.

## Commands

### `say`

```json
{ "type": "say", "id": "chat:3f…", "tokenId": "9a…", "name": "Brakka",
  "text": "Hold the door!", "kind": "line", "ms": 6000 }
```

- The engine judges placement once (research R2):
  - The token exists, its `Visibility` is not `Hidden`, and its anchor (top
    centre of the token) lies inside the view shrunk by the insets: it draws
    a bubble as the token's child above the nameplate, replacing any bubble
    the token wears. It then emits `speech_placed { placement: "over" }`.
  - Otherwise it draws nothing and emits `speech_placed { placement: "edge" }`.
- `kind: "roll"`: the same bubble with the result emphasised.
- Under the existing `set_reduced_motion {reduced: true}` there is no rise or
  fade: the bubble appears and disappears. `ms` defaults to 6000 and is capped
  at 60000.
- The bubble wraps at 260 screen px (a third of a phone's width), and a line
  past 120 characters ends in "…" (about three lines).
- It despawns itself after `ms`.

### `clear_speech`

```json
{ "type": "clear_speech" }
```

Despawns every bubble at once.

### `set_view_insets`

```json
{ "type": "set_view_insets", "top": 0, "right": 0, "bottom": 180, "left": 0 }
```

The CSS px of the canvas that page chrome covers. The engine keeps the
values until the page changes them, and only `say` placement reads them.

## Event

### `speech_placed`

```json
{ "type": "speech_placed", "id": "chat:3f…", "tokenId": "9a…", "placement": "over" }
```

`placement` is `"over"` or `"edge"`. The page adds `"edge"` floats to its
stack, using the `name` and `text` it already holds for `id`.

## Read-only probe (tests)

`speech_bubbles(): string` returns a JSON array of
`{ id, tokenId, name, text, kind }` for every bubble alive now, modelled on
`token_nameplates()`. E2E reads it to assert a bubble is over a token without
pixel matching.

## Invariants

- A token never wears more than one bubble.
- No bubble is ever spawned on a token whose `Visibility` is `Hidden` for the
  current viewer (FR-015).
- `set_viewer_token` changing the viewer despawns bubbles on tokens that turn
  hidden.
