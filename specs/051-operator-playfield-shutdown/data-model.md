# Data Model: Pausing a World's Play

**Spec**: [spec.md](spec.md) · **Research**: [research.md](research.md)

One migration, `src/server/migrations/<timestamp>_pausing_a_worlds_play/`
(take the timestamp at implementation, after the latest directory present;
`2026-09-13-160000-0000_*` was the latest on 2026-09-13). Paired `up.sql` and
`down.sql`.

None of the record tables has a foreign key to `worlds` or `users` (research R8).
`world_live_play` is the exception: it is a fact about a world that exists now,
and goes with the world.

## world_play_pauses

A world's live play stopped by an operator. While a row has `lifted_at IS NULL`,
the world is paused.

| Column | Type | Rules |
|---|---|---|
| `id` | uuid PK | |
| `world_id` | uuid NOT NULL | no FK |
| `world_name` | text NOT NULL | snapshot at pause |
| `paused_by` | uuid NOT NULL | operator; no FK |
| `paused_by_name` | text NOT NULL | snapshot |
| `paused_at` | timestamp NOT NULL DEFAULT now() | |
| `grounds` | text NOT NULL | `CHECK (length(btrim(grounds)) > 0)` (FR-004) |
| `request_id` | uuid NULL | the approved request, if any; `REFERENCES world_play_pause_requests(id)` |
| `lifted_by` | uuid NULL | no FK |
| `lifted_by_name` | text NULL | snapshot |
| `lifted_at` | timestamp NULL | |
| `lift_grounds` | text NULL | |
| `created_by` / `updated_by` | uuid NOT NULL | Principle III provenance |
| `updated_at` | timestamp NOT NULL | |

- **One active pause per world**: `UNIQUE (world_id) WHERE lifted_at IS NULL`.
  This index is also what the gate and the stream poll read.
- **Lifting is all-or-nothing**: `CHECK ((lifted_at IS NULL) = (lifted_by IS NULL)
  AND (lifted_at IS NULL) = (lifted_by_name IS NULL))`.
- **Append-only except the lift**: a trigger refuses any UPDATE other than
  setting the lift columns once, and refuses DELETE.

**State**: `active` → `lifted`. There is no way back from `lifted`; pausing again
is a new row.

## world_play_pause_requests

A proposal to pause a world, waiting for an operator.

| Column | Type | Rules |
|---|---|---|
| `id` | uuid PK | |
| `world_id` | uuid NOT NULL | no FK |
| `world_name` | text NOT NULL | snapshot at raise |
| `raised_at` | timestamp NOT NULL DEFAULT now() | |
| `state` | `"PauseRequestState"` NOT NULL DEFAULT 'Pending' | `Pending`, `Approved`, `Declined` |
| `decided_by` | uuid NULL | no FK |
| `decided_by_name` | text NULL | snapshot |
| `decided_at` | timestamp NULL | |
| `decision_note` | text NULL | required when `Declined` or `Approved` (becomes the pause's grounds when approved) |
| `created_by` / `updated_by` | uuid NULL | NULL when raised by the system (a takedown filed by an anonymous claimant) |
| `updated_at` | timestamp NOT NULL | |

- **One pending request per world**: `UNIQUE (world_id) WHERE state = 'Pending'`
  (FR-033).
- **Decided means fully decided**: `CHECK ((state = 'Pending') = (decided_at IS
  NULL))`, the same for `decided_by` and `decision_note`.
- **A decision is final**: a trigger refuses any UPDATE of a row whose old
  `state` is not `Pending`, and refuses DELETE.

**State**: `Pending` → `Approved` (creates a `world_play_pauses` row with
`request_id` set, in the same transaction) or `Pending` → `Declined`. The
transition is the conditional update in research R4; losing it returns the
winner.

## world_play_pause_triggers

What led to a request or a pause. Each trigger belongs to exactly one of them.

| Column | Type | Rules |
|---|---|---|
| `id` | uuid PK | |
| `request_id` | uuid NULL | `REFERENCES world_play_pause_requests(id)` |
| `pause_id` | uuid NULL | `REFERENCES world_play_pauses(id)` |
| `kind` | `"PauseTriggerKind"` NOT NULL | `Takedown`, `Operator`, `AbuseReport` (FR-037; nothing raises `AbuseReport` yet) |
| `moderation_action_id` | uuid NULL | the `content_moderation_actions` row, for `Takedown`; no FK, as that table has none either |
| `entity_type` | text NULL | the taken-down content's type |
| `entity_id` | uuid NULL | |
| `note` | text NULL | an operator's grounds, when a second operator pause lands on an active one (FR-036, FR-051) |
| `recorded_at` | timestamp NOT NULL DEFAULT now() | |
| `created_by` | uuid NULL | |

- **Exactly one owner**: `CHECK (num_nonnulls(request_id, pause_id) = 1)`.
- **A takedown names its action**: `CHECK (kind <> 'Takedown' OR
  moderation_action_id IS NOT NULL)`.
- **Idempotent per action and world**: `UNIQUE (moderation_action_id, request_id)`
  and `UNIQUE (moderation_action_id, pause_id)`, so a retried hook does not
  double-record.

A trigger on an approved request stays on the request; the pause reaches it
through `request_id`. A trigger raised while the world is already paused is
attached to the pause directly (FR-036).

## world_live_play

Whether a world has been played in the last minute, durable across processes
(research R3).

| Column | Type | Rules |
|---|---|---|
| `world_id` | uuid PK | `REFERENCES worlds(id) ON DELETE CASCADE` |
| `last_beat_at` | timestamp NOT NULL | |

Written by `heartbeat` with `INSERT … ON CONFLICT (world_id) DO UPDATE SET
last_beat_at = excluded.last_beat_at WHERE world_live_play.last_beat_at <
excluded.last_beat_at - interval '30 seconds'`, and skipped entirely in-process
when this process marked the world under 30 seconds ago. *In live play* is
`last_beat_at > now() - interval '45 seconds'`.

## Existing types that change

| Where | Change |
|---|---|
| `src/server/src/world_events.rs` | `EVENT_CODE_WORLD_PLAY_PAUSED = 28`. Payload carries `pausedAt` only. No event for a lift: no stream is open to receive it (research R6). |
| `crates/thunderforge-cache-core/src/queue.rs` `RejectionReason` | `PlayPaused` |
| `src/server/src/graphql/mutations_reconcile.rs` reason enum | `PlayPaused` |
| `apps/web` generated types (ts-rs / codegen) | follow the two enums above |

## What is read by whom

| Reader | Reads | Never reads |
|---|---|---|
| Any member of the world (`worldPlayState`) | `world_play_pauses.paused_at`, `lifted_at` for that world | grounds, names, triggers, requests |
| The gate and the stream poll | existence of an active `world_play_pauses` row | everything else |
| Operators (`admin_user`) | all three record tables, and `world_live_play` for "played now" | — |
