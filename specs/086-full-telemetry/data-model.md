# Data Model: Full Telemetry

One row is persisted: the instance id. Everything else is either process
configuration resolved once at start, or browser session state that dies with
the tab.

## Persisted

### `instance_settings` row `system.telemetry_instance_id`

This uses the existing table (migration
`2026-09-07-120000-0000_instance_settings`). There is no new migration.

| Column | Value |
| --- | --- |
| `key` | `system.telemetry_instance_id` |
| `value` | a UUIDv4, lower-case hyphenated (`Uuid::new_v4().to_string()`) |
| `created_by` and `updated_by` | `NULL`, because the instance wrote it |
| `created_at` and `updated_at` | the default |

**Rules (FR-007)**:

- It is written once:
  `INSERT ... ON CONFLICT (key) DO NOTHING`, then `SELECT value`.
- Two replicas starting together cannot mint two ids, because the second
  insert does nothing and both read the winner.
- It is never updated. Nothing else writes the key.
- The key is under `settings::registry::RESERVED_PREFIX` (`system.`), the
  prefix that exists for "keys the instance keeps for its own bookkeeping"
  (R23). A row there:
  - does not resolve;
  - is not editable through `updateInstanceSetting`;
  - is not reported as unrecognised by readiness.

  So FR-007's "nothing else writes that key" holds with no new code in
  `settings/`.
- A value that does not parse as a UUID is replaced only by deleting the row.
  The server logs a warning and runs with the stored text as it is. The
  server never writes it.
- It is created even with `TELEMETRY=false`.

**Lifecycle**: absent, then created on the first start after this spec, then
unchanged. An operator who deletes the row gets a new id on the next start.

## Process configuration (resolved once, in `apps/thunderforge`)

### `TelemetrySettings` (`apps/thunderforge/src/telemetry/settings.rs`)

| Field | From | Rule |
| --- | --- | --- |
| `enabled: bool` | `TELEMETRY` | false, 0, no or off in any case means `false`; anything else, or unset, means `true` |
| `sdk_disabled: bool` | `OTEL_SDK_DISABLED` | `true` (any case) means `true` |
| `server_endpoint: String` | `OTEL_EXPORTER_OTLP_ENDPOINT`, or the per-signal `OTEL_EXPORTER_OTLP_{TRACES,METRICS,LOGS}_ENDPOINT` | default `PROJECT_TELEMETRY_ENDPOINT`. The tier is decided on the general endpoint when set, otherwise on the traces endpoint, otherwise the default. Any per-signal endpoint that differs from the project's makes the tier operator for all signals, so a mixed set never sends operator-tier data to the project |
| `server_tier: Tier` | `tier_for(server_endpoint)` | |
| `browser: BrowserTelemetry` | `THUNDERFORGE_BROWSER_TELEMETRY_*` | below |
| `exporting: bool` | `enabled && !sdk_disabled` | the providers are installed only when this is true |

### `Tier`

`Tier::Anonymous` or `Tier::Operator`. It is decided only by `tier_for`
(FR-006). It is shown as `anonymous` or `full` in Appendix A's text, and
`anonymous` or `operator` in the served config and in resource attributes.

### `BrowserTelemetry` (`crates/thunderforge-server/src/telemetry/served_config.rs`)

`BrowserTelemetry` is built by the app and handed to `AppState`, so that the
server crate never decides a tier (R3).

| Field | From | Default |
| --- | --- | --- |
| `enabled` | `TELEMETRY` | `true` |
| `endpoint` | `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` | `PROJECT_TELEMETRY_ENDPOINT` |
| `sample_rate` | `THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE`, clamped to 0..1 | `1.0` |
| `environment` | `THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT` | `self-hosted` |
| `tier` | `tier_for(endpoint)` | |
| `instance_id` | the stored id | |

It provides:

- `served_json() -> serde_json::Value`, the contract's body;
- `connect_src() -> String`, the `connect-src` header.

Both are pure, so they are tested in a table.

### `TelemetryStatus` (`crates/thunderforge-server/src/telemetry/status.rs`)

`TelemetryStatus` is what the admin query and the startup line read. It holds
`enabled`, `server_exporting`, `server_tier`, `server_endpoint`,
`browser: BrowserTelemetry` and `instance_id`.

It lives in `AppState`. It is immutable after start, because a change needs a
restart, by design: there is no stored toggle (ADR-114).

## Instruments

The instruments are listed in
[contracts/server-instruments.md](contracts/server-instruments.md). Their
state is the existing atomics, plus one new atomic:

| New state | Where | Why |
| --- | --- | --- |
| `static WORLD_CHANNELS_REAPED: AtomicU64` | `network/listener.rs` | the reaper keeps no cumulative count today (R19) |
| `static DELIVERY: OnceLock<Arc<DeliveryMetrics>>` | `telemetry/instruments.rs` | lets the observable callbacks read the listener's `Arc` without a new parameter on every path |

## Browser state (`packages/telemetry`)

### Session

This is in `sessionStorage` under `thunderforge.telemetry`. See
[contracts/browser-events.md](contracts/browser-events.md).

| Field | Type | Rule |
| --- | --- | --- |
| `id` | 32 hex | 128 bits from `crypto.getRandomValues`. It is sent as `session.id` |
| `start` | epoch ms | the session start. It is not sent |
| `steps` | `FunnelStep[]` | the steps already sent. `funnel(step)` is a no-op for a step already in the list |
| `sampled` | bool | `random() < sampleRate`, decided once |
| `errors` | `{ [messageHash]: count }` | in memory only, for folding. It is not persisted |

**State transitions**:

- A new tab with an empty `sessionStorage` gets a new session.
- A reload or the viewer switch keeps the same session.
- A tab opened from another copies the session (spec 081), which the funnel
  accepts.
- Closing the tab ends the session.

### Queue

- It is bounded at 200 records, and the oldest is dropped first.
- Counters `dropped`, `internal_errors`, `sent_events` and `sent_errors` are
  checked against the per-session caps of 2,000 events and 50 errors.
- A record is `{ kind: "log" | "span", name, time, attrs }`. Attributes are
  already filtered by `ALLOWED_ATTRIBUTES` and redacted before they enter the
  queue.

### Funnel step

There are seven values, in order: `landing_viewed`, `demo_opened`,
`map_loaded`, `token_moved`, `dice_rolled`, `view_switched`, `cta_clicked`.
Each is sent at most once per session.

## Validation summary

| Rule | Enforced by | Tested by |
| --- | --- | --- |
| Only allow-listed span and resource attributes on the anonymous tier | `AllowListSpanProcessor`, the hand-built resource | the SC-011 leak test, and the `tier.rs` list tests |
| Metric names pass the public filter | `INSTRUMENTS` const | the `tier.rs` test |
| Every event code has a name | `event_names.rs` | the `include_str!` scan test |
| Only allow-listed browser attributes | `allowList.ts` in the core | `allowList.test.ts`, and the demo e2e canary test |
| Free text only in `error.message` and `error.stack`, redacted and cut | the core's `error()` | `errors.test.ts` |
| The instance id is never overwritten | `ON CONFLICT DO NOTHING`, and no other writer | the `instance_id` tests |
