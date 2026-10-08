# Research: Full Telemetry

Each item gives the decision, what was checked or why, and what was
rejected. Checked against the tree and the live cluster on 2026-10-07.

## R1. The Rust OpenTelemetry crates and their versions

**Decision**: These are the latest on crates.io, and they move together.

| Crate | Version | Where | Why |
| --- | --- | --- | --- |
| `opentelemetry` | 0.33.0 | `thunderforge-server`, `thunderforge` | The API. Its global meter and tracer do nothing until an SDK is installed (FR-005). |
| `opentelemetry_sdk` | 0.33.0 | `thunderforge` | The providers, the batch processors, and the span processor that enforces the allow-list. The `testing` feature (dev only) gives the in-memory exporters the tests need. |
| `opentelemetry-otlp` | 0.33.0 | `thunderforge` | The OTLP/HTTP exporter. `default-features = false`, with features `http-proto`, `reqwest-client`, `reqwest-rustls`, `trace`, `metrics` and `logs`. |
| `tracing-opentelemetry` | 0.34.0 | `thunderforge` | The layer beside Bunyan (FR-001). 0.34 is the release that pairs with otel 0.33. |
| `opentelemetry-appender-tracing` | 0.33.0 | `thunderforge` | The log bridge. Installed on the operator tier only (FR-006). |

**Verified**:

- `opentelemetry-otlp` 0.33 uses `opentelemetry-http`, which is built on
  reqwest 0.13. That is the workspace's reqwest (0.13.4, rustls), so no
  second HTTP client or TLS stack is compiled.
- The `http-proto` feature posts protobuf to `/v1/{traces,metrics,logs}`.
  Both the in-cluster collector (:4318) and the public one (:4319 behind
  `telemetry.thunderforge.dev`) accept it.
- The SDK's batch processors and its periodic metric reader run on their own
  thread with bounded queues by default (2048 spans, 512 per batch). That
  meets FR-003 without the `rt-tokio` feature. A collector that is down costs
  one failed POST per interval, and the queue drops what it cannot hold.

**Not added**:

- `opentelemetry-semantic-conventions`. About ten attribute names are needed,
  and they are written as constants in `tier.rs`, which is where FR-009 wants
  them listed anyway.
- `grpc-tonic`. gRPC would pull in tonic and a second HTTP stack, and the
  public route is HTTP only.
- `gzip-http`. Batches are small, the collector's body cap is 4 MiB, and
  gzip adds flate2. It can be added later.

## R2. `uuid` needs `v4`

**Decision**: Add the `v4` feature to `uuid` in
`crates/thunderforge-server/Cargo.toml`. This adds a feature, not a crate.

**Why**: The crate has `serde` and `v7` today. A v7 id embeds its creation
time, and FR-007 forbids an id derived from a time. `Uuid::new_v4()` reads
the OS random source through `getrandom`, which is already in the tree.

## R3. Where the code lives

**Decision**:

- `crates/thunderforge-server/src/telemetry/` holds only API-level code. That
  covers the instruments (`instruments.rs`), the event-code name table
  (`event_names.rs`), the GraphQL extension (`graphql_extension.rs`), the
  pool event handler (`pool_events.rs`), the instance id (`instance_id.rs`),
  the served browser config and the CSP header built from it
  (`served_config.rs`), and the `TelemetryStatus` that the admin query and
  the startup line read (`status.rs`).
- `apps/thunderforge/src/telemetry/` holds the SDK:
  - `tier.rs`: `PROJECT_TELEMETRY_ENDPOINT`, `tier_for`, and the three
    allow-list constants;
  - `settings.rs`: reads the environment into one `TelemetrySettings`;
  - `install.rs`: the providers and the layers;
  - `anonymous.rs`: the allow-list span processor and the `server.error`
    layer;
  - `startup_line.rs`: the Appendix A.3 text.

**Why**:

- FR-005 keeps the SDK out of the server crate.
- `main.rs` is 865 lines, and Rust files must stay at 1000 or fewer
  (`scripts/check-file-length.sh`). `main.rs` therefore gains only a few calls
  into `telemetry::`.
- The served config needs the tier, but the tier is decided in the app crate.
  So the app computes `TelemetrySettings` and passes the resolved
  `BrowserTelemetry` value (enabled, endpoint, sample rate, environment and
  tier) into `AppState`. The server crate serialises that value and never
  decides a tier itself. That keeps `tier_for` the only place a tier is
  decided (FR-006).

**Rejected**: A new `thunderforge-telemetry` crate. It would hold one file of
SDK wiring that has one caller. If a second binary ever needs it, it can be
moved then.

## R4. Instrument names and the collector's conversion (open item 1)

**Verified** in the live collector config (contrib 0.162.0, configmap
`otel-collector` in `monitoring`):

- The `prometheus` exporter on :8889 is scraped by the ServiceMonitor
  `otel-collector-prometheus`, with port `prom-exporter`, `honorLabels: true`,
  30 s, and the label `release: kube-prometheus-stack`.
- It runs with `resource_to_telemetry_conversion: false`, so resource
  attributes go to `target_info`, not onto every series.
- `job` is `service.namespace/service.name`, or just `service.name` when no
  namespace is set. `instance` is `service.instance.id`.
- Suffixes are added: a monotonic sum gains `_total`, unit `s` becomes
  `_seconds`, and a `{annotation}` unit adds nothing.
- `metric_expiration` is 10m.

**Decision**:

- Instruments are named in OTel's dotted form, and the collector produces the
  Prometheus names. The full table is in
  [contracts/server-instruments.md](contracts/server-instruments.md). For
  example, `thunderforge.backplane.polls` with unit `{poll}` becomes
  `thunderforge_backplane_polls_total`, and
  `thunderforge.graphql.operation.duration` with unit `s` becomes
  `thunderforge_graphql_operation_duration_seconds`.
- **`service.instance.id` is never set.** The instance id travels as
  `thunderforge.instance.id`. If it were `service.instance.id`, it would
  become Prometheus's `instance` label on every series, and Loki 3.7 would
  promote it to an index label by default. Either would break the
  cardinality rule.
- The OTel SDK's default resource detector sets no `service.instance.id`.
  `install.rs` builds the anonymous resource by hand (FR-006) and does not
  merge `OTEL_RESOURCE_ATTRIBUTES` on that tier. A unit test checks that the
  key is absent.
- Every name starts with `thunderforge.`, so every one passes the public
  route's filter `^(thunderforge\.|http\.server\.|db\.client\.)`. A unit test
  in `tier.rs` asserts that.

**Resolved**: Open item 1. The names in FR-010 to FR-014 stand as written.

## R5. Loki over native OTLP (open item 2)

**Verified**: Loki 3.7.8 runs with `allow_structured_metadata: true` and
`retention_period: 336h`. The collector's `otlphttp/loki` exporter posts to
Loki's `/otlp`. `service_name` is an index label, and log attributes arrive as
structured metadata, with dots turned into underscores (`event.name` becomes
`event_name`).

**Decision**:

- LogQL filters with `| event_name="funnel"`, not `| json`. For example,
  `sum by (step) (count_over_time({service_name="thunderforge-demo"} | event_name="funnel" [$__range]))`.
- `session.id` is structured metadata, never an index label. The browser
  never sets `service.instance.id` either (R4).
- The 14 days stated in Appendix A matches `336h`, so the disclosure stands.

**Resolved**: Open item 2.

## R6. The public route's filter, and what it does not strip (open item 7)

**Verified**:

- The public pipelines (receiver on :4319, behind `telemetry.thunderforge.dev`)
  run `memory_limiter` (768 MiB), `filter/public` and `transform/public`, then
  `batch` (8192 / 1 s). They do not run `k8sattributes`.
- `filter/public` drops every metric whose name does not match
  `^(thunderforge\.|http\.server\.|db\.client\.)`.
- `transform/public` deletes `host.name`, `host.ip`, `net.*`,
  `client.address`, `url.full`, `user.*` and `enduser.*` from resource, log,
  datapoint, span and span-event attributes.
- There is no auth and no per-IP rate limit, and the body cap is 4 MiB.

**Decision**:

- Open item 7 is done (fluxified 4890522).
- The collector does **not** strip `process.*`, `container.*` or `k8s.*`, and
  it has no allow-list for span attributes. `tier.rs`'s allow-list span
  processor and hand-built resource are therefore the guarantee, and SC-011
  proves them.
- What remains is a collector-side allow-list, kept as residual hardening on
  the Flux side. It is in the spec's open items as item 7a, and it does not
  block this spec.

## R7. CORS on the public route (new open item)

**Verified**: The public receiver's CORS allows only
`https://thunderforge.dev` and `https://vtt-dev.thunderforge.dev`.

**Finding**: The spec assumed that CORS "allows any origin". It does not.

- A browser on a self-hosted instance's own origin posts
  `Content-Type: application/json`, which needs a preflight. The preflight is
  refused, and the batch is dropped.
- Posting as `text/plain` would skip the preflight, but OTLP/HTTP answers
  that with 415.
- So, today, self-hosted *browsers* reporting to the project's default
  arrive nowhere. Self-hosted *servers* are unaffected, because CORS is a
  browser rule.

**Decision**:

- The code is written for the spec as it stands. The browser posts JSON with
  no credentials, so widening CORS later needs no change here.
- The fix is on the Flux side: `allowed_origins: ["*"]` on the public
  receiver. Without credentials, that is safe. It is one task in the last
  phase (T089), and it waits on the owner's yes.
- Until then, the dashboards' self-hosted browser panels are empty. That is a
  floor, as the spec's first edge case already says of every count.
- The assumption in spec.md is corrected, and the item is added as open
  item 9.

**Rejected**: Proxying browser telemetry through each instance's own server.
That would make every instance forward strangers' posts, which is a bigger
surface than one CORS line.

## R8. Browser alerts: a collector `count` connector (open item 3)

**Verified**: Loki has a `ruler` block with local storage at
`/var/loki/rules`, but no `alertmanager_url` and no rules sidecar. Prometheus
selects `PrometheusRule` objects labelled `release: kube-prometheus-stack` from
every namespace.

**Decision**: A `count` connector on the public and in-cluster logs pipelines
turns browser log records into metrics. It feeds the metrics pipelines, and the
alerts go into the same `PrometheusRule` as the server's.

- The connector's outputs are named `thunderforge.browser.events` and
  `thunderforge.browser.errors`, with the attributes `service.name` and
  `event.name`. Because they start with `thunderforge.`, they pass
  `filter/public`. They are listed in
  [contracts/collector-count-connector.md](contracts/collector-count-connector.md).
- The connector snippet goes to the Flux repository as one task in the last
  phase (T088). The two browser alerts in `prometheus-rules.yaml` are added in
  the same phase, after the series exist.
- FR-031's `loki-rules.yaml` is dropped, and FR-031 is rewritten to match.

**Rejected**: The Loki ruler. It needs a rules sidecar or ConfigMap mount and
an `alertmanager_url`, which is two Flux changes. It would also put a second
alerting path beside Prometheus.

## R9. Applying `deploy/k8s/observability` (open items 4 and 5)

**Verified**:

- `thunderforge-dev` is not Flux-managed. Both Deployments live only in the
  cluster:
  - `thunderforge`: container `app`, port `http` on 30000, label
    `app=thunderforge`;
  - `thunderforge-landing`: container `nginx`, port `http` on 8080, label
    `app=thunderforge-landing`.
- Neither Deployment's manifest is in any repository.
- The Makefile already deploys with
  `kubectl --context $(KUBE_CONTEXT) -n $(KUBE_NAMESPACE)` (`push`,
  `push-landing`).
- Grafana 13.1.1's sidecar loads ConfigMaps labelled `grafana_dashboard: "1"`
  from every namespace.

**Decision**: A `make observability` target applies everything with kubectl.

1. `kubectl apply -k deploy/k8s/observability` applies the dashboards'
   ConfigMaps to `monitoring`, and the `PrometheusRule` and the landing's
   `PodMonitor` to `thunderforge-dev`.
2. `kubectl patch deployment thunderforge-landing --type strategic --patch-file deploy/k8s/observability/landing/exporter-sidecar.yaml`
   adds the exporter.
3. `kubectl set env deployment/thunderforge ...` sets the in-cluster OTLP
   endpoint.

**Why**:

- A kustomize patch, or a Flux Kustomization, needs the base it patches. The
  landing Deployment's base is in no repository, so neither can patch it.
- A strategic-merge patch applied with kubectl is the smallest thing that
  works, and it is idempotent.
- The owner prefers simple, and the Makefile's `push` targets are the
  precedent.

If the namespace is moved under Flux later, the same directory becomes a Flux
Kustomization path, and the patch file moves in beside the Deployment.

**Resolved**: Open items 4 and 5.

## R10. The browser OTLP adapter: hand-written, not the OTel JS SDK

**Decision**: `packages/telemetry/src/otlp/` encodes OTLP/JSON logs and traces
itself, in about 200 lines. The only new browser dependency is `web-vitals`.

**Why**:

- SC-007 caps the telemetry chunk at 25 KB brotli.
- `@opentelemetry/api` 1.9.1, `sdk-trace-web` 2.12.0 and
  `exporter-trace-otlp-http` 0.223.0 come to well over that before logs are
  added. A logs SDK would add more.
- The browser sends one batch shape: `resourceLogs` and `resourceSpans` with
  string, int, double and bool attributes. That is a small, stable encoding
  (OTLP/JSON, with hex trace ids and nanosecond strings).
- A unit test checks the encoder against a fixed OTLP/JSON body.

**Rejected**: The OTel JS SDK. It is the bundle size, and it brings context
managers and auto-instrumentation the spec forbids anyway.

| Package | Version | Licence | Where | Why |
| --- | --- | --- | --- | --- |
| `web-vitals` | 6.2.3 | Apache-2.0 | `packages/telemetry` | LCP, INP, CLS, FCP and TTFB as Google measures them. About 2 KB brotli. |

## R11. Unit tests for `packages/telemetry`: `node --test`

**Decision**: The package is tested with `node --test src/*.test.ts`, which is
how `packages/downloads` does it. That means no Vitest and no new dev
dependency.

**Why**:

- The core is pure: the allow-list, caps and folding, sampling, privacy, batch
  splitting and the queue. The OTLP encoder is pure too.
- The DOM collectors are thin adapters over `PerformanceObserver`,
  `web-vitals` and `addEventListener`. The landing and demo e2e prove them in
  a real browser.

The spec's Proof section says "vitest". It is corrected to the package's
runner.

## R12. Redaction for the landing

**Verified**:

- `apps/web/src/services/feedbackRedaction.ts` (168 lines) imports only
  `config/feedback-redaction.json`.
- The demo already compiles apps/web source through a Vite alias
  (`"@": path.resolve(web, "src")`, `apps/demo/vite.config.mts:57`).

**Decision**:

- The landing reaches the same `redact` through an alias,
  `@thunderforge/feedback-redaction`, pointing at that file.
- The landing passes it into the package's `Redactor` port, as the web app and
  the demo do.
- There is one implementation of the rule set, and spec 037's file is
  unchanged.

**Rejected**:

- Moving `redact` into `packages/telemetry`. It would edit the feedback
  slice's file for no gain.
- A second implementation in the landing. Two implementations would drift.

## R13. The landing's `telemetry.json` and CSP from nginx

**Verified**:

- The `landing` stage is `nginx:stable-alpine`, and its template goes through
  the image's envsubst. That step substitutes only variables that are defined,
  which is why the stage sets `ENV GITHUB_TOKEN=""`.
- The template already uses a `map "${GITHUB_TOKEN}"` pattern.

**Decision**:

- The stage sets `ENV TELEMETRY=true`,
  `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://telemetry.thunderforge.dev`
  and `THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE=1.0`.
- The template builds everything with `map` blocks: the on/off switch, the
  tier (`anonymous` only when the endpoint normalises to the project's), the
  endpoint's origin, the JSON body and the `connect-src` value.
  `location = /telemetry.json` and `location = /demo/telemetry.json` then
  `return 200 $telemetry_json` with `default_type application/json`.
- No file is written into the image's root filesystem at start, and no
  entrypoint script is added. The exact maps are in
  [contracts/served-config-and-csp.md](contracts/served-config-and-csp.md).

## R14. The demo's preview server and its e2e

**Verified**:

- The demo e2e runs `vite preview` over the built bundle
  (`apps/demo/playwright.config.ts`, port 5193).
- `sealedPage()` writes the `<meta>` CSP at build (`vite.config.mts:21`,
  `connect-src` at line 26).
- There are 15 `toEqual([])` assertions over `outside` across the demo e2e,
  not the 14 the spec counted. Nine files use `openDemo`'s list, and
  `demo.spec.ts` keeps its own.

**Decision**:

- A small Vite plugin, `servedTelemetry()`, in
  `packages/telemetry/src/vite.ts`, is added to both the demo's and the
  landing's configs.
  - It serves `telemetry.json`, and `/demo/telemetry.json` for the landing,
    in dev and preview.
  - It sets the `connect-src` header in `preview.headers` from the same
    config, with one builder (`connectSrcFor`).
  - It is off unless `THUNDERFORGE_PREVIEW_TELEMETRY` holds a config. So every
    existing demo spec runs with telemetry off, and spec 074's assertion holds
    unchanged (SC-005).
- `apps/demo/e2e/telemetry.spec.ts` turns telemetry on per test.
  - It routes the document, replacing its CSP header with
    `connectSrcFor(config)` for a fake origin `https://telemetry.invalid`.
  - It routes `**/telemetry.json` to that config.
  - It answers `https://telemetry.invalid/v1/*` with 204 and keeps the bodies.
  - The browser enforces a real header, nothing leaves the machine, and one
    preview server serves every spec.
- The landing's new e2e does the same.
- The server's and nginx's header builders are proven separately: the server's
  by Rust tests on `demo_router`, and nginx's by `scripts/check-landing-nginx.sh`
  (R16).

**Rejected**:

- Playwright's `bypassCSP`. It would hide the very header FR-021 adds.
- A second preview server with telemetry on. It doubles the demo build's
  e2e cost.

## R15. Web e2e: no new harness lane

**Verified**: `scripts/e2e-parallel.mjs` has a first-run lane and a GitHub-apps
lane, each with an environment of its own (`GITHUB_APPS_LANE_ENV`, line 167).
The web app sends no CSP of its own; the only server CSP is
`assets_serve/feedback.rs:132`.

**Decision**:

- The web telemetry specs run on the ordinary stack, which is
  `TELEMETRY=false` (FR-036).
- `telemetry-off.spec.ts` asserts what that stack really serves: `/telemetry.json`
  is `{"enabled":false}`, no telemetry chunk loads, and no request leaves the
  instance.
- The other web telemetry specs route `**/telemetry.json` to an enabled config
  that names `https://telemetry.invalid` (or `https://otel.example.org` for
  the redirect case), and answer it with 204. The web app has no CSP, so
  routing is enough.
- The server's half of each state is proven in Rust, without a browser:
  `/telemetry.json` with the defaults, with a redirected endpoint and with
  `TELEMETRY=false`, through `served_config` tests and a router `oneshot`.
- The spec's Proof line "these start their stack with `TELEMETRY=true`" is
  corrected to this.

**Why**: A third lane is a harness change with its own port and database
partition. And a stack with `TELEMETRY=true` would make the server export
unless it also sets `OTEL_SDK_DISABLED=true`. Routing proves the browser half
with none of that.

## R16. Proving the landing's nginx

**Decision**: A script, `scripts/check-landing-nginx.sh`, builds the
Dockerfile's `landing` stage and runs it three times: with no telemetry
variables, with `TELEMETRY=false`, and with a redirected endpoint. Each time
it checks:

- `/telemetry.json` and `/demo/telemetry.json` (FR-026);
- the `connect-src` on `/` and `/demo/` (FR-021, FR-027);
- that one access-log line, for `/?utm_source=x`, parses as JSON and holds no
  query string, address or user agent (SC-009);
- that `stub_status` answers on `127.0.0.1:8081` inside the container and is
  not reachable on 8080.

The script uses Docker, which every developer here already has for the e2e
stack. It skips with a note when Docker is not running.

## R17. Bunyan carries `trace_id` (FR-004)

**Decision**:

- `TraceLayer::new_for_http().make_span_with(...)` declares the request span
  with `trace_id` and `span_id` as `tracing::field::Empty`.
- `on_request` records them from
  `tracing_opentelemetry::OpenTelemetrySpanExt::context()`.
- `JsonStorageLayer` hands span fields to every event inside the request, so
  each Bunyan line under a request carries them.

**Why**: When the OTel layer is not installed (`TELEMETRY=false`), there is no
context, the fields are never recorded, and `JsonStorageLayer` omits empty
fields. Bunyan's stdout is then unchanged, as SC-001 requires.

## R18. The pool's checkout wait (FR-013)

**Verified**: r2d2 0.8.10's `HandleEvent` receives `CheckoutEvent` (with
`duration()`) and `TimeoutEvent` (with `timeout()`). `Builder::event_handler`
takes a `Box<dyn HandleEvent>`. Diesel re-exports it as `diesel::r2d2`.

**Decision**: `PoolTelemetry` implements `HandleEvent`. It records the
histogram on checkout and counts timeouts. The observable gauges read
`pool.state()` from a clone of the pool. `pool.state()` takes the pool's
mutex once per collection interval (60 s), which is negligible.

## R19. The backplane counters

**Verified**:

- `spawn_listen_task` (`network/listener.rs`) creates the
  `Arc<DeliveryMetrics>` and hands it only to the delivery loop and the
  reporter.
- `subscription_metrics` keeps process-global statics.
- `spawn_channel_reaper` (line 171) prints a count per sweep but keeps no
  cumulative atomic.
- The reporter's `eprintln!` is at about line 211; the spec says 208.

**Decision**:

- `spawn_listen_task` also stores its `Arc<DeliveryMetrics>` in a
  `static DELIVERY: OnceLock<Arc<DeliveryMetrics>>` in `telemetry/instruments.rs`.
  The observable callbacks read the atomics from there, and they read the
  subscription statics directly.
- `spawn_channel_reaper` adds a `static WORLD_CHANNELS_REAPED: AtomicU64`.
- The delivery path gains no work (FR-010), and the reporter is unchanged.

## R20. GraphQL root-field labels

**Decision**:

- The extension (async-graphql 7.2.1 `ExtensionFactory`) reads the operation
  in `execute`, after parsing and validation. Its `root_field` is the first
  selection's field name, checked against the schema's query, mutation and
  subscription field names. That list is built once from `schema.registry()`
  at start.
- A name that is not in the list is labelled `unknown`. More than one root
  field sets `root_fields=multiple` on the span.
- The client's operation name goes on the operator tier's span only. It is not
  on the allow-list, so the anonymous processor drops it, and it is never a
  label.
- `outcome` is `ok`, `error`, or `refused`. `refused` means the error code is
  one of the authorisation codes (`FORBIDDEN`, `UNAUTHENTICATED`,
  `NOT_IN_DEMO`).

**Why**: The schema is the bounded set (about 400 fields). Labelling by a
client's operation name would let any client mint series.

## R21. `world_events.rs` is cross-cutting

**Verified**: `crates/thunderforge-server/src/world_events.rs` is in
`slices.json`'s `crossCutting` list, so `pnpm e2e:which --diff` will say
FULL SUITE once it is edited.

**Decision**:

- The edit is two counter calls in `record_world_event`, plus the name table,
  which lives in `telemetry/event_names.rs`.
- Under the owner's rule, the gate is still slices: `e2e:telemetry`, `rolls`,
  `worlds`, `feedback` and `resumable-downloads`, plus every slice `e2e:which`
  names apart from the full-suite line. The full suite is not run.
- The plan records that `e2e:which` will print FULL SUITE, and that it is
  answered by the named slices.

## R22. Line drift in the spec

These are corrected in research only. The spec's claims hold.

| Spec says | Tree says |
| --- | --- |
| `network/listener.rs:208` | the `eprintln!` is at about 211 |
| `services/graphqlClient.ts` | `apps/web/src/api/graphqlClient.ts` (fetch at 285, `withCsrf` at 318) |
| `engine/bevy/index.ts:509` / `:462` | `mountEngine` at 522, `webgl2Unavailable` at 475 |
| fourteen `toEqual([])` | fifteen |
| `install.ts` line 142 | `refuse("Reaching another website")` at 143 |

## R23. The instance id's key is `system.telemetry_instance_id`

**Verified**: `settings/registry.rs:261` declares
`RESERVED_PREFIX = "system."`, for keys the instance keeps for its own
bookkeeping. `resolver.rs:213` skips that prefix when it reports unrecognised
rows. A row without the prefix and without a declaration would show up in
readiness as an operator's stale setting.

**Decision**: The key is `system.telemetry_instance_id`, not
`telemetry_instance_id`. spec.md (FR-007, Key Entities, the instance-id
decision) and Appendix A.2's guide text are corrected to match.

**Rejected**: Declaring the id in the registry. That would make it resolvable
and writable through `updateInstanceSetting`, which FR-007 forbids.

## R24. The demo's `<meta>` keeps a wider `connect-src`

**Verified**: `sealedPage()` writes `default-src 'self' data: blob:`
(`apps/demo/vite.config.mts:23`). With no `connect-src`, CSP falls back to
`default-src` for `fetch`.

**Finding**: FR-021 says to drop `connect-src` from the `<meta>`. That would
still block the telemetry origin, because the `<meta>`'s `default-src` would
govern `fetch`, and a header cannot loosen a `<meta>`.

**Decision**: The `<meta>` keeps `connect-src 'self' data: blob: https: http:`,
and the served header narrows it to the one origin. The browser enforces both,
so the stricter wins. FR-021 is corrected to this wording. On a host that
sends no header, the guard is the limit, as open item 6 already says.
