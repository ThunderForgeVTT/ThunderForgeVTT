# Contract: the engine's dice

This covers how the web tells the engine about a roll, and what the engine
says back. The commands go through `apply_world_command(JSON)`
(`crates/thunderforge-engine/src/sdk.rs`), as every command does.

## Command `trigger_dice_roll` (changed)

```json
{
  "type": "trigger_dice_roll",
  "roll": {
    "id": "0d9b…",
    "rollerName": "Ayla",
    "label": "Stealth",
    "formula": "1d20 + MODIFIER",
    "bindings": [{ "placeholder": "MODIFIER", "value": 3 }],
    "resultKind": "TOTAL",
    "resultValue": 16,
    "dice": [
      {
        "sidesKind": "NUMERIC",
        "numericSides": 20,
        "rolls": [13],
        "steps": [],
        "kept": true,
        "finalValue": 13
      }
    ]
  }
}
```

- The field names are the GraphQL names of `WorldRoll`. The web passes
  through what it fetched, with no renaming (`buildDiceThrow` in
  `apps/web/src/engine/bevy/diceThrow.ts`).
- In Rust, it is `ExternalCommand::TriggerDiceRoll { roll: DiceRollPayload }`
  in `payloads.rs`, with `#[serde(rename_all = "camelCase")]`. `steps` and
  `bindings` are `#[serde(default)]`.
- `resultKind` is `"TOTAL"` or `"SUCCESS_COUNT"`.
- The old shape, `{ "dice": [{ "finalValue": n }] }`, is no longer
  accepted. Its only sender is `triggerDiceRollAnimation`
  (`apps/web/src/engine/bevy/index.ts:1768`), which the demo also uses
  through the web's engine module. The sandbox gains a sender in this spec.
- A payload with an unknown `sidesKind`, or a missing `numericSides` for
  `NUMERIC`, plays as a disc showing `finalValue`.

## Command `set_reduced_motion` (new)

```json
{ "type": "set_reduced_motion", "reduced": true }
```

It is sent once when the engine is ready, and again on every
`matchMedia("(prefers-reduced-motion: reduce)")` change. It applies to the
next throw that leaves the queue.

## Export `dice_timings() -> String` (new)

```json
{
  "tumbleMs": 1200,
  "stepMs": 500,
  "holdMs": 2500,
  "fadeMs": 400,
  "reducedMs": 150
}
```

These are constants from `thunderforge_canvas_core::dice_throw::TIMINGS`.

The web reads them through `engineDiceTimings()` in `diceThrow.ts`. When the
engine module is not loaded, that returns `null`, and `DiceRollerPanel`
falls back to 1200 ms.

## Export `dice_landed() -> String` (new)

This is a JSON array of the throws the engine has finished or skipped,
oldest first, with at most 50 entries:

```json
[
  {
    "rollId": "0d9b…",
    "skipped": false,
    "reducedMotion": false,
    "readout": "Ayla: Stealth   13 + 3 = 16",
    "chip": null,
    "dice": [
      {
        "sides": 20,
        "face": 13,
        "kept": true,
        "succeeded": null,
        "rerolled": [],
        "clamped": null,
        "explosionOf": null,
        "restingPlace": [-32, 0]
      }
    ]
  }
]
```

| Field          | Meaning                                                                                          |
| -------------- | ------------------------------------------------------------------------------------------------ |
| `sides`        | The numeric sides, as a number. `"F"` for Fate and `"C"` for a coin. A d100 reports `100`, once. |
| `face`         | The value of the face the die came to rest on.                                                   |
| `rerolled`     | The values struck through on this die by a reroll, in order.                                     |
| `clamped`      | The value struck through by a min or max clamp, or `null`.                                       |
| `explosionOf`  | For a die added by an explosion, the index in `dice` of the die whose chain it continues.        |
| `restingPlace` | Pixels from the throw's anchor, rounded to whole pixels. The same on every board (SC-005).       |
| `chip`         | `"+N more"` when more than 20 dice were rolled.                                                  |
| `skipped`      | `true` for a throw the queue dropped. It has `"dice": []`, `"readout": null` and `"chip": null`. |

An entry is written when the throw's last die lands. A skipped throw's
entry is written when it is dropped. The readout is shown at the moment it
is written.

## The probe (apps/web/src/engine/bevy/index.ts, `installEngineProbe`)

| Member           | Shape                                                                                               |
| ---------------- | --------------------------------------------------------------------------------------------------- |
| `dicePlayed()`   | `number[][]`, unchanged (FR-018): each roll's `finalValue` list as the web sent it.                 |
| `diceLanded()`   | the parsed `dice_landed()`. `[]` when the export is absent.                                         |
| `diceEntities()` | the number of live throw entities, from `dice_entity_count()`. It is `0` after every fade (SC-004). |

## Export `dice_entity_count() -> u32` (new)

This is the number of entities that carry the `DiceThrowEntity` marker,
mirrored each frame.
