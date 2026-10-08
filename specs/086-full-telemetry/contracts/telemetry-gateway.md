# Contract: The telemetry gateway and its policy crate

This contract is the source of truth for `apps/telemetry-gateway`
(FR-037 to FR-046) and for `crates/thunderforge-telemetry-policy`, which the
gateway and the server's anonymous tier share (FR-038). A name, a status, a
label or a default changes here first.

## Where it sits

```
browser / self-hosted server
  → https://telemetry.thunderforge.dev   (shared-gateway, HTTPRoute in monitoring)
  → telemetry-gateway:4318               (this service)
  → otel-collector-public:4319           (otlp/public: filter/public, transform/public)
  → Loki, Tempo, Prometheus

telemetry-gateway's own telemetry
  → http://otel-collector.monitoring:4318 (in-cluster receiver, never the public one)
```

## HTTP surface

| Method and path | Content types | Answer |
| --- | --- | --- |
| `POST /v1/traces`, `/v1/logs`, `/v1/metrics` | `application/x-protobuf`, `application/json` | see below |
| `OPTIONS` on the same paths | any | `204`, `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Methods: POST`, `Access-Control-Allow-Headers: Content-Type`, `Access-Control-Max-Age: 7200`, no `Access-Control-Allow-Credentials` |
| `GET /healthz` | none | `200` when the process is up; never touches the upstream |
| anything else | any | `404` |

Every `POST` answer also carries `Access-Control-Allow-Origin: *`. The
answer's body is the OTLP export response in the request's own content
type.

### Statuses, in the order they are checked

| Check | Status | Forwarded | `reason` |
| --- | --- | --- | --- |
| Tower concurrency limit full (`load_shed`) | `503` | nothing | `overloaded` |
| Per-IP bucket empty | `429`, `Retry-After: <s>` | nothing | `rate_limited_ip` |
| Body over 4 MiB (`RequestBodyLimitLayer`) | `413` | nothing | `body_too_large` |
| Unknown content type, or the body does not decode | `400` | nothing | `undecodable` |
| Content checks (next table) | `200` with `partial_success` | what passed | per resource or record |
| Every resource over its instance's bucket | `429`, `Retry-After: <s>` | nothing | `rate_limited_instance` |
| No upstream slot free (`try_acquire`) | `503` | nothing | `overloaded` |
| Upstream error, non-2xx, or timeout | `503` | attempted | `upstream_error` |
| Otherwise | `200`, with `partial_success` when anything was dropped | what passed | none |

`200` with `partial_success` is final for OTLP senders: they do not retry it.
`429` and `503` are retried with backoff by the OTLP exporters, and dropped
by the browser's bounded queue.

### Content checks

Applied per `ResourceSpans`, `ResourceLogs` or `ResourceMetrics`, then per
record:

| Grain | Dropped when | `reason` |
| --- | --- | --- |
| resource | `service.name` is not `thunderforge`, `thunderforge-landing`, `thunderforge-demo` or `thunderforge-web` | `unknown_service` |
| resource | `thunderforge.instance.id` is present and not a hyphenated UUID, or absent (except `thunderforge-landing` and `thunderforge-demo` from `owner_site`) | `instance_id` |
| resource | its instance's bucket is empty | `rate_limited_instance` |
| metric | its name is not in `INSTRUMENTS` | `metric_name` |
| span, log record, data point | an attribute value, or a log body, is longer than its cap | `attribute_too_large` |
| attribute | its key is not on the allow-list for its place | not dropped: removed, and counted in `attributes_stripped` |

A dropped resource counts once per request in `dropped`, whatever it held.
`partial_success.rejected_*` counts the records it held.

## Labels added (FR-041)

Set on every forwarded resource, overwriting a sender's attribute of the
same name:

| Attribute | Value | When |
| --- | --- | --- |
| `thunderforge.ingress` | `public` | always |
| `thunderforge.source` | `owner_site`, `self_hosted_browser`, `server` | always (R30) |
| `thunderforge.origin.host` | the `Origin` host, lower-cased, no scheme or port; `opaque` for `Origin: null` | browsers only |
| `thunderforge.instance.id` | the sender's, unchanged after the check | when present |
| `thunderforge.client.version` | `service.version` when it matches `^\d+\.\d+\.\d+([-+][0-9A-Za-z.-]{1,64})?$`, else `unknown` | always |
| `thunderforge.user_agent.family` | `chrome`, `edge`, `firefox`, `safari`, `opera`, `samsung`, `otel-rust`, `other` | always |
| `thunderforge.user_agent.major` | the major version digits, or `unknown` | always |
| `thunderforge.country` | `CF-IPCountry`, when it matches `^[A-Z]{2}$` and is not `XX` or `T1` | only when the header is present (R29) |

These are resource attributes. The collector moves them to `target_info` for
metrics, and Loki keeps them as structured metadata for logs; none becomes a
Prometheus or Loki index label. The labels are indicative: `Origin`,
`User-Agent` and `CF-IPCountry` can be forged by a non-browser sender.

The forwarded request is built fresh: `POST <upstream>/v1/<signal>`,
`Content-Type: application/x-protobuf`, the re-encoded body, and no other
header. No IP address is in any header, attribute or body.

## The IP address (FR-042, R28)

- client IP = the entry `TELEMETRY_GATEWAY_TRUSTED_HOPS` from the right of
  `X-Forwarded-For`, or the TCP peer when there is no header;
- bucket key = the IP's bytes hashed with a `RandomState` made once at
  start (SipHash with a random per-process key); the address is dropped
  when the request ends, and the key dies with the process;
- the bucket map holds at most `TELEMETRY_GATEWAY_MAX_IP_KEYS` entries and
  evicts any idle longer than a full refill; a full map refuses new keys as
  `rate_limited_ip`;
- the `TraceLayer` records method, matched path, status and latency only.

## Configuration

`clap` flags, each with an environment fallback.

| Flag | Environment | Default |
| --- | --- | --- |
| `--listen` | `TELEMETRY_GATEWAY_LISTEN` | `0.0.0.0:4318` |
| `--upstream` | `TELEMETRY_GATEWAY_UPSTREAM` | `http://otel-collector-public.monitoring:4319` |
| `--trusted-hops` | `TELEMETRY_GATEWAY_TRUSTED_HOPS` | `1` |
| `--ip-rate` / `--ip-burst` | `TELEMETRY_GATEWAY_IP_RATE` / `_IP_BURST` | `5` per second / `60` |
| `--instance-rate` / `--instance-burst` | `TELEMETRY_GATEWAY_INSTANCE_RATE` / `_INSTANCE_BURST` | `20` per second / `200` |
| `--max-ip-keys` / `--max-instance-keys` | `TELEMETRY_GATEWAY_MAX_IP_KEYS` / `_MAX_INSTANCE_KEYS` | `100000` / `100000` |
| `--concurrency` | `TELEMETRY_GATEWAY_CONCURRENCY` | `256` |
| `--upstream-in-flight` | `TELEMETRY_GATEWAY_UPSTREAM_IN_FLIGHT` | `64` |
| `--upstream-timeout-ms` | `TELEMETRY_GATEWAY_UPSTREAM_TIMEOUT_MS` | `5000` (connect `2000`) |
| none | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector.monitoring:4318`, for the gateway's own telemetry |
| none | `TELEMETRY` | `true`; `false` turns the gateway's own export off, never its checks |

A self-hosted operator never runs the gateway. It is the project's.

## The policy crate

`crates/thunderforge-telemetry-policy`. Dependencies: none beyond `std` (`regex`
as a dev-dependency, for the `PUBLIC_METRIC_NAME_FILTER` test).
No IO, no clock reads and no randomness: the caller passes the time in, and
the gateway owns the hashing key.

```rust
// tier (moved from apps/thunderforge/src/telemetry/tier.rs)
pub const PROJECT_TELEMETRY_ENDPOINT: &str = "https://telemetry.thunderforge.dev";
pub enum Tier { Anonymous, Operator }
pub fn tier_for(endpoint: &str) -> Tier;

// lists
pub const SERVICE_NAMES: &[&str] = &["thunderforge", "thunderforge-landing", "thunderforge-demo", "thunderforge-web"];
pub const ANONYMOUS_SPAN_ATTRIBUTES: &[&str];        // contracts/server-instruments.md
pub const ANONYMOUS_RESOURCE_ATTRIBUTES: &[&str];    // server resource
pub const SERVER_ERROR_ATTRIBUTES: &[&str];          // server.error record
pub const BROWSER_RESOURCE_ATTRIBUTES: &[&str];      // contracts/browser-events.md
pub const BROWSER_RECORD_ATTRIBUTES: &[&str];        // = ALLOWED_ATTRIBUTES in packages/telemetry
pub const BROWSER_SPAN_ATTRIBUTES: &[&str];
pub const GATEWAY_LABELS: &[&str];                   // the table above
pub const INSTRUMENTS: &[(&str, InstrumentKind, &str)];
pub const GATEWAY_INSTRUMENTS: &[(&str, InstrumentKind, &str)];

pub enum Place { Resource, Span, SpanEvent, LogRecord, DataPoint }
pub fn attribute_allowed(service: &str, place: Place, key: &str) -> bool;
pub fn metric_allowed(name: &str) -> bool;

// caps, in characters
pub const CAP_DEFAULT: usize = 1024;
pub const CAP_MESSAGE: usize = 512;    // error.message, exception.message
pub const CAP_STACK: usize = 4096;     // error.stack, exception.stacktrace
pub const CAP_LOG_BODY: usize = 4096;
pub fn cap_for(key: &str) -> usize;

// checks and labels
pub fn is_instance_id(s: &str) -> bool;
pub fn instance_id_required(service: &str, source: Source) -> bool;
pub enum Source { OwnerSite, SelfHostedBrowser, Server }
pub fn source_for(origin: Option<&str>) -> (Source, Option<String>); // with the origin host
pub fn client_version(service_version: Option<&str>) -> &str;
pub fn reduce_user_agent(ua: Option<&str>) -> (&'static str, String);
pub fn country(cf_ipcountry: Option<&str>) -> Option<&str>;
pub enum DropReason { RateLimitedIp, RateLimitedInstance, BodyTooLarge, Undecodable,
                      UnknownService, MetricName, AttributeTooLarge, InstanceId,
                      Overloaded, UpstreamError }
impl DropReason { pub fn as_str(&self) -> &'static str; }

// rate
pub struct TokenBucket { /* tokens, last refill */ }
impl TokenBucket {
    pub fn new(rate_per_s: f64, burst: u32, now: std::time::Instant) -> Self;
    pub fn try_take(&mut self, now: std::time::Instant) -> Result<(), std::time::Duration>; // Err = retry after
}
pub fn print_instruments(); // prints INSTRUMENTS and GATEWAY_INSTRUMENTS as Prometheus names
```

Every honest sender's own caps are at or below these, so only a sender that
ignores its contract trips `attribute_too_large`.

## The gateway's own instruments

From `opentelemetry::global::meter("thunderforge-telemetry-gateway")`, with
`service.name=thunderforge-telemetry-gateway`.

| OTel name | Kind | Unit | Attributes | Prometheus series |
| --- | --- | --- | --- | --- |
| `thunderforge.telemetry_gateway.dropped` | counter | `{request}` | `reason` | `thunderforge_telemetry_gateway_dropped_total` |
| `thunderforge.telemetry_gateway.accepted` | counter | `{request}` | `signal` (`traces`, `logs`, `metrics`), `source` | `thunderforge_telemetry_gateway_accepted_total` |
| `thunderforge.telemetry_gateway.attributes_stripped` | counter | `{attribute}` | `signal` | `thunderforge_telemetry_gateway_attributes_stripped_total` |
| `thunderforge.telemetry_gateway.upstream.duration` | histogram | `s` | `signal`, `outcome` (`ok`, `error`) | `thunderforge_telemetry_gateway_upstream_duration_seconds_{bucket,sum,count}` |

`reason` is `DropReason::as_str()`, a closed set of ten. No attribute holds
an IP, an origin host, a country or an instance id.

## Dashboard and alerts

`deploy/k8s/observability/dashboards/server.json` gains a **Public telemetry
intake** row:

- accepted per second by `signal` and `source`;
- dropped per second by `reason`, stacked;
- the share dropped, `sum(rate(dropped)) / (sum(rate(dropped)) + sum(rate(accepted)))`;
- upstream p95 from the duration histogram.

`prometheus-rules.yaml` gains:

| Alert | Expression | For | Severity |
| --- | --- | --- | --- |
| `ThunderForgeTelemetryIntakeFailing` | `sum(rate(thunderforge_telemetry_gateway_dropped_total{reason=~"upstream_error\|overloaded"}[5m])) > 0.1` | 10m | warning |
| `ThunderForgeTelemetrySpam` | `sum(rate(thunderforge_telemetry_gateway_dropped_total{reason!~"upstream_error\|overloaded"}[5m])) > 5 * sum(rate(thunderforge_telemetry_gateway_accepted_total[5m])) + 1` | 15m | info |

## Tests

`cargo test -p thunderforge-telemetry-policy`, written first, deterministic:

- each list is enumerated, so an addition shows in review;
- `INSTRUMENTS` and `GATEWAY_INSTRUMENTS` names are unique and start with
  `thunderforge.`;
- `BROWSER_RECORD_ATTRIBUTES` equals the keys in
  `packages/telemetry/src/allowList.ts`, read with `include_str!`;
- `tier_for`'s normalisation table (moved from `tier.rs`);
- `is_instance_id`, `source_for` (`thunderforge.dev`, `vtt-dev`, a
  self-hosted host, `null`, a port, upper case, absent), `reduce_user_agent`
  (one fixture string per family), `country` (`DE`, `XX`, `T1`, `de`,
  absent), `client_version`, `cap_for`;
- `TokenBucket` under a fixed `Instant` advanced by hand: burst, refill,
  `Retry-After` rounding up.

`cargo test -p thunderforge-telemetry-gateway`, the router in process with
`tower::ServiceExt::oneshot`, a fake upstream (an axum router on
`127.0.0.1:0` that records each request), and an in-memory meter:

- the preflight for `https://game.example.org` and for `null`;
- protobuf and JSON fixtures for each signal decode and forward as protobuf;
  the JSON fixtures are captured from `packages/telemetry` (R26);
- every label of the table above, for each source;
- a request with `X-Forwarded-For: 203.0.113.77` and
  `User-Agent: ... Chrome/131...`: neither string appears in any byte the
  fake upstream received, nor in the captured `tracing` output or metric
  attributes;
- one test per `reason`, each asserting the status, what the fake received,
  and the counter at one;
- the `429` with `Retry-After`, under a fake clock;
- the fast `503`: the fake upstream holds every slot, and the next request
  answers within 50 ms;
- the upstream's own `partial_success` is passed back.

## Shipping

`Dockerfile`, before the `server` stage so `server` stays the default:

```dockerfile
FROM toolchain AS gateway-cook
WORKDIR /build
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p thunderforge-telemetry-gateway

FROM gateway-cook AS gateway-build
COPY . .
RUN cargo build --release -p thunderforge-telemetry-gateway \
 && install -D target/release/thunderforge-telemetry-gateway /out/thunderforge-telemetry-gateway \
 && strip /out/thunderforge-telemetry-gateway

FROM debian:bookworm-slim AS telemetry-gateway
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=gateway-build /out/thunderforge-telemetry-gateway /usr/local/bin/
USER 65532:65532
EXPOSE 4318
ENTRYPOINT ["/usr/local/bin/thunderforge-telemetry-gateway"]
```

`Makefile`:

```make
TELEMETRY_GATEWAY_IMAGE ?= mbround18/thunderforgevtt:telemetry-gateway

telemetry-gateway-image:
	docker build --target telemetry-gateway -t $(TELEMETRY_GATEWAY_IMAGE) .

push-telemetry-gateway: telemetry-gateway-image
	docker push $(TELEMETRY_GATEWAY_IMAGE)
```

The Deployment is Flux's, so the target only builds and pushes; the
Deployment pulls `Always` and the owner restarts it.

In the Flux repository, `overlays/services/otel-collector/`:

- `telemetry-gateway.yaml`: a Deployment (2 replicas, `readinessProbe` on
  `/healthz`, 64 Mi request and 256 Mi limit, `runAsNonRoot`, read-only root
  filesystem) and a Service `telemetry-gateway` on 4318, in `monitoring`;
- `route.yaml`: the backendRef becomes `telemetry-gateway:4318`, and the
  header comment says so;
- `helmrelease.yaml`: the `otlp/public` receiver's `cors` block is removed;
  `filter/public` and `transform/public` stay;
- `public-service.yaml` stays, for the gateway's hop.

Each replica keeps its own buckets, so the effective limit is the configured
one times the replica count. That is accepted; the defaults are set with it
in mind.
