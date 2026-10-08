# Feature Specification: Full Telemetry

**Feature Branch**: `086-full-telemetry`
**Created**: 2026-10-07
**Status**: Planned (plan.md, tasks.md)
**Amended**: 2026-10-07, to the owner's decision to put a proxy in front of the public route: "could our telemetry proxy to our in cluster otel ? and add necessary labels and optics about where the event came from and drop anything when its considered spam". User Story 9, FR-037 to FR-046, SC-014 and SC-015, and R25 to R31 record it.
**Input**: The owner, 2026-10-07: "full telemetry for the landing and the demo, like a crazy amount of telemetry, so i can act on it", plus server telemetry and Grafana dashboards on the k8s cluster.
**Revised**: 2026-10-07, to the owner's decision: "for the base image i want TELEMETRY=true default and i want to change our constitution to allow telemetry of people's thunderforge instances to tell me what's going on not just my own but they can override the otel endpoint if they want their own telemetry else i see it and they can do false and this all goes into a disclaimer and spec". Constitution v1.5.0, Principle VII, and [ADR-114](../../docs/adrs/20261007-114-telemetry_is_on_and_the_operators_to_redirect.md) record it.

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

The same blindness covers every instance somebody else runs. A self-hosted
ThunderForge that breaks tells nobody; most people who hit a bug leave
rather than file it. So every build, the self-hostable server image
included, reports to the project by default. What it reports to the project
is anonymous and allow-listed. The operator can send it to their own
collector instead, or turn it off, with one environment variable each, and
they are told so in every place they would look.

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
    (`apps/demo/e2e/support.ts:25-33`). Nine files use it, with fifteen
    `toEqual([])` assertions (R14). `demo.spec.ts` keeps its own copy (line 40).
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
  - In-cluster, the project's own server sends to
    `http://otel-collector.monitoring:4318`. That is an operator endpoint
    (the project operating its own instance), so it gets the full tier.
  - Every other server, and every browser, sends OTLP/HTTP to
    `https://telemetry.thunderforge.dev` by default. That route reaches
    `apps/telemetry-gateway`, a small proxy that answers CORS for any
    origin, labels where each batch came from, drops spam, and forwards
    the rest to the collector's public receiver (User Story 9).
- **The demo sends anonymous usage telemetry, and says so.** This amends
  spec 074. The events cover the funnel, timings and errors. They never
  carry what a visitor types, rolls, names or uploads.
  - The guard lets through exactly one thing: POSTs to the configured
    telemetry origin's OTLP paths.
  - The e2e harness allows exactly that origin and nothing else.
  - The landing and the demo say what is collected.
- **On the server, the OpenTelemetry layer sits beside Bunyan.** The
  `opentelemetry`, `opentelemetry-otlp` and `tracing-opentelemetry` layer
  is added beside the existing Bunyan layer, not in place of it. Bunyan's
  stdout is unchanged in every mode. The stderr report at `listener.rs:208`
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
- **Every build is on by default, decided at runtime (Principle VII).** The
  owner decided on 2026-10-07, first for the landing and demo ("i want
  telemetry on landing and demo absolutely on by default the reason for
  this is i wanna see if people are hitting issues") and then for every
  instance ("for the base image i want TELEMETRY=true default").
  - **One switch, `TELEMETRY`, default `true`.** It is read by the server
    image (the server, the web app it serves, and the demo it serves at
    `/demo`) and by the landing image's nginx. `TELEMETRY=false` sends
    nothing anywhere: the server installs no exporter, and both config files
    answer `{"enabled":false}`, so no browser loads the telemetry chunk.
    `OTEL_SDK_DISABLED=true` is honoured as well and means the same for the
    server's export.
  - **The default destination is `https://telemetry.thunderforge.dev`**,
    for the server's OTLP export and for the browser config the server
    serves at `/telemetry.json` and `/demo/telemetry.json`. The landing's
    nginx serves the same default.
  - **Redirect.** `OTEL_EXPORTER_OTLP_ENDPOINT` (or a signal-specific
    `OTEL_EXPORTER_OTLP_*_ENDPOINT`) sends the server's export to the
    operator's collector, and nothing of it to the project's.
    `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` does the same for browsers.
    The two are independent: an operator may redirect one and leave the
    other on the default, and the disclosures say which is which.
  - Each app reads a small config file at start. The landing reads
    `/telemetry.json`, the demo `${BASE_URL}telemetry.json`. The web app
    reads `/telemetry.json` from the server.
  - The file names the endpoint, the sample rate, the tier, the instance id
    and an `enabled` flag (FR-018).
  - A missing file, an unreadable file or `enabled: false` means nothing is
    sent, and the telemetry chunk is never loaded.
  - No built bundle carries an endpoint. The endpoint is always the served
    config's, so redirecting or turning off never needs a rebuild.
- **Two tiers, decided by the destination.** What is sent depends on where
  it goes, not on who runs the instance.
  - **The anonymous tier** applies whenever the destination is the project's
    collector. Only the allow-list leaves: errors with redacted stacks,
    timings, counts, versions and bounded labels. For the server that means
    metrics, `server.error` event records, and redacted spans whose
    attributes are filtered to the allow-list: no GraphQL variables, no
    client operation names, no ids, no hostnames. The server's logs are not
    sent; there is no log bridge on this tier.
  - **The operator tier** applies when the destination is anything else.
    The operator's collector may receive full logs (through the
    `tracing` log bridge) and unredacted spans, including `world.id`,
    because that data stays on infrastructure the operator controls.
  - **Where it is decided.** One function, `tier_for(endpoint)` in
    `crates/thunderforge-telemetry-policy`, called only through
    `apps/thunderforge/src/telemetry/tier.rs`, compares the resolved
    endpoint with the compiled-in project default
    (`PROJECT_TELEMETRY_ENDPOINT`), after normalising scheme, host case, a
    default port and a trailing slash. Equal means anonymous; anything else
    means operator. The browser's tier comes from the served config, which
    the server fills from the same function. The landing's nginx serves
    `anonymous` unless its endpoint was changed. A destination that cannot
    be parsed is treated as anonymous, never as operator, so a typo can
    only ever send less.
  - Browser telemetry is already anonymous in full (FR-019) and is the same
    on both tiers, except for the extra resource attributes of FR-019a.
- **A random instance id.** So the owner can tell one instance with 50
  errors from 50 instances with one each, the server generates a random
  128-bit UUIDv4 the first time it starts, stores it in `instance_settings`
  under `system.telemetry_instance_id` (the prefix the instance keeps for
  its own bookkeeping, R23), and never regenerates it. It is derived
  from nothing: not the hostname, the database URL, a MAC address, an admin
  or a time. It travels as the resource attribute `thunderforge.instance.id`
  on the server's export, and in the served browser config as `instanceId`,
  which the browser adds as the same resource attribute. It is never a
  Prometheus label (the collector moves it to `target_info`), and never a
  Loki index label. An operator who wants a new one deletes the row.
- **The disclosure is required everywhere Principle VII lists**, and the
  exact words are in **Appendix A**: the README, `docs/guides/telemetry.md`,
  the server's startup log line, the admin settings page, and the landing's
  and demo's **What we measure**.
- **GPC and DNT are a browser rule.** The server never sees a visitor's
  privacy signal for its own export, so the server ignores them. In the
  browser, the rule stands as written below.
- **Tests never report.** Every test run and every e2e stack runs with
  `TELEMETRY=false` (FR-036). The tests that prove telemetry intercept it
  with an in-memory exporter or Playwright routing, so nothing leaves the
  machine.
- **The demo's `connect-src` moves to a header set at serve time.** The
  demo's CSP was a `<meta>` baked at build by `sealedPage()`. A baked policy
  can only name the endpoint known at build, so an operator who redirected
  browser telemetry would have to rebuild the demo, which Principle VII
  forbids. A header cannot loosen a `<meta>` (the browser enforces both, so
  the stricter wins), so the `<meta>`'s `connect-src` is widened to
  `'self' data: blob: https: http:` (it cannot simply go, because
  `default-src` would then govern `fetch`, R24), and the server's demo router and the landing's nginx send it as a
  `Content-Security-Policy` header built from the same config they serve:
  `'self' data: blob:` plus the configured telemetry origin when it is on,
  and nothing more when it is off. Every other directive stays in the
  `<meta>`, where it holds whichever host serves the demo. The guard
  (`install.ts`) stays the in-page enforcement either way.
- **Privacy rules.**
  - The only identifier is a random, session-scoped id: 128 bits, kept in
    `sessionStorage` so the demo's viewer switch (which reloads) stays one
    session, and gone when the tab closes.
  - No cookies, no `localStorage` and no fingerprint.
  - There are no user, world, actor or scene ids in browser telemetry.
    Paths are sent as route templates (`/world/:worldId/play`).
  - The user agent is reduced to browser family and major version, OS
    family and a mobile flag.
  - A browser with Global Privacy Control or Do Not Track set still sends
    error and failure events, because finding people's problems is the
    point. It sends no page views, funnel steps, vitals or traces. This
    rule is the browser's only; the server's export does not consult it.
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
Grafana, the `telemetry.thunderforge.dev` route, the telemetry gateway's
Deployment and Service, retention, and the landing Deployment itself. This
repository owns the gateway's code, its image and its policy crate. Where this spec depends on how that side
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

1. **Given** the server with `TELEMETRY=false`,
   **When** it runs,
   **Then** it opens no connection to a collector, installs no
   OpenTelemetry layer, and its stdout is the same as before this spec,
   apart from the one startup line of FR-008.
2. **Given** `OTEL_EXPORTER_OTLP_ENDPOINT` set to the owner's in-cluster
   collector,
   **When** world events flow,
   **Then** the backplane, GraphQL, pool and world-event series of FR-010
   to FR-014 reach it, with the operator tier's full logs and spans.
3. **Given** the server with no telemetry variable at all,
   **When** world events flow,
   **Then** the same series reach `https://telemetry.thunderforge.dev` on
   the anonymous tier (FR-006): metrics, `server.error` records and
   allow-listed spans, and no log records from `tracing`.
4. **Given** the delivery loop stops polling,
   **When** two minutes pass,
   **Then** `ThunderForgeBackplaneStalled` fires.
5. **Given** a collector that is down,
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
   **Then** only `error` events are sent: no page view, funnel step,
   vital or trace.

---

### User Story 6 - A self-hosted instance reports anonymously, and its operator redirects it or turns it off (Priority: P1)

Somebody runs the ThunderForge server image on their own machine and sets
nothing. Their server, their web app and the demo it serves at `/demo`
report the anonymous tier to `https://telemetry.thunderforge.dev`, so the
owner sees the errors their players hit. If the operator wants the data
themselves, they set the endpoint and it goes to *their* collector, in full,
and nothing reaches the project. If they want none of it, `TELEMETRY=false`
and nothing is sent anywhere.

**Why this priority**: This is the owner's decision of 2026-10-07, and the
image ships with it. A default that reports is only acceptable if both
switches work exactly as written, so they are proven first.

**Independent Test**: The server started with no telemetry variables
answers `/telemetry.json` with `enabled: true`, the project endpoint and
`tier: "anonymous"`. With `TELEMETRY=false` it answers `{"enabled":false}`,
and a web e2e run makes no request outside the instance. With
`THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://otel.example.org` it names
that endpoint and `tier: "operator"`.

**Acceptance Scenarios**:

1. **Given** no telemetry variable,
   **When** the web app or the demo loads from that server,
   **Then** the telemetry chunk loads after `load`, and events go to
   `https://telemetry.thunderforge.dev` and nowhere else, carrying the
   instance's `thunderforge.instance.id`.
2. **Given** `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://otel.example.org`,
   **When** the web app loads,
   **Then** events go there and nowhere else; none reaches the project's
   origin.
3. **Given** `OTEL_EXPORTER_OTLP_ENDPOINT=http://collector.local:4318`,
   **When** the server runs,
   **Then** its export goes there on the operator tier, logs included, and
   nothing of the server's reaches the project.
4. **Given** `TELEMETRY=false`,
   **When** the server, the web app or the demo runs,
   **Then** no telemetry chunk is loaded, no telemetry request is made, and
   the server installs no exporter.
5. **Given** the anonymous tier and a world where an admin with the email
   `canary@example.org` names a token, types in chat and runs a mutation,
   all with a canary string,
   **When** the export is captured,
   **Then** neither the email, nor the canary, nor any user, world, actor or
   scene id, nor the machine's hostname appears in any metric, span or log
   record (SC-011).

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

### User Story 8 - An operator is told, wherever they look (Priority: P1)

An operator who never reads documentation still learns, the first time they
start the server, that it reports, where to, and how to change that. One who
reads the README, the guide or their admin settings learns the same, in the
same words.

**Why this priority**: Principle VII makes the disclosure the condition of
the default. On-by-default that nobody is told about is not what the owner
decided.

**Independent Test**: The server's startup output contains the FR-008 line
for each of the three states. The admin settings page shows the
**Telemetry** panel with the state, destination and switches, and its text
matches Appendix A.

**Acceptance Scenarios**:

1. **Given** the server starts with no telemetry variable,
   **When** its startup log is read,
   **Then** one line says it is on, anonymous, sending to
   `https://telemetry.thunderforge.dev`, and names
   `OTEL_EXPORTER_OTLP_ENDPOINT`, `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT`
   and `TELEMETRY=false`.
2. **Given** an admin opens the admin settings,
   **When** the page renders,
   **Then** a read-only **Telemetry** panel shows the server's and the
   browser's state, tier and destination, the instance id, and the
   variables that change them, linking to `docs/guides/telemetry.md`.
3. **Given** a reader of `README.md`,
   **When** they reach **Telemetry**,
   **Then** they read Appendix A's short text and a link to the guide.
4. **Given** a visitor on the landing or in the demo,
   **When** they open **What we measure**,
   **Then** it says self-hosted instances report the same anonymous set by
   default, and how an operator changes that.

---

### User Story 9 - The owner's endpoint labels where each batch came from, and drops spam (Priority: P2)

`telemetry.thunderforge.dev` takes posts from anyone. The owner wants to
know where each batch came from (his own sites, a self-hosted instance's
browsers, or a self-hosted server), and wants junk dropped before it
reaches the collector, counted by why it was dropped. A small proxy,
`apps/telemetry-gateway`, sits between the public route and the
collector's public receiver to do both. It also answers CORS for any
origin, which is what open item 9 asked of the collector.

**Why this priority**: Without it, self-hosted browsers are refused at
preflight and a single sender can fill the counts. The P1 stories still
work without it, because the project's own sites are already allowed and
the server tier is enforced at the sender.

**Independent Test**: The gateway's integration tests post OTLP/HTTP
batches to the router in process, against a fake upstream, and read what
the fake received and what the gateway's in-memory meter counted. No
cluster is needed.

**Acceptance Scenarios**:

1. **Given** a browser on `https://game.example.org` posting
   `/v1/logs` as JSON,
   **When** its preflight and its post reach the gateway,
   **Then** the preflight is answered with
   `Access-Control-Allow-Origin: *` and no credentials, and the upstream
   receives the batch as protobuf with
   `thunderforge.ingress=public`, `thunderforge.source=self_hosted_browser`
   and `thunderforge.origin.host=game.example.org` on its resource.
2. **Given** a self-hosted server posting `/v1/metrics` as protobuf with
   no `Origin` header,
   **When** the gateway accepts it,
   **Then** the resource carries `thunderforge.source=server`, no
   `thunderforge.origin.host`, and the sender's `thunderforge.instance.id`
   unchanged.
3. **Given** any accepted batch,
   **When** the upstream receives it,
   **Then** no header, attribute or body field holds the sender's IP
   address, and the gateway's own logs and telemetry hold none either.
4. **Given** a sender over its per-IP rate,
   **When** it posts again,
   **Then** it gets `429` with `Retry-After`, nothing reaches the upstream,
   and `dropped{reason="rate_limited_ip"}` goes up by one.
5. **Given** a batch with an unknown `service.name`, a metric name off the
   instrument list, an attribute value over its cap, or a missing or
   non-UUID instance id,
   **When** it arrives,
   **Then** the offending resource or metric is dropped, the rest is
   forwarded, the sender gets `200` with an OTLP `partial_success`, and
   `dropped` goes up under that reason.
6. **Given** the upstream is slow or the gateway is at its concurrency
   limit,
   **When** another post arrives,
   **Then** it gets `503` at once, not after a queue, and
   `dropped{reason="overloaded"}` goes up.

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
- **Tests.** Because the default is now on, a test that forgot to say so
  would report. Rust tests and every e2e stack run with `TELEMETRY=false`
  (FR-036), so they export nothing, unless a test asks for it with an
  in-memory exporter or Playwright routing.
- **The demo served by a self-hosted server** gets its `connect-src` from
  the header its server builds from the served config. By default that
  names the project's origin, which is what the default sends to. An
  operator who redirects gets their own origin in the header with no
  rebuild; one who turns telemetry off gets no extra origin at all.
- **An operator's reverse proxy sets its own CSP** on `/demo/`. Two
  policies intersect, so theirs must allow the telemetry origin too, or the
  browser blocks the post and the batch is dropped as in the first edge
  case. The guide says so.
- **Half redirected.** An operator sets `OTEL_EXPORTER_OTLP_ENDPOINT` but
  not the browser endpoint. The server goes to their collector on the
  operator tier, and browsers still report the anonymous set to the
  project. That is allowed. The startup line and the admin panel show the
  two destinations separately, so it is never a surprise.
- **The project default written out by hand.** An operator who sets
  `OTEL_EXPORTER_OTLP_ENDPOINT=https://telemetry.thunderforge.dev/` gets
  the anonymous tier, because the tier follows the normalised destination,
  not whether a variable was set.
- **No database yet.** The instance id is read after migrations. Until it
  exists (the first start), nothing is exported; export begins once the id
  is stored, so no record ever leaves without one.
- **A forged `Origin`.** A server or a script can send any `Origin` it
  likes, so `thunderforge.source` and `thunderforge.origin.host` are
  indicative, not authenticated. They are for slicing dashboards, never
  for trust. A sender that claims to be `thunderforge.dev` is still held
  to the same allow-lists and rates.
- **The landing and the demo have no instance id.** Their nginx and the
  demo's served config carry none. The gateway lets `thunderforge-landing`
  and `thunderforge-demo` through without one only when the source is
  `owner_site`. Every other batch without a valid id is dropped.
- **No `CF-IPCountry` header.** Whether Cloudflare fronts the domain is
  unverified (R29). Without the header the gateway adds no country; it
  never guesses one from the IP.
- **A release adds an instrument before the gateway knows it.** The new
  metric is dropped as `metric_name` until a gateway built from that
  release is deployed. The gateway ships with, or before, the server
  release that adds it (R27).

## Requirements

### Functional Requirements

**Server: the layer**

- **FR-001**: `apps/thunderforge` MUST add an OpenTelemetry tracer provider
  and meter provider with the OTLP/HTTP exporter. It MUST add a
  `tracing-opentelemetry` layer to the registry built at `main.rs:350`,
  beside `JsonStorageLayer` and `BunyanFormattingLayer`, which stay.
- **FR-002**: The layer MUST be installed unless `TELEMETRY` is `false`
  (case-insensitive; also `0`, `no`, `off`) or `OTEL_SDK_DISABLED` is
  `true`. The endpoint is `OTEL_EXPORTER_OTLP_ENDPOINT` or a
  signal-specific endpoint variable when set, and
  `https://telemetry.thunderforge.dev` otherwise. On the operator tier, all
  other configuration (`OTEL_SERVICE_NAME`, `OTEL_RESOURCE_ATTRIBUTES`,
  `OTEL_TRACES_SAMPLER`, `OTEL_TRACES_SAMPLER_ARG`,
  `OTEL_METRIC_EXPORT_INTERVAL`) is read from the standard variables. On the
  anonymous tier, `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_SERVICE_NAME` are
  ignored, because they are free text the project did not choose; the
  sampler defaults to `parentbased_traceidratio` at 0.1. The service name
  defaults to `thunderforge`.
- **FR-003**: Export MUST be batched and non-blocking, with bounded queues.
  An unreachable collector MUST NOT delay start-up or requests. The
  providers MUST be flushed on graceful shutdown.
- **FR-004**: Bunyan records MUST carry the current `trace_id` and `span_id`
  when a span is active, so Loki's log lines link to Tempo's traces.
- **FR-005**: `crates/thunderforge-server` MUST depend only on the
  `opentelemetry` API crate. Instruments are created from the global meter,
  which is a no-op until FR-001 installs a provider.

**Server: tiers, identity and disclosure**

- **FR-006 The tier.** `crates/thunderforge-telemetry-policy` MUST hold
  `PROJECT_TELEMETRY_ENDPOINT`, `Tier` and `tier_for(endpoint) -> Tier`, as
  decided above. `apps/thunderforge/src/telemetry/tier.rs` re-exports them,
  and it MUST be the only place in the server that decides the tier. On
  `Tier::Anonymous` the server MUST:
  - export metrics (`/v1/metrics`) and spans (`/v1/traces`), and
    `server.error` event records (`/v1/logs`), and install no `tracing`
    log bridge;
  - pass every span through an allow-list span processor that keeps only
    the span name, status, duration, and the attributes
    `graphql.operation.type`, `graphql.root_field`, `graphql.error.codes`,
    `root_fields`, `outcome`, `http.route`, `http.request.method`,
    `http.response.status_code`, `event`, `visibility` and
    `state`; it drops every other attribute, every span
    event other than the redacted `exception`, and every link;
  - emit a `server.error` record for each `tracing` event at `ERROR`, with
    `error.type`, the redacted `error.message` (512 chars) and a backtrace
    reduced to crate paths and lines (4096 chars), through the same
    `config/feedback-redaction.json` rule set, and nothing from the event's
    other fields;
  - set only these resource attributes: `service.name`, `service.version`,
    `thunderforge.instance.id`, `thunderforge.tier=anonymous`, `os.type`,
    `host.arch`, and `deployment.environment=self-hosted`; no
    `host.name`, `process.*`, `container.*` or `k8s.*` detector runs.

  On `Tier::Operator` none of these limits apply beyond FR-014's: the log
  bridge is installed, spans keep every attribute, and the standard
  resource detectors run.
- **FR-007 The instance id.** On start, after migrations, the server MUST
  read the `instance_settings` row `system.telemetry_instance_id` (R23), and if it is absent insert
  a fresh UUIDv4 from the OS random source with an insert that does nothing
  on conflict, then read it back. It MUST NOT overwrite an existing value,
  and nothing else writes that key. With `TELEMETRY=false` it is still
  created, so turning telemetry back on does not mint a new identity.
- **FR-008 The startup line.** After the exporter decision, the server MUST
  log exactly one `INFO` line whose message is Appendix A.3's text for its
  state, with the destination filled in for the server and for browsers.
- **FR-009 The anonymous allow-list is code, and tested.** The span
  attribute allow-list, the resource attribute list and the metric
  instrument list MUST be constants in `crates/thunderforge-telemetry-policy`,
  a unit test MUST enumerate each, and adding an entry MUST be a reviewed
  change to that crate. The server and the gateway (FR-040) read the same
  constants, so the two cannot drift.

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
  - On the operator tier, span attributes MAY carry `world.id`. On the
    anonymous tier they never do (FR-006). On either tier, neither spans
    nor metrics carry a user id, a name or any payload content.

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
  `{ "enabled": bool, "endpoint": url, "sampleRate": 0..1, "environment": string, "tier": "anonymous"|"operator", "instanceId"?: uuid }`.
  `instanceId` is present only when a server serves the file; the landing's
  nginx omits it.
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
  - GPC or DNT MUST limit the session to `error` events, decided before
    the chunk loads.
- **FR-019a Browser tiers.** The browser's events are the same on both
  tiers. The only difference is resource attributes: on both tiers it adds
  `thunderforge.instance.id` when the config carries one, and
  `thunderforge.tier`; on the operator tier it also adds the config's
  `environment` as `deployment.environment`, which on the anonymous tier is
  fixed to `production` for the landing image and `self-hosted` otherwise.
- **FR-020**: The caps are 50 error events and 2,000 events in all per
  session. Batches are flushed every 5 s, on `visibilitychange` to
  hidden, and on `pagehide`. Each batch body is at most 60 KB.

**Browser: what each app reports**

- **FR-021 The demo.**
  - The guard (`apps/demo/src/guard/install.ts`) MUST pass a POST to
    `<telemetry origin>/v1/logs` or `/v1/traces` through to the real
    `fetch`, and only when the config is enabled and names that origin.
    Every other cross-origin request is still refused.
  - `sealedPage()` MUST widen the `<meta>` policy's `connect-src` to
    `'self' data: blob: https: http:` and keep every other directive. It
    cannot drop it, because `default-src` would then govern `fetch` and
    block the telemetry origin whatever the header said (R24). The header
    narrows it, because the browser enforces both and the stricter wins. The server's `demo_router` and the landing's
    nginx `location /demo/` MUST send `Content-Security-Policy:
    connect-src 'self' data: blob: <origin>`, where `<origin>` is the
    served config's endpoint origin when it is enabled and absent when it is
    not. One function builds both the served config and that header on the
    server, so they cannot disagree. The demo's `vite preview` (its e2e
    server) MUST send the same header through `preview.headers`. No build
    variable names a telemetry origin.
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
  you type, roll, name or upload, your IP address, cookies, any account.
  It says what the project's endpoint adds on arrival (the site the report
  came from, and the country when the edge supplies it), and that it uses
  the IP address only in memory, to rate-limit, and never stores or
  forwards it (FR-041, FR-042). It
  also says that self-hosted instances report the same anonymous set by
  default, and how their operators redirect it or turn it off. Its text is
  Appendix A.5. The demo's notice MUST say "Anonymous usage counts go to
  ThunderForge; what you type does not," linking to it. When the served
  config is the operator tier, the notice says "go to this server's
  operator" instead, and when telemetry is off it says nothing about it.
  `apps/demo/index.html:10` MUST no longer say that nothing leaves the
  browser.
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
    `apps/web/src/api/graphqlClient.ts`.

  `startLogCapture` and its buffer are unchanged.
- **FR-026 Serving the config.**
  - The server MUST answer `GET /telemetry.json` and
    `GET /demo/telemetry.json` from `TELEMETRY`,
    `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT`,
    `THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE` and
    `THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT`. Its defaults are on: the
    endpoint `https://telemetry.thunderforge.dev`, sample rate 1.0, and the
    tier from `tier_for`. It adds `instanceId` (FR-007). With
    `TELEMETRY=false` it answers `{"enabled":false}` and nothing else.
  - The landing's nginx MUST answer both from a template fed by the same
    variables, with the same defaults and environment `production`.
    `TELEMETRY=false` turns it off there too.
  - The built bundles MUST NOT carry an endpoint or an `enabled` default of
    their own. Whether anything is sent, and where, is always the served
    config's, so neither switch needs a rebuild.

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
  dropping to zero) MUST be Prometheus alerts in the same
  `prometheus-rules.yaml`, over the series that a collector `count`
  connector makes from browser log records:
  `thunderforge_browser_events_total` and
  `thunderforge_browser_errors_total`, by `service_name` and `event_name`
  (R8, `contracts/collector-count-connector.md`). The connector is a Flux
  change, and the two alerts are added after its series exist. There is no
  `loki-rules.yaml`.

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
    `[]` in all fifteen places.

  No telemetry request leaves the test machine.
- **FR-034**: `docs/guides/telemetry.md` MUST be Appendix A.2, in full: that
  every build, the server image included, reports the anonymous tier to
  `https://telemetry.thunderforge.dev` by default; the `TELEMETRY`,
  `OTEL_*` and `THUNDERFORGE_BROWSER_TELEMETRY_*` variables; the two tiers;
  the instance id; retention; and exactly what the server and the browser
  send on each tier. `CONTRIBUTING.md` MUST state the naming and
  cardinality rules: bounded labels, no ids as labels, the allow-list, and
  that adding to the anonymous allow-list is a constitution-level change
  (Principle VII) reviewed as such.

**The disclosure**

- **FR-035 Where it is said.** The disclosure MUST appear, with Appendix A's
  text, in each of:
  1. `README.md`: a **Telemetry** section, Appendix A.1;
  2. `docs/guides/telemetry.md`: Appendix A.2 (FR-034);
  3. the server's startup log: Appendix A.3 (FR-008);
  4. the admin settings page (`apps/web/src/pages/admin/SettingsPage.tsx`):
     a read-only **Telemetry** panel, Appendix A.4, filled from a new admin
     GraphQL query `telemetryStatus { enabled tier serverEndpoint
     browserEndpoint instanceId }`. The environment variables are the
     control; the panel says which to set and does not offer a toggle;
  5. the landing's and the demo's **What we measure**: Appendix A.5
     (FR-024).

  A change to what is sent MUST change Appendix A and each of these in the
  same commit.
- **FR-036 Tests run with telemetry off.** `TELEMETRY=false` MUST be set:
  - in the e2e harness's stack environment: the `webServer.env` of
    `apps/web/playwright.config.ts` and the stack env in
    `scripts/e2e-parallel.mjs` (beside `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`),
    and in `scripts/journeys.mjs` and `scripts/torture.mjs` where they start a
    server;
  - for `cargo test`, in `.cargo/config.toml`'s `[env]` (which never
    overrides a variable already set), so a test that wants telemetry sets
    it itself, with an in-memory exporter;
  - for the demo's and the landing's e2e preview servers, which serve a
    `telemetry.json` with `enabled: false` unless a telemetry spec serves
    its own. Those specs (FR-033, `apps/landing/e2e/telemetry.spec.ts`)
    turn it on and intercept the endpoint with `page.route`, answering
    `204`, so nothing leaves the machine.

**The telemetry gateway (User Story 9)**

- **FR-037 The service.** `apps/telemetry-gateway` (package
  `thunderforge-telemetry-gateway`) MUST be a thin Axum and Tower binary
  that the `telemetry.thunderforge.dev` route points at. Its configuration
  is `clap` flags with environment fallbacks: the listen address, the
  upstream URL (default `http://otel-collector-public.monitoring:4319`),
  the rate settings and the trusted proxy hop count
  (`contracts/telemetry-gateway.md`). It keeps no state on disk.
- **FR-038 One policy, two users.** `crates/thunderforge-telemetry-policy`
  MUST hold, as pure functions and constants with no network, OTel SDK or
  Axum dependency:
  - the tier (`PROJECT_TELEMETRY_ENDPOINT`, `Tier`, `tier_for`);
  - the resource attribute allow-lists, for the server and for each
    browser `service.name`;
  - the span and log record attribute allow-lists;
  - the metric name allow-list, which is `INSTRUMENTS`' names exactly;
  - the attribute value caps;
  - the instance id shape check (a hyphenated UUID);
  - the known `service.name` values (`thunderforge`,
    `thunderforge-landing`, `thunderforge-demo`, `thunderforge-web`);
  - the source rule, the user agent reduction and the country check of
    FR-041;
  - the token bucket of FR-043, with the clock passed in.

  The server's anonymous tier and the gateway MUST both use it. Neither
  keeps a copy of any list.
- **FR-039 What it accepts.** The gateway MUST accept `POST` on
  `/v1/traces`, `/v1/logs` and `/v1/metrics`, with
  `Content-Type: application/x-protobuf` or `application/json`, decoded
  with `opentelemetry-proto` 0.33 (R26). It MUST:
  - answer `OPTIONS` preflights on those paths for any origin with
    `Access-Control-Allow-Origin: *`, the `POST` method, the
    `Content-Type` header and no credentials;
  - refuse a body over 4 MiB with `413` and a body it cannot decode, or a
    content type it does not know, with `400`;
  - answer `404` to every other path and method;
  - forward what it accepts to the upstream as OTLP/HTTP protobuf, with
    only a `Content-Type` header. No incoming header is copied.

  The collector's public filters stay on behind it as defence in depth.
- **FR-040 What it drops, and at what grain.** For each accepted request,
  the gateway MUST apply FR-038's policy:
  - a resource whose `service.name` is not known is dropped
    (`unknown_service`);
  - a resource whose `thunderforge.instance.id` is present and not a
    UUID, or absent, is dropped (`instance_id`). The one exception is
    `thunderforge-landing` and `thunderforge-demo` with source
    `owner_site`, which may have none;
  - a metric whose name is not on the list is dropped (`metric_name`);
  - a span, log record or data point holding an attribute value over its
    cap is dropped (`attribute_too_large`);
  - an attribute not on its allow-list is removed, and the record kept,
    counted in `thunderforge.telemetry_gateway.attributes_stripped`.

  When anything is dropped and something is left, the gateway MUST
  forward the rest and answer `200` with an OTLP `partial_success` naming
  the rejected count. When nothing is left it forwards nothing and answers
  the same way, so a sender does not retry.
- **FR-041 What it adds.** On every resource it forwards, the gateway MUST
  set these attributes, overwriting any the sender sent under the same
  name:
  - `thunderforge.ingress` = `public`;
  - `thunderforge.source` = `owner_site` when the `Origin` host is
    `thunderforge.dev` or `vtt-dev.thunderforge.dev`,
    `self_hosted_browser` for any other `Origin`, and `server` when there
    is no `Origin`;
  - `thunderforge.origin.host`, the `Origin` header's host, lower-cased,
    without scheme or port, for browsers only (`opaque` for `Origin: null`);
  - `thunderforge.instance.id`, passed through after FR-040's check;
  - `thunderforge.client.version`, the resource's `service.version` when
    it looks like a version, else `unknown`;
  - `thunderforge.user_agent.family` and `thunderforge.user_agent.major`,
    the `User-Agent` header reduced to one of `chrome`, `edge`, `firefox`,
    `safari`, `opera`, `samsung`, `otel-rust` or `other`, and a major
    version number or `unknown`;
  - `thunderforge.country`, the `CF-IPCountry` header when it is two
    capital letters other than `XX` and `T1`. When the header is absent,
    no country is added.

  The full `User-Agent` is never forwarded.
- **FR-042 The IP address.** The gateway MUST use the client IP only in
  memory, as the key of the per-IP rate limit, and only as a keyed hash
  whose key is random per process. It MUST NOT log it, store it, put it
  in its own telemetry, or forward it in any header, attribute or body.
  The client IP is the address `TELEMETRY_GATEWAY_TRUSTED_HOPS` from the
  right of `X-Forwarded-For`, or the peer address when there is none.
- **FR-043 Rates.** The gateway MUST hold two token buckets in memory, in
  maps bounded in size and evicted when idle:
  - per client IP: over the rate, `429` with `Retry-After` in whole
    seconds, and nothing forwarded (`rate_limited_ip`);
  - per instance id, checked after decoding: a resource over its
    instance's rate is dropped (`rate_limited_instance`), and when every
    resource in the request is, the answer is `429` with `Retry-After`.

  The rates and burst sizes are configuration, with defaults in
  `contracts/telemetry-gateway.md`.
- **FR-044 Load shedding.** Per AGENTS.md, the gateway MUST drop, not
  queue: a Tower concurrency limit with load shedding in front of the
  handlers answers `503` at once when full (`overloaded`). The upstream
  client is bounded: a fixed number of requests in flight, a connect and
  a total timeout. A request that cannot get an upstream slot at once, or
  whose upstream call fails or times out, gets `503` (`overloaded` or
  `upstream_error`).
- **FR-045 Its own telemetry.** The gateway MUST report, through the
  OTel SDK, to the in-cluster collector
  (`http://otel-collector.monitoring:4318`), never the public route:
  - `thunderforge.telemetry_gateway.dropped{reason}`, where `reason` is
    one of `rate_limited_ip`, `rate_limited_instance`, `body_too_large`,
    `undecodable`, `unknown_service`, `metric_name`,
    `attribute_too_large`, `instance_id`, `overloaded` or
    `upstream_error`;
  - `thunderforge.telemetry_gateway.accepted{signal, source}`;
  - `thunderforge.telemetry_gateway.attributes_stripped`;
  - `thunderforge.telemetry_gateway.upstream.duration`.

  `server.json` gains a **Public telemetry intake** row, and the rules gain
  `ThunderForgeTelemetryIntakeFailing` and `ThunderForgeTelemetrySpam`.
  No label holds an IP, an origin host or an instance id.
- **FR-046 Shipping it.** The `Dockerfile` MUST gain a
  `telemetry-gateway` stage built the way the server is (cargo-chef
  planner, cook and build), producing
  `mbround18/thunderforgevtt:telemetry-gateway`, and the `server` stage
  stays the default last one. `make push-telemetry-gateway` builds and
  pushes it. In the Flux repository, a Deployment and Service for the
  gateway go in `monitoring`, the `telemetry-thunderforge-dev` route's
  backend moves from `otel-collector-public:4319` to the gateway, and the
  `otlp/public` receiver's `cors` block is removed.

### Key Entities

- **Telemetry config**: `{ enabled, endpoint, sampleRate, environment,
  tier, instanceId? }`, served per origin. It is the runtime switch.
- **Tier**: `anonymous` (the destination is the project's collector) or
  `operator` (anything else), decided only by `tier_for`.
- **Instance id**: a random UUIDv4 in `instance_settings`, created once,
  never regenerated, derived from nothing.
- **Session**: a random 128-bit id, its start time, and the funnel steps
  reached. All three are in `sessionStorage` and die with the tab.
- **Event**: an OTLP log record with `event.name` and allow-listed
  attributes. `session.id` travels as an attribute, which Loki keeps as
  structured metadata.
- **Funnel step**: one of seven names, sent once per session.
- **Server instrument**: a counter, gauge or histogram with a bounded label
  set, named `thunderforge_*`.
- **Telemetry policy**: the allow-lists, caps and checks in
  `crates/thunderforge-telemetry-policy`, read by the server's anonymous
  tier and by the gateway.
- **Source labels**: the `thunderforge.*` resource attributes the gateway
  adds (FR-041), which say where a batch came from. They are indicative,
  because `Origin` can be forged.
- **Rate bucket**: a token bucket held in the gateway's memory, keyed by a
  hashed client IP or by an instance id, gone when the process stops.

## Success Criteria

### Measurable Outcomes

- **SC-001**: With `TELEMETRY=false`, the server opens no outbound
  connection to a collector, and its stdout is the same shape as before
  apart from the one FR-008 line. With no telemetry variable, it exports
  to `https://telemetry.thunderforge.dev` on the anonymous tier and to
  nowhere else; with `OTEL_EXPORTER_OTLP_ENDPOINT` set, to that endpoint
  only.
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
- **SC-005**: With `TELEMETRY=false` (the served config off), the same run
  makes no telemetry request and loads no telemetry chunk, and spec 074's
  original assertion holds unchanged. With a redirected endpoint, every
  telemetry request goes to that origin and none to the project's. With
  GPC set, it sends only `error` events.
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
- **SC-011**: On the anonymous tier, with in-memory exporters, a server
  test seeds an admin `canary-7f3a@example.org`, a world, actor, scene and
  token named with the canary `zq-canary-7f3a`, sends a chat line and a
  mutation whose variables carry it, raises an error whose message holds
  the email, and reads the machine's hostname. No exported metric, span,
  span event or log record contains the email, the canary, the hostname,
  or any seeded user, world, actor, scene or token id; and every span
  attribute and resource attribute is on FR-009's lists. The same test on
  the operator tier finds `world.id`, which proves the filter is what
  removed it. The browser half is SC-004.
- **SC-012**: The FR-008 line appears exactly once in startup output, with
  the right text for each of on, redirected and off. The admin
  **Telemetry** panel and the README and guide sections match Appendix A
  (a test compares the strings), and the instance id survives a restart
  unchanged.
- **SC-013**: A full `cargo test` run and each named e2e slice export
  nothing to any network destination (their stacks run with
  `TELEMETRY=false`, FR-036).
- **SC-014**: The gateway's integration tests, against a fake upstream,
  show each of User Story 9's scenarios: the preflight for an arbitrary
  origin, every label of FR-041 for each source, one test per drop reason
  of FR-045 with the counter at one, the `429` with `Retry-After`, the
  fast `503`, and, for a request sent with a known
  `X-Forwarded-For` address, that the address appears nowhere in what
  the fake upstream received or in the gateway's captured logs and
  metrics.
- **SC-015**: The server's anonymous tier and the gateway read the same
  lists: `crates/thunderforge-telemetry-policy` is the only Rust source
  that names an allow-listed attribute or instrument, a test there fails when
  `packages/telemetry/src/allowList.ts` differs from it, and after the
  cluster apply a batch posted from an origin other than the project's
  reaches Loki with `thunderforge.source=self_hosted_browser`.

### Proof

- Rust, `cargo test -p thunderforge-server` and `-p thunderforge`, with
  in-memory exporters:
  - the three states: off, default and redirected (SC-001);
  - `tier_for`'s normalisation table, including a trailing slash, an
    uppercase host, `:443`, and an unparseable value (anonymous);
  - the anonymous tier's leak test (SC-011);
  - the instance id: created once, unchanged across a restart, untouched
    when present (SC-012);
  - the startup line for each state (SC-012);
  - backplane instruments equal to the atomics;
  - the GraphQL extension's span and labels, including `unknown` and
    `multiple`;
  - pool gauges;
  - the event-code name table.
- `packages/telemetry` unit tests (`node --test`, as `packages/downloads` runs them, R11):
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
    boundary, the engine load trace, `traceparent`, an instance with
    telemetry off, one redirected (US6), and the admin **Telemetry** panel
    (US8). These run on the ordinary `TELEMETRY=false` stack and route
    `**/telemetry.json` to an enabled config whose endpoint `page.route`
    intercepts; the server's half of each state is proven in Rust (R15).
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
- `cargo test -p thunderforge-telemetry-policy`: the lists, the caps, the
  instance id check, the source rule, the user agent reduction, the
  country check and the token bucket under a fixed clock. These are
  written first (TDD) and are deterministic.
- `cargo test -p thunderforge-telemetry-gateway`: the router in process
  with `tower::ServiceExt::oneshot`, against a fake upstream on
  `127.0.0.1`, for SC-014. No slice covers the gateway, and none is added:
  it has no browser surface of its own, and the browsers' half is the
  `telemetry` slice's `page.route` interception, which never reaches it.
- `deploy/k8s/observability`: `kustomize build` succeeds, every dashboard
  parses, the SC-008 script passes, and `promtool check rules` passes.

## Assumptions

- The Flux side runs an OpenTelemetry Collector at
  `otel-collector.monitoring:4318`. It exports metrics to Prometheus, logs
  to Loki over Loki's native OTLP endpoint, and traces to Tempo. It also
  exposes `https://telemetry.thunderforge.dev`. That route will point at
  the telemetry gateway, which answers CORS for any origin and forwards to
  the collector's `otlp/public` receiver on `:4319` (User Story 9). Until
  the gateway is deployed, the receiver's CORS allows only
  `https://thunderforge.dev` and `https://vtt-dev.thunderforge.dev` (R7),
  so self-hosted browsers are refused at preflight. Servers are
  unaffected.
- Whether Cloudflare proxies `telemetry.thunderforge.dev`, and so whether
  `CF-IPCountry` arrives, is unverified (R29). The gateway adds a country
  only when the header is there.
- The shared gateway in front of the route is assumed to append the
  client address to `X-Forwarded-For`, so one trusted hop is the default
  (unverified, R28). If it does not,
  every sender shares one per-IP bucket, which `TELEMETRY_GATEWAY_TRUSTED_HOPS`
  corrects (R28).
- Grafana runs kube-prometheus-stack's dashboard sidecar, which picks up
  ConfigMaps labelled `grafana_dashboard: "1"`.
- `thunderforge-dev` is not Flux-managed, and neither Deployment's manifest
  is in a repository (R9). `make observability` applies the dashboards, the
  rules and the PodMonitor with `kubectl apply -k`, patches the landing's
  exporter sidecar in with `kubectl patch`, and sets vtt-dev's
  `OTEL_EXPORTER_OTLP_ENDPOINT` to the in-cluster collector with
  `kubectl set env`. The landing image's defaults are its `ENV` lines.
- `web-vitals` (Apache 2.0) is an acceptable dependency. Whether the OTLP
  adapter uses the OpenTelemetry JS SDK or a small hand-written JSON
  encoder is a planning decision, bound by SC-007's 25 KB.
- Retention, sampling at the collector, and access to Grafana are the
  Flux side's. The disclosure states 14 days, which is the cluster's Loki
  and Tempo retention today; if that changes, Appendix A changes with it.

## Open items

Planning (research.md) resolved items 1 to 5 and 7 against the live
cluster. The telemetry gateway (User Story 9) resolved 7a and 9. Items 6
and 8 remain open:

1. **Resolved (R4).** **Metric names after the collector.** The names above assume the
   Prometheus exporter adds `_total` and unit suffixes, and turns
   `service.name` into a `job` or `service_name` label. If the collector is
   set up differently, the dashboards and rules change. Instrumentation
   does not.
2. **Resolved (R5): native OTLP, structured metadata.** **Loki over OTLP.** The LogQL assumes Loki 3's native OTLP ingestion:
   `service_name` as an index label, and record attributes as structured
   metadata, so queries filter with `| event_name="funnel"` and not
   `| json`. If logs arrive through a Loki exporter instead, the queries
   change.
3. **Resolved (R8): a collector `count` connector; FR-031 rewritten.** **Browser alerts.** A `PrometheusRule` cannot read Loki. FR-031's rules
   need the Loki ruler enabled, or the collector needs a `count`
   connector over browser log records that turns them into Prometheus
   series. Until one exists, browser errors are dashboards only.
4. **Resolved (R9): `make observability`, because `thunderforge-dev` is not Flux-managed.** **Where `deploy/k8s/observability` is applied from.** This assumes the
   Flux repository adds a `GitRepository` source on this repository and a
   `Kustomization` for that path. It also assumes the dashboard
   ConfigMaps go to the namespace Grafana's sidecar watches (`monitoring`
   assumed), and that the PodMonitor goes to `thunderforge-dev`.
5. **Resolved (R9): `kubectl patch` from `make observability`.** **The landing Deployment is not in this repository.** FR-028 ships the
   sidecar as a patch against `deploy/thunderforge-landing`. If the Flux
   repository owns that Deployment, the patch moves there, and only the
   PodMonitor and the `stub_status` port stay here.
6. **The demo's CSP.** Decided above: `connect-src` is a header built at
   serve time from the served config, and the rest of the policy stays in
   the `<meta>`. The project's origin is allowed by default because that is
   where the default sends; a redirected origin is allowed without a
   rebuild. What remains open is outside this repository: a host that
   serves the built demo without our server or nginx (a plain static host)
   sends no `connect-src` header, so only the guard limits where the page
   posts. The guide says so, and that such a host should send the header
   itself.
7. **Resolved (R6): done in the Flux repository (fluxified 4890522); the remainder is 7a.** **The project collector must accept server metrics publicly.** The
   fluxified `telemetry.thunderforge.dev` route allows only `/v1/traces`
   and `/v1/logs` today. Server-side anonymous telemetry needs
   `/v1/metrics` back on that route. It MUST return only behind:
   - a collector `filter` processor on the public metrics pipeline that
     drops every metric whose name is not `thunderforge.*`,
     `http.server.*` or `db.client.*` (the instrument list of FR-010 to
     FR-014, as SC-008's script prints it);
   - a `resource` / `transform` processor on every public pipeline that
     deletes every resource attribute not on FR-006's list
     (`host.name`, `process.*`, `container.*`, `k8s.*`, `telemetry.sdk.*`
     beyond the name and version), so a misconfigured or hostile sender
     cannot store more than the allow-list;
   - the same span-attribute allow-list as FR-006, applied again at the
     collector for traces from the public route.

   The rate limit this item called future work is now the gateway's
   (FR-043): per IP and per instance id. A sender inside both limits can
   still skew the self-hosted counts within the allowed names, so those
   panels stay indicative.
7a. **Resolved by the gateway (FR-040).** **Collector-side allow-list (residual hardening, Flux side).** The
   public route's `transform/public` does not strip `process.*`,
   `container.*` or `k8s.*`, and the collector has no span-attribute
   allow-list (R6). `tier.rs`'s span processor and hand-built resource are
   the guarantee, and SC-011 proves them. The gateway now applies the same
   lists from the same crate to everything that arrives publicly, so a
   hostile sender meets them too; the collector's filters stay behind it.
8. **GDPR wording for the server-image default.** The anonymous tier and
   the two switches are why on-by-default is defensible: nothing sent
   identifies a person, and the operator controls it with one variable.
   That is not legal advice. Before the server-image default ships to EU
   operators, the owner checks whether the disclosure needs
   legitimate-interest wording (GDPR Art. 6(1)(f)) and whether an
   operator-facing data-processing note is wanted. This is an open item,
   not a blocker; ADR-114 records it.
9. **Resolved by a proxy (R7, R25).** **CORS on the public route.** The public
   receiver allows only `https://thunderforge.dev` and
   `https://vtt-dev.thunderforge.dev`, so self-hosted browsers' JSON posts
   fail preflight (R7). The owner chose a proxy over widening the
   receiver: the gateway answers preflights for any origin without
   credentials (FR-039), the route moves to it, and the receiver's `cors`
   block goes (T089). Until that is applied, the self-hosted browser
   panels are empty.

## Appendix A: The disclosure

These are the words. Each place in FR-035 uses its part verbatim, apart
from Markdown or JSX markup and the values in angle brackets, which are
filled from the running configuration. A change to what is sent changes
this appendix first.

### A.1 README.md

```markdown
## Telemetry

ThunderForge reports anonymous diagnostics to its developers by default,
so we hear about the bugs on instances we don't run. That covers every
build, including the server image you run yourself.

- **What is sent:** errors with personal details stripped from them,
  timings, counts, the ThunderForge version, and a random id for this
  install.
- **What is never sent:** anything anyone types, rolls, names or uploads;
  emails, accounts, or world, character or scene ids; IP addresses;
  machine hostnames; cookies.
- **What we add when it arrives:** where it came from (our own sites,
  your players' browsers and your site's domain, or your server), the
  browser family and major version, and the country when our network edge
  supplies it. Our endpoint uses your IP address only in memory, to limit
  how fast one sender can post, and never stores or forwards it.
- **Where it goes:** `https://telemetry.thunderforge.dev`, kept for 14 days.
- **Send it to your own collector instead:** set
  `OTEL_EXPORTER_OTLP_ENDPOINT` for the server and
  `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` for browsers. Nothing then
  reaches us.
- **Turn it off:** `TELEMETRY=false`. Nothing is sent anywhere.

Details: [docs/guides/telemetry.md](docs/guides/telemetry.md).
```

### A.2 docs/guides/telemetry.md

```markdown
# Telemetry

ThunderForge reports anonymous diagnostics by default. This page says
exactly what, where it goes, and how to send it somewhere else or turn it
off. Both take one environment variable and a restart; nothing needs
rebuilding.

## Why it is on

ThunderForge is self-hosted. When it breaks on your instance, we only find
out if somebody tells us, and most people who hit a bug just leave. The
default lets us see what goes wrong everywhere, not only on the instances
we run. It is anonymous so that this costs you and your players nothing,
and it is yours to change.

## The three settings

| You set | What happens |
| --- | --- |
| nothing | Anonymous diagnostics go to `https://telemetry.thunderforge.dev`. |
| `OTEL_EXPORTER_OTLP_ENDPOINT=<your collector>` | The server's telemetry goes to your collector, in full, and none of it to us. |
| `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=<your collector>` | Your players' browsers report to your collector, and none of it to us. |
| `TELEMETRY=false` | Nothing is sent anywhere. Browsers do not even load the telemetry code. |

The server and browser endpoints are separate. If you set only one, the
other still reports to us anonymously; the startup log and the admin
settings show both destinations.

## What we receive, when it comes to us

Only this, and the list is enforced in code, by the sender
(`crates/thunderforge-telemetry-policy` and `packages/telemetry`) and again
by our endpoint, from the same crate:

- **Errors**: the error's type and its message and stack, with emails,
  tokens and other personal details removed by the same rules as feedback
  reports (`config/feedback-redaction.json`), and cut to a fixed length.
- **Timings**: how long requests, GraphQL operations, database waits,
  page loads and the board's engine take; frame rates.
- **Counts**: requests, GraphQL operations by their field name, world
  events by kind, rolls by visibility, open connections, backplane
  deliveries.
- **Versions and coarse environment**: the ThunderForge version, the
  operating system family and CPU architecture, the browser family and
  major version, a mobile flag, and bucketed screen width, memory and core
  count.
- **Steps reached** in the demo and on the landing, and page views by route
  pattern (`/world/:worldId/play`, never the id itself).
- **A random install id**: a random UUID made the first time your server
  starts, stored in your database (`instance_settings`,
  `system.telemetry_instance_id`), and never changed. It is made from nothing
  about your machine. It lets us tell one instance with fifty errors from
  fifty instances with one each. Delete the row for a new one.
- **A random browser session id** that is forgotten when the tab closes.

## What our endpoint adds

Our endpoint at `telemetry.thunderforge.dev` labels each report before it
is stored, so we can tell where it came from:

- whether it came from our own sites, from a browser on a ThunderForge we
  don't run, or from a server;
- for a browser, the domain of the site it was on (your site's domain,
  for your players), never the page's path;
- your install's random id, after checking it is one;
- the ThunderForge version;
- the browser family and major version, from the `User-Agent` header,
  which is then discarded;
- the country, when our network edge supplies it. If it does not, no
  country is recorded; we never work one out from your address.

Your IP address is used only in the endpoint's memory, to limit how fast
one sender can post. It is never logged, stored, or passed on. Reports
that break the rules above, or come too fast, are dropped and only
counted.

## What we never receive

- Anything anyone types, rolls, names or uploads: chat, names of worlds,
  characters or tokens, notes, dice results, file names, GraphQL variables.
- Emails, usernames, or user, world, character, scene or token ids.
- IP addresses (used in memory to rate-limit, as above, and never kept),
  machine hostnames, container or Kubernetes names, full user agents,
  cookies.
- Your server's logs. Only error records built from the list above leave
  for us.

## Where it goes and how long it is kept

To `https://telemetry.thunderforge.dev`, the project's endpoint, which
labels and checks it as above and passes it to the project's
OpenTelemetry collector, both run by the ThunderForge maintainer. It is
stored in Loki,
Tempo and Prometheus and kept for 14 days. It is not sold or shared, and
it is used only to find and fix problems in ThunderForge.

## Sending it to your own collector

Set `OTEL_EXPORTER_OTLP_ENDPOINT` (and the other standard `OTEL_*`
variables you need) for the server, and
`THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` for browsers. Your collector
then receives everything, not just the anonymous list: the server's full
logs and its spans with every attribute. That data stays on your
infrastructure. Your collector must accept OTLP/HTTP JSON from browsers
on your origin (CORS). If a reverse proxy in front of ThunderForge sets its
own `Content-Security-Policy` on `/demo/`, it must allow your collector's
origin in `connect-src` too.

## Turning it off

`TELEMETRY=false`. The server installs no exporter, `/telemetry.json`
answers `{"enabled":false}`, and no browser loads telemetry code.
`OTEL_SDK_DISABLED=true` also stops the server's export.

## Browser privacy signals

A browser with Global Privacy Control or Do Not Track set reports errors
only: no page views, steps, timings or traces.

## Checking it

The server's startup log names where telemetry is going. Your admin
settings show the same under **Telemetry**. The browser's requests are
visible in its developer tools, to `/v1/logs` and `/v1/traces` on the
endpoint shown.
```

### A.3 The startup log line (FR-008)

One `INFO` line, the first that matches:

- **Off:** `telemetry: off (TELEMETRY=false). Nothing is sent anywhere. See docs/guides/telemetry.md`
- **Otherwise:** `telemetry: server → <server destination> (<tier>), browsers → <browser destination> (<tier>). Anonymous goes to the ThunderForge project; set OTEL_EXPORTER_OTLP_ENDPOINT and THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT to use your own collector, or TELEMETRY=false to send nothing. See docs/guides/telemetry.md`

`<tier>` is `anonymous` or `full`. With nothing set it reads:
`telemetry: server → https://telemetry.thunderforge.dev (anonymous), browsers → https://telemetry.thunderforge.dev (anonymous). Anonymous goes to the ThunderForge project; …`

### A.4 The admin settings panel (FR-035.4)

Heading **Telemetry**, then:

- **On, anonymous (the default):** "This instance sends anonymous
  diagnostics to the ThunderForge project at
  `https://telemetry.thunderforge.dev`: errors with personal details
  removed, timings, counts and versions. Nothing anyone types, rolls, names
  or uploads, and no emails, ids, IP addresses or machine hostnames;
  browsers' reports name your site's domain. Kept 14 days."
- **Redirected:** "This instance sends its telemetry to `<destination>`,
  your own collector, and nothing to the ThunderForge project."
- **Off:** "Telemetry is off. Nothing is sent anywhere."

Then a two-row table, **Server** and **Browsers**, each with its state,
tier and destination; the line "Install id: `<instanceId>`"; and:
"To change this, set `OTEL_EXPORTER_OTLP_ENDPOINT` (server) or
`THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` (browsers) to your own
collector, or `TELEMETRY=false` to turn it off, and restart. [What is
sent](docs/guides/telemetry.md)". There is no toggle.

### A.5 What we measure (landing `/#telemetry`, and the demo)

> **What we measure.** We count what happens, not what you say. This page
> and the demo send us anonymous usage: which pages you view, how far you
> get in the demo, how long things take, errors, your browser family and a
> rough device class, and a random id that is forgotten when you close the
> tab. We never receive what you type, roll, name or upload, cookies, or
> any account. Our endpoint uses your IP address only in memory, to stop
> one sender flooding it, and never stores or passes it on. It notes which
> site the report came from, and your country when our network edge
> supplies it. It goes to `telemetry.thunderforge.dev` and is kept for 14
> days. If your browser sends Global Privacy Control or Do Not
> Track, we receive errors only.
>
> ThunderForge you run yourself sends us the same kind of anonymous
> diagnostics by default, so we hear about the bugs we would otherwise
> never see. Its operator can send them to their own collector instead, or
> turn them off with `TELEMETRY=false`.
> [How](https://github.com/ThunderForgeVTT/ThunderForgeVTT/blob/main/docs/guides/telemetry.md)
