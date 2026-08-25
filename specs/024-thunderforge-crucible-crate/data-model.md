# Phase 1 Data Model: Thunderforge Crucible Crate

No persisted (database) entities — this crate is stateless (Technical
Context, plan.md). The "entities" here are in-memory Rust types shared
between `LocalAdjudicator` and `RemoteAdjudicator`/`crucible-server`'s wire
format.

## `AdjudicationRequest`

The proposed action to resolve.

| Field | Type | Notes |
|---|---|---|
| `world_id` | `Uuid` | Which world's session this belongs to — carried through even though this spec's placeholder ruleset doesn't yet use it per-world, so the shape doesn't need to change when the real ruleset does. |
| `actor_id` | `Uuid` | Which token/actor the action applies to. |
| `kind` | enum: `Move`, `Manipulate` | Matches spec.md's "movement and manipulation" framing exactly — not a broader action-type enum, to avoid speculative scope. |
| `payload` | `serde_json::Value` | The action's own detail (e.g. proposed x/y for `Move`). Deliberately untyped at this layer — this spec's placeholder ruleset does not need to interpret it; a typed payload-per-`kind` is a natural evolution once the real ruleset (ADR-047, future work) needs to actually inspect it. |

## `AdjudicationResult`

The authoritative outcome.

| Field | Type | Notes |
|---|---|---|
| `outcome` | enum: `Accepted`, `Rejected`, `Adjusted` | `Adjusted` carries a corrected `payload` (e.g. "you can move this far, not that far") — included in the enum shape now even though this spec's placeholder ruleset only ever produces `Accepted`, so the wire contract doesn't need a breaking change when the real ruleset starts producing the other two. |
| `payload` | `Option<serde_json::Value>` | Present only for `Adjusted`. |
| `reason` | `Option<String>` | Present only for `Rejected`/`Adjusted` — human-readable, not meant to be parsed by callers. |

## `SessionAdjudicatorError`

| Variant | Meaning |
|---|---|
| `RemoteUnavailable` | The configured remote adjudicator could not be reached within the bounded timeout (research.md §3) — `RemoteAdjudicator` only; `LocalAdjudicator` never produces this variant. |
| `InvalidRequest` | The request itself was malformed (e.g. unknown `kind`) — both implementations can produce this. |

## Configuration (not persisted, read once at startup)

| Env var | Values | Default |
|---|---|---|
| `CRUCIBLE_MODE` | `local` \| `remote` | `local` |
| `CRUCIBLE_ENDPOINT` | a URL | required only when `CRUCIBLE_MODE=remote`; absent/invalid in that mode fails startup (FR-005) |
