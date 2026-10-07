# Data Model: Rolls at the Table

## `world_roll_records` (existing, gains four columns)

| Column        | Type        | Null | Default      | Notes                                     |
| ------------- | ----------- | ---- | ------------ | ----------------------------------------- |
| `visibility`  | text        | no   | `'everyone'` | check: `everyone`, `gm_eyes`, `gm_only`   |
| `label`       | text        | yes  |              | at most 80 characters                     |
| `revealed_at` | timestamptz | yes  |              | set once, by `revealRoll`                 |
| `revealed_by` | uuid        | yes  |              | references `users(id)` on delete set null |

- A check constraint: `revealed_by` is null whenever `revealed_at` is null.
  `revealed_by` may be null on a revealed roll, once the revealer's account
  is deleted.
- Index: `(world_id, created_at DESC)` for the feed, if not already present.
- Existing rows become `everyone`. Down migration drops the four columns.

## Visibility rules

| Roll          | Roller | GM / admin | Other player |
| ------------- | ------ | ---------- | ------------ |
| `everyone`    | whole  | whole      | whole        |
| `gm_eyes`     | whole  | whole      | masked       |
| `gm_only`     | whole  | whole      | hidden       |
| any, revealed | whole  | whole      | whole        |

| May roll   | Player | GM  |
| ---------- | ------ | --- |
| `everyone` | yes    | yes |
| `gm_eyes`  | yes    | no  |
| `gm_only`  | no     | yes |

An admin who is not the world's GM rolls as a player and reveals as a GM.

## World events

| Code | Name                       | Payload                  | Delivered to                                             |
| ---- | -------------------------- | ------------------------ | -------------------------------------------------------- |
| 36   | `EVENT_CODE_ROLL_MADE`     | `{ rollId, visibility }` | every member, except `gm_only` → GM and admins only (R1) |
| 37   | `EVENT_CODE_ROLL_REVEALED` | `{ rollId, visibility }` | every member; `visibility` is the roll's original one    |

## The demo's saved world

`DemoState.rolls[]` entries gain optional `visibility`, `label`,
`revealedAt` and `revealedBy`. An entry without `visibility` is
`everyone`, so the saved version stays 4. The viewer leaves the saved
world for `sessionStorage` (research R7); a saved world that still has one
is read once as the first tab's viewer.
