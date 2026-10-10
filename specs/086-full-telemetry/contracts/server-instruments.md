# Contract: Server instruments, spans and the anonymous allow-lists

This contract is the source of truth for two things:

- the constants in `crates/thunderforge-telemetry-policy`, which FR-009
  requires, and which `apps/thunderforge/src/telemetry/tier.rs` re-exports;
- the list that `cargo test -p thunderforge-telemetry-policy -- print_instruments --nocapture`
  prints for SC-008's checker.

The telemetry gateway reads the same constants
([telemetry-gateway.md](telemetry-gateway.md)), so a name added here is
also a name the public route accepts.

A name changes here first.

## Conversion rule (R4)

- The OTel name's dots become underscores.
- A monotonic counter gains `_total`.
- Unit `s` becomes `_seconds`.
- A `{...}` unit adds nothing.
- Resource attributes go to `target_info{job="thunderforge"}`, and never to a
  series.

## Instruments

All instruments come from `opentelemetry::global::meter("thunderforge")`.

| OTel name | Kind | Unit | Attributes | Prometheus series | Reads |
| --- | --- | --- | --- | --- | --- |
| `thunderforge.backplane.sent` | observable counter | `{event}` | none | `thunderforge_backplane_sent_total` | `DeliveryMetrics.sent` |
| `thunderforge.backplane.dropped` | observable counter | `{event}` | none | `thunderforge_backplane_dropped_total` | `.dropped` |
| `thunderforge.backplane.polls` | observable counter | `{poll}` | none | `thunderforge_backplane_polls_total` | `.polls` |
| `thunderforge.backplane.errors` | observable counter | `{error}` | none | `thunderforge_backplane_errors_total` | `.errors` |
| `thunderforge.backplane.panics` | observable counter | `{panic}` | none | `thunderforge_backplane_panics_total` | `.panics` |
| `thunderforge.backplane.timeouts` | observable counter | `{timeout}` | none | `thunderforge_backplane_timeouts_total` | `.timeouts` |
| `thunderforge.backplane.cursor` | observable gauge | `{event}` | none | `thunderforge_backplane_cursor` | `.cursor` |
| `thunderforge.subscriptions.opened` | observable counter | `{subscription}` | none | `thunderforge_subscriptions_opened_total` | `subscription_metrics::OPENED` |
| `thunderforge.subscriptions.refused` | observable counter | `{subscription}` | none | `thunderforge_subscriptions_refused_total` | `REFUSED` |
| `thunderforge.subscriptions.delivered` | observable counter | `{event}` | none | `thunderforge_subscriptions_delivered_total` | `DELIVERED` |
| `thunderforge.subscriptions.lagged` | observable counter | `{event}` | none | `thunderforge_subscriptions_lagged_total` | `LAGGED_EVENTS` |
| `thunderforge.websocket.sockets_open` | observable gauge | `{socket}` | none | `thunderforge_websocket_sockets_open` | `SOCKETS_OPEN` |
| `thunderforge.world_channels.reaped` | observable counter | `{channel}` | none | `thunderforge_world_channels_reaped_total` | new `WORLD_CHANNELS_REAPED` |
| `thunderforge.graphql.operation.duration` | histogram | `s` | `operation_type`, `root_field`, `outcome` | `thunderforge_graphql_operation_duration_seconds_{bucket,sum,count}` | extension |
| `thunderforge.graphql.errors` | counter | `{error}` | `root_field`, `code` | `thunderforge_graphql_errors_total` | extension |
| `thunderforge.http.server.duration` | histogram | `s` | `route`, `method`, `status_class` | `thunderforge_http_server_duration_seconds_{bucket,sum,count}` | `TraceLayer` `on_response` |
| `thunderforge.db.pool.connections` | observable gauge | `{connection}` | `state` = `idle` or `in_use` | `thunderforge_db_pool_connections` | `pool.state()` |
| `thunderforge.db.pool.max_connections` | observable gauge | `{connection}` | none | `thunderforge_db_pool_max_connections` | `pool.max_size()` |
| `thunderforge.db.pool.checkout_wait` | histogram | `s` | none | `thunderforge_db_pool_checkout_wait_seconds_{bucket,sum,count}` | `HandleEvent::handle_checkout` |
| `thunderforge.db.pool.checkout_timeouts` | counter | `{timeout}` | none | `thunderforge_db_pool_checkout_timeouts_total` | `HandleEvent::handle_timeout` |
| `thunderforge.world_events` | counter | `{event}` | `event` | `thunderforge_world_events_total` | `record_world_event` (ok) |
| `thunderforge.world_event.record_failures` | counter | `{event}` | `event` | `thunderforge_world_event_record_failures_total` | `record_world_event` (err) |
| `thunderforge.rolls` | counter | `{roll}` | `event`, `visibility` | `thunderforge_rolls_total` | `record_world_event` for codes 36 and 37 |
| `thunderforge.sheet_imports` | counter | `{import}` | `system`, `reader`, `outcome` (spec 048 values) | `thunderforge_sheet_imports_total` | `sheet_import::telemetry::record_import`, once per `applySheetImport` (spec 048) |
| `thunderforge.sheet_import.read_duration` | histogram | `s` | `reader` | `thunderforge_sheet_import_read_duration_seconds_{bucket,sum,count}` | `record_read_duration`, around the server's own reading (spec 048) |
| `thunderforge.sheet_import.fields` | counter | `{field}` | `certainty` | `thunderforge_sheet_import_fields_total` | `record_fields`, per applied plan (spec 048) |
| `thunderforge.staged_content.decisions` | counter | `{decision}` | `decision` | `thunderforge_staged_content_decisions_total` | `record_decision`, after the decision commits (spec 048) |
| `thunderforge.unadopted_use_attempts` | counter | `{attempt}` | `result` | `thunderforge_unadopted_use_attempts_total` | `staged_content::report::record_attempt` (spec 048) |

Histogram buckets:

- seconds instruments use `0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10`;
- the pool wait uses `0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1, 5, 30`.

These are set with SDK views in `install.rs`.

Spec 048's five rows are in the policy's `INSTRUMENTS`, and record through
`Recorders` from `crates/thunderforge-server/src/sheet_import/telemetry.rs`.
Their labels (`system`, `reader`, `certainty`, `decision`, `result`) are in
`SERVER_METRIC_ATTRIBUTES`, so the anonymous tier keeps them; every value is
from a closed set or a pack-declared id, never a name, an id or content.

### Bounded label values

| Label | Values |
| --- | --- |
| `operation_type` | `query`, `mutation`, `subscription` |
| `root_field` | a field name of the schema's Query, Mutation or Subscription root, or `unknown` |
| `outcome` | `ok`, `error`, `refused` |
| `code` | an `extensions.code` the server emits, or `internal` when there is none |
| `route` | an axum `MatchedPath` template, or `unmatched` |
| `method` | `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS`, `HEAD`, `other` |
| `status_class` | `1xx` .. `5xx` |
| `state` | `idle`, `in_use` |
| `event` | a name from `telemetry/event_names.rs`, covering 33 codes (spec 088 added `rolls_cleared`, 39; spec 048 added `sheet_import_applied`, `staged_content_decided` and `actor_rolled_back`, 40 to 42) |
| `visibility` | `everyone`, `gm_eyes`, `gm_only`: `rolls::visibility::Visibility::as_str()`, a closed enum |
| `system` | a pack's game-system id with a `sheetImport` block, or `none` before the actor's system is known (spec 048) |
| `reader` | a pack-declared reader id (`ddb-pdf`, ...), or `none` before the sheet was read (spec 048) |
| `outcome` on `thunderforge.sheet_imports` | `applied`, `refused_flag`, `refused_permission`, `refused_bounds`, `refused_unrecognised`, `refused_unmapped_system`, `refused_plan_changed`, `failed` |
| `certainty` | `read`, `uncertain`, `unread`, `corrected` |
| `decision` | `adopt`, `adopt_all`, `decline`, `revisit` |
| `result` | `reported`, `suppressed_stale`, `rate_limited` |

`code` is bounded by the server's own error codes. Nothing outside the server
can set it, because errors are produced by our resolvers. An `internal`
fallback catches the rest.

### The event-code name table

`telemetry/event_names.rs` has `pub fn event_name(code: i32) -> &'static str`.
It covers all 29 `EVENT_CODE_*` constants in `world_events.rs`, by the
constant's own name lower-cased with the prefix removed. For example,
`EVENT_CODE_ROLL_MADE` (36) is `roll_made`. An unknown code is `unknown`.

A test reads `world_events.rs` with `include_str!`, collects every
`pub const EVENT_CODE_` line, and fails when one has no name (FR-014).

## Spans

| Span | Name | Attributes, operator tier | Kept on the anonymous tier |
| --- | --- | --- | --- |
| HTTP request (`TraceLayer`) | `HTTP {method} {route}` | `http.route`, `http.request.method`, `http.response.status_code`, `url.path`, `user_agent.original`, `trace_id` and `span_id` fields | `http.route`, `http.request.method`, `http.response.status_code` |
| GraphQL operation | `graphql.{type} {root_field}` | `graphql.operation.type`, `graphql.root_field`, `graphql.operation.name`, `graphql.error.codes`, `root_fields`, `outcome`, `world.id` (when a resolver records it) | everything but `graphql.operation.name` and `world.id` |
| `record_world_event` | `world_event.record` | `event`, `visibility`, `world.id` | `event`, `visibility` |
| Pool checkout (operator only) | none: a span event on the current span | `state` | dropped |

A subscription gets one span, for its setup only.

## Policy constants (FR-009)

In `crates/thunderforge-telemetry-policy/src/lib.rs` (and its modules).
`tier.rs` re-exports them and adds only the SDK glue.

```rust
pub const PROJECT_TELEMETRY_ENDPOINT: &str = "https://telemetry.thunderforge.dev";

pub const ANONYMOUS_SPAN_ATTRIBUTES: &[&str] = &[
    "graphql.operation.type", "graphql.root_field", "graphql.error.codes",
    "root_fields", "outcome",
    "http.route", "http.request.method", "http.response.status_code",
    "event", "visibility", "state",
];

pub const ANONYMOUS_RESOURCE_ATTRIBUTES: &[&str] = &[
    "service.name", "service.version", "thunderforge.instance.id",
    "thunderforge.tier", "os.type", "host.arch", "deployment.environment",
];

pub const INSTRUMENTS: &[(&str, InstrumentKind, &str)] = &[ /* the table above */ ];

pub const PUBLIC_METRIC_NAME_FILTER: &str = r"^(thunderforge\.|http\.server\.|db\.client\.)";
```

`PUBLIC_METRIC_NAME_FILTER` mirrors the collector's `filter/public`. The
gateway is stricter: it accepts exactly the `INSTRUMENTS` names (R27).

On the anonymous tier, the processor that enforces these
(`anonymous::AllowListSpanProcessor`) wraps the batch processor. On `on_end`
it:

- drops every attribute not in the list;
- drops every link;
- keeps only span events named `exception`, with their `exception.message`
  and `exception.stacktrace` redacted and truncated (512 and 4096
  characters);
- forwards the result.

Tests:

- `INSTRUMENTS` matches `PUBLIC_METRIC_NAME_FILTER`;
- `ANONYMOUS_RESOURCE_ATTRIBUTES` excludes `service.instance.id`, `host.name`,
  `process.*`, `container.*` and `k8s.*`;
- each list is enumerated, so an addition is visible in review.

## The `server.error` record (anonymous tier)

The `anonymous::ServerErrorLayer`, a `tracing` layer, emits one OTLP log
record per `ERROR` event. No other log record leaves on that tier.

| Field | Value |
| --- | --- |
| body | the redacted message, cut to 512 characters |
| `event.name` | `server.error` |
| `error.type` | the event's `error.type` field if it has one, else its target (a module path) |
| `error.message` | the same as the body |
| `error.stack` | `std::backtrace::Backtrace::capture()` reduced to `crate::path:line` frames, cut to 4096 characters (empty unless `RUST_BACKTRACE` is set) |
| severity | `ERROR` |

Redaction uses `crates/thunderforge-server/src/feedback/redaction.rs`, which
reads `config/feedback-redaction.json`. No other field of the event is read.

## Resource

| Attribute | Anonymous | Operator |
| --- | --- | --- |
| `service.name` | `thunderforge` | `OTEL_SERVICE_NAME`, or `thunderforge` |
| `service.version` | `CARGO_PKG_VERSION` | the same |
| `thunderforge.instance.id` | the instance id | the same |
| `thunderforge.tier` | `anonymous` | `operator` |
| `os.type`, `host.arch` | from `std::env::consts` | the detectors' values |
| `deployment.environment` | `self-hosted` | `OTEL_RESOURCE_ATTRIBUTES`, or unset |
| everything else | none | the SDK's default detectors plus `OTEL_RESOURCE_ATTRIBUTES` |
| `service.instance.id` | never set | never set by us (R4) |
