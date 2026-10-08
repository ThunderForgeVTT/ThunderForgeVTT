# Feature Specification: Full Telemetry

**Feature Branch**: `086-full-telemetry`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "full telemetry for the landing and the demo, like a crazy amount of telemetry, so i can act on it", plus server telemetry and Grafana dashboards on the k8s cluster.

## Why

thunderforge.dev is live, and so is the demo behind it. Today nobody can
say how many people open the demo, how many get as far as moving a token,
where the rest give up, or whether the engine failed to load on their
machine. A visitor whose browser has no WebGL2 sees an error and leaves,
and we never hear about it.

The server is in the same position. It counts the things that matter: the
backplane's sent, dropped and polled events, and the subscribers that lag.
Those counts go to stderr as a line every ten seconds
(`network/listener.rs:208`). Nothing graphs them, nothing alerts on them,
and the one fault they were added to catch, a delivery loop that has
stopped, is found by somebody reading a log.

The cluster is getting a collector with Tempo, Loki and Prometheus behind
it. This spec makes ThunderForge feed it: the landing, the demo, the web
app and the server. It also ships the Grafana dashboards and alert rules
that turn the data into something the owner can act on.

"A crazy amount" is the brief, and it is taken at its word for what
visitors *do*: every funnel step, every timing, every error. It is not
taken for what visitors *say*. Nothing anyone types, rolls, names or
uploads is sent, and the pages say so openly.

## What exists

Counted on 2026-10-07:

- **Server logging.** `apps/thunderforge/src/main.rs:350` builds a
  `tracing` registry with an `EnvFilter`, `JsonStorageLayer` and a
  `BunyanFormattingLayer` to stdout. The router carries
  `TraceLayer::new_for_http()` (`main.rs:767`). There is no OpenTelemetry
  dependency anywhere in the workspace, and no `OTEL_*` variable is read.
- **The backplane counters.**
  - `DeliveryMetrics` (`crates/thunderforge-pg/src/delivery.rs:50`) holds
    `polls`, `sent`, `dropped`, `errors`, `panics` and `timeouts` as
    `AtomicU64`, and `cursor` as `AtomicI64`.
  - The subscriber half is `graphql::subscription_metrics`, a module inside
    `crates/thunderforge-server/src/graphql/mutations_admin.rs:143`. Its
    `snapshot()` returns `(sockets_open, opened, refused, delivered,
    lagged_events)`.
  - `spawn_metrics_reporter` (`crates/thunderforge-server/src/network/listener.rs:184`)
    reads both every `METRICS_LOG_INTERVAL_SECS` (10) and `eprintln!`s them
    at line 208. It also prints a "NO POLLS COMPLETED" line when `polls`
    did not move.
  - `spawn_channel_reaper` prints how many idle world channels it released.
- **GraphQL.** One schema, built at `main.rs:559` with
  `Schema::build(...).data(app_state).finish()` and no extensions. It
  answers `/graphql`, `/graphql/public` and the `/ws` subscriptions.
  async-graphql is 7.2.1.
- **The pool.** `DbPool` is r2d2 0.8.10 over Diesel
  (`crates/thunderforge-server/src/state.rs:11`). It is built at
  `main.rs:398` from `thunderforge_pg::pool_sizing_from_env()` and has no
  event handler.
- **World events.** Every event goes through `record_world_event`
  (`crates/thunderforge-server/src/world_events.rs:344`). There are 29
  `EVENT_CODE_*` constants in that file. Rolls are `EVENT_CODE_ROLL_MADE`
  (36) and `EVENT_CODE_ROLL_REVEALED` (37), and their payload carries the
  roll's `visibility` (`roll_event_payload`, line 274).
- **The landing.** `apps/landing` is served by nginx from the Dockerfile's
  `landing` stage, with `apps/landing/nginx.conf.template` as its config.
  `location /` sends a Content-Security-Policy with `connect-src 'self'`.
  `/demo/` sends none of its own: the demo's policy is a `<meta>` written
  at build time by `sealedPage()` in `apps/demo/vite.config.mts:21`, also
  `connect-src 'self' data: blob:`. The access log is nginx's default text
  format. There is no `stub_status`.
- **Where it runs.** `deploy/thunderforge-landing` and `deploy/thunderforge`
  run in namespace `thunderforge-dev`. Their manifests are not in this
  repository, and there is no `deploy/` directory here at all.
- **The demo is sealed** (spec 074).
  - `apps/demo/src/guard/install.ts` replaces `fetch` and refuses any other
    origin with "Reaching another website" (line 142). It also replaces
    `XMLHttpRequest`, and makes `navigator.sendBeacon` return `false`
    (line 189).
  - Spec 074 says so in three places: the Why ("nothing they do leaves
    their browser", `spec.md:27`), the decision that the demo is sealed by
    construction (`spec.md:76`), and SC-003 ("the browser makes no request
    to any path outside the demo's own static files, and opens no
    socket").
  - The demo's e2e records every such request in `outside`
    (`apps/demo/e2e/support.ts:25-33`). Nine files use it, with fourteen
    `toEqual([])` assertions. `demo.spec.ts` keeps its own copy (line 40).
  - The copy repeats it. `DemoNotice.tsx:51` says "Nothing is saved
    anywhere but this browser". The meta description in
    `apps/demo/index.html:10` says "Nothing you do here leaves this
    browser."
- **Browser error capture.** `apps/web/src/services/feedbackLogBuffer.ts`
  (spec 037) already listens to `console`, `error` and
  `unhandledrejection` with `addEventListener`. It redacts every line on
  push through `redact()` in `feedbackRedaction.ts`, using the rule set in
  `config/feedback-redaction.json`, which the server's validator also reads.
  `startLogCapture()` runs in `apps/web/src/main.tsx:43`.
- **No app-level error boundary.** The only React error boundary in
  `apps/web` is `appearance/PackSurfaceBoundary.tsx`. A render throw
  elsewhere unmounts the app.
- **Engine loading.** `mountEngine` (`apps/web/src/engine/bevy/index.ts:509`)
  reports the `downloading` and `starting` stages, and
  `webgl2Unavailable` (line 462) names the reason a board cannot start.
  `engine/bevy/stats.ts` mirrors `fps` and `frameTimeMs` out of the ECS
  each frame.
- **Demo moments.** The viewer switch is `switchView` in `DemoNotice.tsx`
  ("View as player", which reloads the tab). An operation the demo does not
  answer goes through `reportNotInDemo` (`backend/notInDemo.ts:24`). The
  demo has no call to action of its own. The landing's are **Try the
  demo** (`Hero.tsx:385`, `MapLegend.tsx:231`), **Sponsor**
  (`Nav.tsx:39`, `Hero.tsx:390`) and **GitHub** (`Nav.tsx:27`,
  `Hero.tsx:394`, `StarChart.tsx:265`).
- **Browser packages.** `packages/` holds `downloads`, `hero-builder` and
  `heroes`. No app depends on an OpenTelemetry or web-vitals package.

## Decisions already made

- **OpenTelemetry everywhere.** The server and all three browser apps speak
  OTLP. The cluster collector lives in the owner's separate Flux repository
  and fans out to Tempo (traces), Loki (logs) and Prometheus (metrics).
  - In-cluster, the server sends to `http://otel-collector.monitoring:4318`.
  - Browsers send OTLP/HTTP to `https://telemetry.thunderforge.dev`. It
    allows CORS from `https://thunderforge.dev` and
    `https://vtt-dev.thunderforge.dev`.
- **The demo sends anonymous usage telemetry, and says so.** This amends
  spec 074. The events cover the funnel, timings and errors. They never
  carry what a visitor types, rolls, names or uploads.
  - The guard lets through exactly one thing: POSTs to the configured
    telemetry origin's OTLP paths.
  - The e2e harness allows exactly that origin and nothing else.
  - The landing and the demo say what is collected.
- **On the server, the OpenTelemetry layer sits beside Bunyan.** The
  `opentelemetry`, `opentelemetry-otlp` and `tracing-opentelemetry` layer
  is added beside the existing Bunyan layer, not in place of it. The
  standard `OTEL_*` variables turn it on. With none set it is off, and the
  server behaves exactly as today. The stderr report at `listener.rs:208`
  stays.
- **Ports and adapters.**
  - On the server, `thunderforge-server` depends only on the `opentelemetry`
    API crate, which is a no-op until an SDK is installed. Only
    `apps/thunderforge` depends on the SDK, the OTLP exporter and
    `tracing-opentelemetry`.
  - In the browser, a new `packages/telemetry` holds an agnostic core and an
    OTLP adapter. `apps/landing`, `apps/demo` and `apps/web` use it.
- **What the browser sends.** Browsers send logs and traces, not metrics.
  - Each event (a page view, a funnel step, a web vital, an error) is one
    OTLP log record, and page and engine loads are traces.
  - Metric aggregation from thousands of short-lived tabs needs delta
    temporality, and the collector would then have to convert it. Counts
    and percentiles of browser events are read from Loki instead.
- **Browser telemetry is off unless the page is told otherwise, at
  runtime.**
  - Each app reads a small config file at start. The landing reads
    `/telemetry.json`, the demo `${BASE_URL}telemetry.json`. The web app
    reads `/telemetry.json` from the server.
  - The file names the endpoint, the sample rate and an `enabled` flag.
  - A missing file, an unreadable file or `enabled: false` means nothing is
    sent, and the telemetry chunk is never loaded. That is the kill switch.
  - A self-hosted instance, or a self-hosted copy of the demo, sends nothing
    unless its operator sets it up.
- **Privacy rules.**
  - The only identifier is a random, session-scoped id: 128 bits, kept in
    `sessionStorage` so the demo's viewer switch (which reloads) stays one
    session, and gone when the tab closes.
  - No cookies, no `localStorage` and no fingerprint.
  - There are no user, world, actor or scene ids in browser telemetry.
    Paths are sent as route templates (`/world/:worldId/play`).
  - The user agent is reduced to browser family and major version, OS
    family and a mobile flag.
  - A browser with Global Privacy Control or Do Not Track set sends
    nothing. This undercounts, and that is accepted.
- **nginx on the landing** writes JSON access logs and gets a
  `nginx-prometheus-exporter` sidecar, scraped through a PodMonitor
  labelled `release: kube-prometheus-stack`.
- **Dashboards and alerts live in this repository**, under a new
  `deploy/k8s/observability/`:
  - the Grafana dashboards as JSON, wrapped by a kustomization into
    ConfigMaps labelled `grafana_dashboard: "1"`;
  - the alert rules as a `PrometheusRule`;
  - the landing's PodMonitor and the exporter sidecar patch.

  The Flux repository points at that directory. This repository does not
  apply it.
- **Proof is slices, never the full suite.**

## Ownership

This repository owns what ThunderForge emits, and what reads it: the
instrumentation, the config files, the dashboards and the alert rules. The
owner's Flux repository owns the collector, Tempo, Loki, Prometheus,
Grafana, the `telemetry.thunderforge.dev` route and its CORS, retention,
and the landing Deployment itself. Where this spec depends on how that side
is configured, it says so under **Open items**.

## User Scenarios & Testing

### User Story 1 - The owner sees where demo visitors drop off (Priority: P1)

The owner opens the **Demo funnel** dashboard and sees, for the chosen
range, how many sessions reached each step:

1. landing viewed;
2. demo opened;
3. map loaded;
4. token moved;
5. dice rolled;
6. view switched;
7. call to action clicked.

For each step it shows the conversion from the step before and the median
time to reach it.

**Why this priority**: This is the request, in the owner's own words.

**Independent Test**: In the demo's e2e, one visitor runs the whole path
from the landing. Each step's event is posted once, in order, with one
session id.

**Acceptance Scenarios**:

1. **Given** a fresh tab on the landing,
   **When** the visitor clicks **Try the demo**, waits for the map, drags a
   token, rolls a die, clicks **View as player**, then follows a call to
   action,
   **Then** seven `funnel` events are posted in that order, all with the
   same `session.id`. Each carries its step and the milliseconds since the
   session began.
2. **Given** the visitor drags a second token,
   **When** the move lands,
   **Then** no second `funnel` event for "token moved" is sent, and one
   more `demo.action` event with `action=token_moved` is.
3. **Given** the visitor opens the demo directly, not from the landing,
   **When** the demo loads,
   **Then** the funnel starts at "demo opened", and `entry=direct` says so.
4. **Given** the visitor asks for something the demo does not answer,
   **When** `reportNotInDemo` runs,
   **Then** one `demo.not_in_demo` event is posted, naming the GraphQL root
   field that was refused, and nothing else.

---

### User Story 2 - The owner sees the errors visitors hit (Priority: P1)

A visitor's engine fails to start, or a page throws. The owner sees it on
the **Demo funnel and errors** dashboard, or the web app's, grouped by
message. Each group shows the redacted stack, the route template, the
build version and the browser family, which is enough to reproduce it.

**Why this priority**: An error in the demo loses a visitor silently.
Today nobody hears about it.

**Independent Test**: A test route throws on render. The app's error
boundary shows its fallback, and one `error` event is posted with
`error.source=boundary` and the redacted message.

**Acceptance Scenarios**:

1. **Given** a render throw anywhere under `App`,
   **When** it happens,
   **Then** the new app error boundary shows a "something broke" panel with
   a reload button, in place of a blank page, and posts one `error`
   event.
2. **Given** an uncaught exception or an unhandled rejection,
   **When** it reaches `window`,
   **Then** one `error` event is posted, with `error.source` set to
   `onerror` or `unhandledrejection`.
3. **Given** a browser without WebGL2,
   **When** the board tries to start,
   **Then** one `engine.load_failed` event is posted, with the reason
   `webgl2Unavailable` gave.
4. **Given** the same error thrown 500 times in a session,
   **When** they are reported,
   **Then** the first is sent in full and the rest are folded into one count
   per message. A session sends at most 50 error events.
5. **Given** an error message that contains an email or a token,
   **When** it is sent,
   **Then** it has passed through the same `redact()` rules as a feedback
   report (`config/feedback-redaction.json`).

---

### User Story 3 - The owner sees the server's health, and is told when it breaks (Priority: P1)

The owner opens the **Server**, **GraphQL**, **Backplane**, **Database**
and **World events and rolls** dashboards. Each one shows its numbers.
When the delivery loop stops, an alert fires within two minutes. It does
not wait for somebody to read stderr.

**Why this priority**: The backplane stall is the fault that has cost the
most time before. The numbers that show it already exist and only need to
leave the process.

**Independent Test**: With `OTEL_EXPORTER_OTLP_ENDPOINT` pointed at an
in-test collector, the server reports `thunderforge_backplane_polls_total`
and the other backplane series. Their values equal the atomics. A GraphQL
mutation produces one span and one latency observation labelled with its
root field.

**Acceptance Scenarios**:

1. **Given** the server with no `OTEL_*` variable set,
   **When** it runs,
   **Then** it opens no connection to a collector, installs no
   OpenTelemetry layer, and logs exactly as today.
2. **Given** `OTEL_EXPORTER_OTLP_ENDPOINT` set,
   **When** world events flow,
   **Then** the backplane, GraphQL, pool and world-event series of FR-010
   to FR-014 reach the collector.
3. **Given** the delivery loop stops polling,
   **When** two minutes pass,
   **Then** `ThunderForgeBackplaneStalled` fires.
4. **Given** a collector that is down,
   **When** the server starts and runs,
   **Then** start-up does not wait on it, requests are not slowed, and the
   exporter drops what it cannot send.

---

### User Story 4 - The owner sees landing traffic and how fast it feels (Priority: P2)

The **Landing** dashboard shows requests by path and status, bytes, the
cache hit rate of `/gh/`, and nginx's connections. It also shows the web
vitals of real visitors (LCP, INP, CLS, FCP and TTFB at p75, by page),
and which calls to action are clicked.

**Why this priority**: The landing is the front door, but the funnel
(US1) already says the most about it.

**Independent Test**: A request to `/` writes one JSON access-log line with
no query string and no address. The exporter sidecar answers on
`/metrics` with `nginx_up 1`. In the landing's e2e, a page view and its web
vitals are posted.

**Acceptance Scenarios**:

1. **Given** a request for `/?utm_source=x`,
   **When** nginx logs it,
   **Then** the line is JSON with `path="/"`, the status, the bytes, the
   duration and the referrer's origin only. There is no query string, no
   client address and no user agent.
2. **Given** the PodMonitor applied,
   **When** Prometheus scrapes,
   **Then** `nginx_http_requests_total` and `nginx_connections_active` are
   present for the landing pod.
3. **Given** a visitor leaves the landing,
   **When** the page is hidden,
   **Then** that page's web vitals are posted, with `fetch(..., { keepalive:
   true })`.

---

### User Story 5 - A visitor is told what is collected, and nothing they type leaves (Priority: P2)

A visitor reads, on the landing and in the demo's notice, that anonymous
usage counts, timings and errors go to us. They also read that what they
type, roll, name or upload does not. That is true, and a test proves it.

**Why this priority**: Spec 074 made the demo a promise. This spec changes
the promise. It has to change it in the open, and the new promise has to
hold.

**Independent Test**: In the demo's e2e, the visitor types a canary string
into chat, names a token with it, uploads a file whose name contains it,
and rolls. The canary appears in no telemetry request body.

**Acceptance Scenarios**:

1. **Given** the demo,
   **When** it renders,
   **Then** the notice says, in one line, that anonymous usage counts go to
   us. It links to the landing's **What we measure** section.
2. **Given** the landing,
   **When** the visitor opens **What we measure** (`/#telemetry`),
   **Then** it lists what is sent and what is not, as FR-024 states.
3. **Given** Global Privacy Control or Do Not Track,
   **When** any of the three apps loads,
   **Then** no telemetry request is made.

---

### User Story 6 - A self-hosted instance sends nothing unless its operator says so (Priority: P2)

Somebody runs ThunderForge on their own machine. Their web app, and the demo
their server serves at `/demo`, send nothing anywhere. If they want
telemetry, they set environment variables, and it goes to *their*
collector.

**Why this priority**: The web app and the demo ship in the same image as
thunderforge.dev's. A default that phones home would be a breach of trust
in every install.

**Independent Test**: The server started with no telemetry variables
answers `/telemetry.json` with `{"enabled":false}`. A web e2e run then
makes no request outside the instance.

**Acceptance Scenarios**:

1. **Given** no `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT`,
   **When** the web app or the demo loads from that server,
   **Then** no telemetry chunk is loaded and no telemetry request is made.
2. **Given** it set to `https://otel.example.org`,
   **When** the web app loads,
   **Then** events go there and nowhere else.

---

### User Story 7 - Engine loads and slow actions can be traced (Priority: P3)

The owner sees how long the engine takes to download, compile and start,
and the frame rate players get. A slow mutation in the web app can be
followed from the click to the server's span in Tempo.

**Why this priority**: These are useful once US1 to US3 exist, and are not
the first thing the owner will act on.

**Independent Test**: In a web e2e with telemetry on, an `engine.load`
trace has `download`, `compile` and `start` child spans. A GraphQL request
carries a `traceparent` header whose trace id matches the browser span.

**Acceptance Scenarios**:

1. **Given** the board mounts,
   **When** the engine reaches `starting` and then its first frame,
   **Then** one `engine.load` trace is sent, with the bytes downloaded,
   whether the download resumed (spec 080), and each stage's duration.
2. **Given** a session with the board open,
   **When** the page is hidden,
   **Then** one `engine.frames` event is posted. It summarises the fps
   (p5, p50) and frame time (p95) over the session from `stats.ts`, and
   the board's token count bucket.
3. **Given** a sampled browser trace,
   **When** the web app sends a GraphQL request,
   **Then** it carries `traceparent`, and the server's span joins that trace.

---

### Edge Cases

- **The telemetry endpoint is down or blocked** (an ad blocker, a corporate
  proxy). Each batch is tried once. Up to 200 records wait in a bounded
  queue, and the oldest are dropped first. Nothing retries in a loop, no
  user-visible error appears, and a failed send is never itself reported
  as an error, so a failure cannot feed itself. The dashboards say that
  their counts are a floor.
- **Telemetry code throws.** Every call into the package is wrapped. A
  fault in telemetry is swallowed and counted in the next batch's
  `telemetry.internal_errors`. It never reaches the page.
- **The feedback buffer and telemetry both listen** to `error` and
  `unhandledrejection`. Both use `addEventListener`, so neither replaces
  the other, and spec 037's capture is unchanged.
- **The tab is closed mid-batch.** On `pagehide`, the queue is flushed with
  `fetch` and `keepalive: true`. Each flushed body stays under 60 KB, inside
  the browser's 64 KB keepalive limit. `sendBeacon` is not used: the demo
  guard disables it.
- **A second demo tab** (spec 081) gets its own session id, because
  `sessionStorage` is per tab. A tab opened from another one copies it, and
  the funnel accepts that.
- **The viewer switch reloads the tab.** The session id survives in
  `sessionStorage`, and the steps already reached are remembered there too,
  so "demo opened" is not counted twice.
- **Bots.** nginx counts every request. Browser telemetry comes only from
  pages that ran their scripts, so the funnel undercounts bots, which is
  the intent.
- **Clock skew.** Timings are measured with `performance.now()` relative
  to the session start, not with the visitor's wall clock.
- **Cardinality.**
  - Prometheus labels are bounded sets only: GraphQL root field (from the
    schema, never the client's operation name), event code name, outcome,
    roll visibility and pool state.
  - The session id is never a Prometheus label, nor a Loki index label. It
    is Loki structured metadata.
  - An unknown root field is labelled `unknown`.
- **A query with several root fields** is labelled by its first, with
  `root_fields=multiple` on the span.
- **Tests.** Rust tests and the e2e stack run with no `OTEL_*` variable, so
  they export nothing, unless a test asks for it with an in-memory
  exporter.
- **The demo served by a self-hosted server** carries the telemetry origin in
  its built CSP (FR-021), but its `/demo/telemetry.json` says
  `enabled: false`, so the origin is allowed and never used.

## Requirements

### Functional Requirements

**Server: the layer**

- **FR-001**: `apps/thunderforge` MUST add an OpenTelemetry tracer provider
  and meter provider with the OTLP/HTTP exporter. It MUST add a
  `tracing-opentelemetry` layer to the registry built at `main.rs:350`,
  beside `JsonStorageLayer` and `BunyanFormattingLayer`, which stay.
- **FR-002**: The layer MUST be installed only when
  `OTEL_EXPORTER_OTLP_ENDPOINT`, or a signal-specific endpoint variable, is
  set, and `OTEL_SDK_DISABLED` is not `true`. All other configuration
  (`OTEL_SERVICE_NAME`, `OTEL_RESOURCE_ATTRIBUTES`, `OTEL_TRACES_SAMPLER`,
  `OTEL_TRACES_SAMPLER_ARG`, `OTEL_METRIC_EXPORT_INTERVAL`) is read from
  the standard variables. The service name defaults to `thunderforge`.
- **FR-003**: Export MUST be batched and non-blocking, with bounded queues.
  An unreachable collector MUST NOT delay start-up or requests. The
  providers MUST be flushed on graceful shutdown.
- **FR-004**: Bunyan records MUST carry the current `trace_id` and `span_id`
  when a span is active, so Loki's log lines link to Tempo's traces.
- **FR-005**: `crates/thunderforge-server` MUST depend only on the
  `opentelemetry` API crate. Instruments are created from the global meter,
  which is a no-op until FR-001 installs a provider.

**Server: what it reports**

Names are given as Prometheus will show them, after the collector's
conversion (see **Open items**).

- **FR-010 Backplane.**
  - Observable instruments MUST read the existing atomics. They MUST NOT
    add work to the delivery path or the subscription path.
  - Counters: `thunderforge_backplane_sent_total`, `_dropped_total`,
    `_polls_total`, `_errors_total`, `_panics_total` and
    `_timeouts_total`. Also
    `thunderforge_subscriptions_opened_total`, `_refused_total`,
    `_delivered_total` and `_lagged_total`, and
    `thunderforge_world_channels_reaped_total`.
  - Gauges: `thunderforge_backplane_cursor` and
    `thunderforge_websocket_sockets_open`.
  - `spawn_metrics_reporter` and its stderr lines stay as they are.
- **FR-011 GraphQL.** An async-graphql extension, registered at
  `Schema::build` in `main.rs:559`, MUST:
  - open one span per operation, named
    `graphql.<operation_type> <root_field>`, with `graphql.operation.type`,
    `graphql.root_field`, the client's `graphql.operation.name` (on the
    span only) and the error codes of any errors;
  - record `thunderforge_graphql_operation_duration_seconds`, a histogram
    with labels `operation_type`, `root_field` and
    `outcome` (`ok`, `error` or `refused`);
  - count `thunderforge_graphql_errors_total` by `root_field` and
    `code`, where `code` is the error's `extensions.code`, or `internal`
    when it has none.

  Subscriptions get a span for their setup only, not for their life.
- **FR-012 HTTP.** `TraceLayer::new_for_http()` spans MUST become OTel
  server spans with the route template, method and status. The collector's
  span metrics, or an equivalent server histogram
  `thunderforge_http_server_duration_seconds{route, method, status_class}`,
  MUST give the **Server** dashboard its rate, errors and duration.
- **FR-013 Pool.**
  - Observable gauges MUST read `pool.state()`:
    `thunderforge_db_pool_connections{state="idle"|"in_use"}` and
    `thunderforge_db_pool_max_connections`.
  - An r2d2 `HandleEvent` handler, set with `Builder::event_handler` at
    `main.rs:398`, MUST record
    `thunderforge_db_pool_checkout_wait_seconds` (a histogram) and
    `thunderforge_db_pool_checkout_timeouts_total`.
- **FR-014 World events and rolls.**
  - `record_world_event` MUST count `thunderforge_world_events_total{event}`
    and `thunderforge_world_event_record_failures_total{event}`, where
    `event` is the code's name (`roll_made`, `token_changed` and so on).
  - A single table MUST map every `EVENT_CODE_*` to its name, and a unit
    test MUST fail when a code is added without a name.
  - Rolls MUST also count `thunderforge_rolls_total{event, visibility}`.
  - Span attributes MAY carry `world.id`. Neither spans nor metrics carry
    a user id, a name or any payload content.

**Browser: `packages/telemetry`**

- **FR-015**: A new workspace package, `packages/telemetry`, MUST hold:
  - **the core**, which knows nothing about OTLP:
    - the session id;
    - the event model and its attribute allow-list;
    - sampling, the per-session caps and the bounded queue;
    - the privacy checks of FR-019;
    - the collectors: web vitals, long tasks, navigation timing, errors and
      page views;
    - a `TelemetrySink` port;
  - **an OTLP/HTTP adapter** implementing `TelemetrySink`, which posts
    `/v1/logs` and `/v1/traces` as JSON;
  - **a no-op sink**, which tests use, and which is used whenever
    telemetry is off.
- **FR-016**: Redaction MUST be injected as a port. `apps/web` and the demo
  pass `redact` from `feedbackRedaction.ts`. The landing passes the same
  rule set, read from `config/feedback-redaction.json`. The package MUST
  NOT import from `apps/*`.
- **FR-017**: The apps MUST load the package by dynamic `import()`, after
  the config file says it is on, and after the page's `load` event. The
  first paint waits for none of it.
- **FR-018**: The config file is
  `{ "enabled": bool, "endpoint": url, "sampleRate": 0..1, "environment": string }`.
  - **Unsampled:** funnel, `demo.action`, `demo.not_in_demo`, `error`,
    `engine.load_failed` and page views are always sent.
  - **Sampled per session at `sampleRate`:** web vitals, long tasks,
    `engine.frames` and traces.
  - The demo reads the file with the guard's own static fetch.
- **FR-019 Privacy.**
  - Every event attribute MUST be on the core's allow-list. An attribute
    not on it is dropped before it is queued, and a unit test enumerates
    the list.
  - Free text is allowed only in `error.message` and `error.stack`, after
    redaction, truncated to 512 and 4096 characters. Stack frames keep
    only the script path and line, without query strings.
  - Resource attributes:
    - `service.name` (`thunderforge-landing`, `thunderforge-demo` or
      `thunderforge-web`);
    - `service.version` (the build's version);
    - `deployment.environment`;
    - browser family and major version, OS family, and a mobile flag;
    - a viewport width bucket;
    - device memory and core-count buckets.
  - No IP, cookie, user or world id, or full user agent.
  - GPC or DNT MUST turn telemetry off before the chunk loads.
- **FR-020**: The caps are 50 error events and 2,000 events in all per
  session. Batches are flushed every 5 s, on `visibilitychange` to
  hidden, and on `pagehide`. Each batch body is at most 60 KB.

**Browser: what each app reports**

- **FR-021 The demo.**
  - The guard (`apps/demo/src/guard/install.ts`) MUST pass a POST to
    `<telemetry origin>/v1/logs` or `/v1/traces` through to the real
    `fetch`, and only when the config is enabled and names that origin.
    Every other cross-origin request is still refused.
  - `sealedPage()` MUST add the telemetry origin, from
    `VITE_TELEMETRY_ORIGIN` at build, to `connect-src`, and nothing else.
  - Funnel steps:
    - `demo_opened`, with `entry=landing|direct`;
    - `map_loaded`: the engine's first frame with the scene's map drawn;
    - `token_moved`: the first `upsert_token` from `ui` that the demo
      backend accepts;
    - `dice_rolled`: the first `ROLL_MADE`;
    - `view_switched`: `switchView`;
    - `cta_clicked`.

    Each is sent once per session, with ms since session start.
  - `demo.action` counts every token move, wall, door, light, shape, roll,
    scene change, map import and start-over by kind, with no content.
  - `demo.not_in_demo` carries the refused root field.
  - The hooks MUST live where the world store, the event sync and the demo
    backend already see these moments, not in components (AGENTS.md §2).
- **FR-022 The demo's call to action.** `DemoNotice` MUST gain one link,
  **Run your own**, to the landing's self-host section (`/#self-host`).
  That link, and the landing's **Try the demo**, **Sponsor** and **GitHub**
  links, post `cta_clicked` with `cta=<name>`.
- **FR-023 The landing** MUST report:
  - page views with the referrer's origin and any `utm_source`,
    `utm_medium` and `utm_campaign`;
  - the funnel's `landing_viewed`;
  - CTA clicks;
  - how far down the page the visitor scrolled, by section reached;
  - web vitals;
  - long tasks;
  - errors.
- **FR-024 What we measure.** The landing MUST gain a **What we measure**
  block at `/#telemetry`. It lists what is sent: pages viewed, steps
  reached in the demo, timings, errors, coarse browser and device class,
  and a random id that dies with the tab. It also lists what is not: what
  you type, roll, name or upload, your address, cookies, any account. The
  demo's notice MUST say "Anonymous usage counts go to ThunderForge; what
  you type does not," linking to it. `apps/demo/index.html:10` MUST no
  longer say that nothing leaves the browser.
- **FR-025 The web app** MUST:
  - add an app-level error boundary in `App.tsx` that reports to the
    package;
  - report page views by route template, web vitals, long tasks and
    errors;
  - add the `engine.load` trace with `download`, `compile` and `start`
    spans from `mountEngine`'s stages;
  - post `engine.load_failed` with `webgl2Unavailable`'s reason;
  - post the `engine.frames` summary from `stats.ts`;
  - in sampled sessions, add `traceparent` to GraphQL requests in
    `graphqlClient.ts`.

  `startLogCapture` and its buffer are unchanged.
- **FR-026 Serving the config.**
  - The server MUST answer `GET /telemetry.json` and
    `GET /demo/telemetry.json` from `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT`,
    `THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE` and
    `THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT`. With no endpoint it
    answers `enabled: false`.
  - The landing's nginx MUST answer both from a template fed by the same
    variables.
  - The built bundles MUST NOT carry a default that is on.

**nginx (landing)**

- **FR-027**: `apps/landing/nginx.conf.template` MUST:
  - add `https://telemetry.thunderforge.dev` (in the template, the
    configured telemetry origin) to `location /`'s `connect-src`, and
    nothing else;
  - write access logs as JSON (`log_format ... escape=json`) with time,
    method, `$uri` (never `$request_uri`), status, bytes sent, request
    time, `$upstream_cache_status`, and the referrer reduced to its origin
    by a `map`. It MUST NOT log the client address, `X-Forwarded-For` or the
    user agent. `/healthz` stays unlogged;
  - add `stub_status` on `127.0.0.1:8081` only, so it is not reachable
    through the Service.
- **FR-028**: `deploy/k8s/observability/landing/` MUST hold a kustomize
  patch that adds the `nginx/nginx-prometheus-exporter` sidecar to
  `deploy/thunderforge-landing`. The sidecar scrapes
  `http://127.0.0.1:8081/stub_status` and serves the named port `metrics`
  (9113). Beside the patch, a `PodMonitor` in `thunderforge-dev`, labelled
  `release: kube-prometheus-stack`, selects the landing pods' `metrics`
  port.

**Dashboards and alerts**

- **FR-029**: `deploy/k8s/observability/dashboards/*.json` MUST hold one
  Grafana dashboard per item below. A `kustomization.yaml` wraps them with a
  `configMapGenerator` labelled `grafana_dashboard: "1"`, with the
  annotation `grafana_folder: ThunderForge`. Every dashboard uses datasource
  template variables (`${prometheus}`, `${loki}`, `${tempo}`), never a
  fixed UID.
  1. **Server**: request rate, error ratio and p50/p95/p99 duration by
     route, from FR-012.
  2. **GraphQL operations**: top root fields by rate, p95 latency and error
     ratio, errors by code, and a link from a slow field to its traces in
     Tempo.
  3. **Backplane**: sent, dropped and poll rates, the cursor, errors,
     panics and timeouts, open sockets, and subscriptions opened, refused,
     delivered and lagged.
  4. **Database**: pool in use against max, checkout wait p95, and checkout
     timeouts.
  5. **World events and rolls**: events by name, record failures, and rolls
     by event and visibility.
  6. **Landing**: from nginx, requests by path and status, bytes, `/gh/`
     cache status and connections. From Loki, page views, referrers and
     UTM sources, CTA clicks, scroll depth, and web vitals at p75 by page.
  7. **Demo funnel and errors**:
     - the seven steps as a funnel, with step-to-step conversion and the
       median time to each step;
     - `demo.action` by kind and `demo.not_in_demo` by root field;
     - errors by message, with a sample of redacted stacks;
     - engine load time and failures by reason;
     - frame-rate percentiles;
     - the same errors and engine panels for `thunderforge-web`.
- **FR-030**: `deploy/k8s/observability/prometheus-rules.yaml` MUST be a
  `PrometheusRule` labelled `release: kube-prometheus-stack`, with these
  alerts, each with a `summary` that says what to look at:
  - `ThunderForgeBackplaneStalled`: no increase in
    `thunderforge_backplane_polls_total` for 2 m (critical).
  - `ThunderForgeBackplanePanicked`: any increase in `_panics_total` in
    5 m.
  - `ThunderForgeBackplaneErrors`: errors or timeouts above 0.1/s for
    10 m.
  - `ThunderForgeSubscribersLagging`: `_lagged_total` rising above 1/s for
    10 m.
  - `ThunderForgeWorldEventsNotRecorded`: any increase in
    `thunderforge_world_event_record_failures_total` in 5 m.
  - `ThunderForgeGraphQLErrorRatio`: over 5 % of operations ending in
    `error` for 10 m.
  - `ThunderForgeGraphQLSlow`: p95 over 1 s for 10 m.
  - `ThunderForgeDbPoolSaturated`: in use over 90 % of max for 5 m, or any
    checkout timeout.
  - `ThunderForgeServerSilent`: `absent_over_time` of the polls series for
    5 m. The server is down or not exporting.
  - `ThunderForgeLandingDown`: `nginx_up == 0` for 2 m.
  - `ThunderForgeLanding5xx`: 5xx above 1 % for 10 m.
- **FR-031**: Browser-side alerts (a spike in demo errors, a funnel step
  dropping to zero) MUST be written as Loki ruler rules in
  `deploy/k8s/observability/loki-rules.yaml`. They are applied once the
  Flux side confirms a Loki ruler (see **Open items**).

**The amendment to spec 074, and its tests**

- **FR-032**: `specs/074-a-world-to-try/spec.md` MUST be amended where it
  promises a sealed demo: the Why at line 27, the decision at line 76 and
  SC-003. Each MUST carry a note that points to this spec, and SC-003 MUST
  read: "...makes no request outside the demo's own static files, except
  telemetry to the configured telemetry origin, and opens no socket."
- **FR-033**: `openDemo` (`apps/demo/e2e/support.ts:17`) and
  `demo.spec.ts`'s own copy MUST:
  - route requests to the telemetry origin with `page.route`, answer them
    `204`, and keep their bodies in a new `telemetry` list;
  - leave every other request to `outside`, which stays asserted equal to
    `[]` in all fourteen places.

  No telemetry request leaves the test machine.
- **FR-034**: `docs/guides/telemetry.md` MUST explain, for operators, the
  `OTEL_*` and `THUNDERFORGE_BROWSER_TELEMETRY_*` variables, that
  everything is off by default, and exactly what the browser sends.
  `CONTRIBUTING.md` MUST state the naming and cardinality rules: bounded
  labels, no ids as labels, and the allow-list.

### Key Entities

- **Telemetry config**: `{ enabled, endpoint, sampleRate, environment }`,
  served per origin. It is the runtime switch.
- **Session**: a random 128-bit id, its start time, and the funnel steps
  reached. All three are in `sessionStorage` and die with the tab.
- **Event**: an OTLP log record with `event.name` and allow-listed
  attributes. `session.id` travels as an attribute, which Loki keeps as
  structured metadata.
- **Funnel step**: one of seven names, sent once per session.
- **Server instrument**: a counter, gauge or histogram with a bounded label
  set, named `thunderforge_*`.

## Success Criteria

### Measurable Outcomes

- **SC-001**: With no `OTEL_*` variable, the server opens no outbound
  connection to a collector, and its stdout is byte-for-byte the same
  shape as before.
- **SC-002**: With an in-memory exporter, every series of FR-010 is
  reported, and its value equals the atomic it reads. A mutation produces
  exactly one operation span and one histogram observation whose
  `root_field` is the mutation's field. A client-chosen operation name
  appears in no metric label.
- **SC-003**: A demo e2e run of the whole funnel posts the seven steps
  once each, in order, under one session id.
- **SC-004**: In that run, the canary string typed into chat, used as a
  token name and in an uploaded file's name appears in no telemetry body.
  Every request outside the demo's files goes to the telemetry origin's
  `/v1/logs` or `/v1/traces`.
- **SC-005**: With the config off, or with GPC set, the same run makes no
  telemetry request and loads no telemetry chunk. Spec 074's original
  assertion holds unchanged.
- **SC-006**: A render throw shows the error boundary and posts one
  redacted `error` event.
- **SC-007**: The landing's initial document and script requests are
  unchanged in number. The telemetry chunk loads after `load`, and is at
  most 25 KB brotli, checked by the landing's build script.
- **SC-008**: Every PromQL and LogQL expression in the dashboards and rules
  names only series and attributes this code emits. A script checks it
  against the instrument list that a Rust test prints and the core's
  allow-list. `promtool check rules` passes.
- **SC-009**: The landing's access-log line parses as JSON and holds no
  query string, address or user agent.
- **SC-010**: `make lint` (host and wasm32) passes.

### Proof

- Rust, `cargo test -p thunderforge-server` and `-p thunderforge`, with
  in-memory exporters:
  - the off-by-default path (SC-001);
  - backplane instruments equal to the atomics;
  - the GraphQL extension's span and labels, including `unknown` and
    `multiple`;
  - pool gauges;
  - the event-code name table.
- `packages/telemetry` unit tests (vitest):
  - the allow-list;
  - redaction through the port;
  - caps and folding;
  - sampling;
  - GPC/DNT;
  - the 60 KB batch split;
  - the bounded queue under a dead endpoint.
- A new `telemetry` slice, `pnpm e2e:telemetry`:
  - **standalone**: the whole demo e2e (`pnpm -F @thunderforge/demo e2e`),
    because `support.ts` changes under every demo spec, with a new
    `apps/demo/e2e/telemetry.spec.ts` for SC-003 to SC-005; and a new
    `apps/landing/e2e/telemetry.spec.ts` for page views, CTAs and vitals;
  - **integration**: `apps/web/e2e/telemetry-*.spec.ts` for the error
    boundary, the engine load trace, `traceparent`, and an instance with
    telemetry off (US6).
- Neighbouring slices:
  - `pnpm e2e:feedback`: the shared error listeners and redaction;
  - `pnpm e2e:resumable-downloads`: it owns
    `apps/web/src/engine/bevy/index.ts`;
  - `pnpm e2e:rolls`: roll counters, and the demo's
    `rolls-across-tabs`;
  - `pnpm e2e:worlds`: `App.tsx` and live sync through the GraphQL
    extension;
  - whatever `pnpm e2e:which --diff` names.

  `App.tsx` and `graphqlClient.ts` are cross-cutting, and the named slices
  are the proof for them. The full suite is not run.
- `deploy/k8s/observability`: `kustomize build` succeeds, every dashboard
  parses, the SC-008 script passes, and `promtool check rules` passes.

## Assumptions

- The Flux side runs an OpenTelemetry Collector at
  `otel-collector.monitoring:4318`. It exports metrics to Prometheus, logs
  to Loki over Loki's native OTLP endpoint, and traces to Tempo. It also
  exposes `https://telemetry.thunderforge.dev` with the CORS above.
- Grafana runs kube-prometheus-stack's dashboard sidecar, which picks up
  ConfigMaps labelled `grafana_dashboard: "1"`.
- The vtt-dev deployment sets the `OTEL_*` and
  `THUNDERFORGE_BROWSER_TELEMETRY_*` variables, and the landing deployment
  sets the nginx template's. Those settings live in the Flux repository.
- `web-vitals` (Apache 2.0) is an acceptable dependency. Whether the OTLP
  adapter uses the OpenTelemetry JS SDK or a small hand-written JSON
  encoder is a planning decision, bound by SC-007's 25 KB.
- Retention, sampling at the collector, and access to Grafana are the
  Flux side's.

## Open items

These depend on the Flux repository, which is still being built. Each is
stated so that planning can proceed and the answer only adjusts names:

1. **Metric names after the collector.** The names above assume the
   Prometheus exporter adds `_total` and unit suffixes, and turns
   `service.name` into a `job` or `service_name` label. If the collector is
   set up differently, the dashboards and rules change. Instrumentation
   does not.
2. **Loki over OTLP.** The LogQL assumes Loki 3's native OTLP ingestion:
   `service_name` as an index label, and record attributes as structured
   metadata, so queries filter with `| event_name="funnel"` and not
   `| json`. If logs arrive through a Loki exporter instead, the queries
   change.
3. **Browser alerts.** A `PrometheusRule` cannot read Loki. FR-031's rules
   need the Loki ruler enabled, or the collector needs a `count`
   connector over browser log records that turns them into Prometheus
   series. Until one exists, browser errors are dashboards only.
4. **Where `deploy/k8s/observability` is applied from.** This assumes the
   Flux repository adds a `GitRepository` source on this repository and a
   `Kustomization` for that path. It also assumes the dashboard
   ConfigMaps go to the namespace Grafana's sidecar watches (`monitoring`
   assumed), and that the PodMonitor goes to `thunderforge-dev`.
5. **The landing Deployment is not in this repository.** FR-028 ships the
   sidecar as a patch against `deploy/thunderforge-landing`. If the Flux
   repository owns that Deployment, the patch moves there, and only the
   PodMonitor and the `stub_status` port stay here.
6. **The demo's built CSP** names thunderforge.dev's telemetry origin in
   every image, including self-hosted ones. It is never used unless
   enabled (US6), but a self-hoster who points browser telemetry at their
   own collector also needs the demo rebuilt with their origin. The
   alternative is to drop the `<meta>` policy in favour of a header set at
   serve time.
