# Quickstart: proving Tokens Talk

## Prerequisites

- An e2e stack per checkout (`make e2e-stacks`), with
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` for the external
  stack.
- A dev engine build (`build.mjs`). Release is not needed; no perf claim is
  made.

## Fast checks

```sh
cargo check -p thunderforge-engine --target wasm32-unknown-unknown
cargo test  -p thunderforge-engine speech          # placement, replace, expiry, hidden never over
cargo test  -p thunderforge-server speaker         # FR-002/003 rules, label withheld
pnpm -F @thunderforge/demo test                    # demo mirrors the rules
pnpm -F @thunderforge/web typecheck
```

## Slice

```sh
pnpm e2e:talk:standalone    # demo: one person, GM and player views
pnpm e2e:talk:integration   # apps/web: two browser contexts
pnpm e2e:which --diff       # the neighbour slices to run besides (chat, rolls, phone)
```

## Scenarios the slice must show

| # | Scenario | Expected |
|---|---|---|
| 1 | The player sends "Hold the door!" | GM's `speech_bubbles()` lists it over the player's token, named after the character, within 2 s; gone after ~6 s; still in the log |
| 2 | The player rolls Perception | GM sees a float "Perception <n>"; no `d20` or `+` anywhere in it |
| 3 | The GM pans the player's token out of view; the player speaks | `edge-float` with the character name at the right edge; `speech_bubbles()` empty |
| 4 | The GM speaks as a hidden goblin | Player gets an edge float named "Someone"; no bubble over the goblin |
| 5 | A GM-only line and a GM-only roll | No float in the player's context; then reveal the roll → it floats for the player |
| 6 | Five lines quickly | At most 4 edge floats, none overlapping each other or the bottom row (phone) or tray (desktop) |
| 7 | Floats showing, then chat opens | Zero floats; a line sent while open never floats, even after closing |
| 8 | A player tries to speak as the GM's token (direct GraphQL) | `FORBIDDEN` |
| 9 | Reload mid-conversation | History does not float |

## Owner check (SC-008)

Run the demo through the tunnel on the owner's phone and play a line and a
roll. Nothing is pushed until they greenlight.
