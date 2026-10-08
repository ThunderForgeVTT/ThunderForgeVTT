# Implementation Plan: Full Telemetry

**Branch**: `086-full-telemetry` | **Date**: 2026-10-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/086-full-telemetry/spec.md`

## Summary

ThunderForge starts speaking OTLP from four places: the server, the web app,
the demo and the landing. A fifth piece, the telemetry gateway, stands in
front of the project's public collector.

**The server.** The server gains an OpenTelemetry layer beside Bunyan.

- One function, `tier_for`, decides whether the destination is the project's
  collector (the **anonymous** tier) or anyone else's (the **operator** tier).
- On the anonymous tier, an allow-list span processor and a hand-built
  resource send metrics, redacted `server.error` records and filtered spans,
  and nothing else.
- The backplane, subscription, pool, GraphQL, HTTP and world-event numbers
  that already exist become observable instruments. The delivery path gains
  no work.
- A random instance id is stored once, in `instance_settings` under the
  reserved `system.` prefix.

**The browser.** A new `packages/telemetry` holds an agnostic core, a
hand-written OTLP/JSON adapter and a no-op sink. Its only dependency is
`web-vitals`, which keeps the lazy chunk inside 25 KB brotli.

- Each app reads a served `telemetry.json`. Off, missing or unreadable means
  the chunk is never imported.
- The server and the landing's nginx build that file and the demo's
  `connect-src` header from the same values. So turning telemetry off, or
  redirecting it, never needs a rebuild.

**The gateway.** `apps/telemetry-gateway` is a thin Axum and Tower binary
behind `telemetry.thunderforge.dev`.

- It decodes OTLP/HTTP (protobuf and JSON) with `opentelemetry-proto`,
  answers CORS for any origin, adds `thunderforge.*` source labels, drops
  spam by reason, and forwards protobuf to the collector's `otlp/public`
  receiver on `:4319`.
- The allow-lists, caps, checks and tier rules move into
  `crates/thunderforge-telemetry-policy`, which the server's anonymous tier
  and the gateway both use.
- It holds the client IP only in memory, hashed, to rate-limit, and never
  logs or forwards it. It sheds load with fast 503s.
- It ships as `mbround18/thunderforgevtt:telemetry-gateway`. Its
  Deployment and the route change are the owner's, in the Flux repository.

**Disclosure.** It goes in all five places Principle VII names, with
Appendix A's words. A test compares the strings.

**Tests.** They run with `TELEMETRY=false` everywhere. The telemetry tests use
in-memory exporters or Playwright routing, so nothing leaves the machine.

**Cluster.** Dashboards, alert rules and the landing's exporter are applied
by a new `make observability` target, because `thunderforge-dev` is not
Flux-managed.

- Browser alerts come from a collector `count` connector, which is one snippet
  for the Flux repository.
- The public route's CORS allows only the project's two origins (R7). The
  owner chose the gateway over widening it (R25), so self-hosted browsers
  can report once the route points at the gateway.

Planning corrected the spec in six places. Each is in research.md and is now
in spec.md:

- the instance id's key is `system.telemetry_instance_id` (R23);
- the demo's `<meta>` keeps a widened `connect-src` (R24);
- the CORS assumption was false (R7), and a proxy now answers it (R25);
- browser alerts use a `count` connector, not the Loki ruler (R8);
- web e2e uses routing, not a `TELEMETRY=true` stack (R15);
- the package's unit tests run on `node --test` (R11).

## Technical Context

**Language/Version**: Rust, latest stable, edition 2024 (`apps/thunderforge`,
`apps/telemetry-gateway`, `crates/thunderforge-server`,
`crates/thunderforge-pg`, `crates/thunderforge-telemetry-policy`). TypeScript 6 and
React 19 for `packages/telemetry`, `apps/web`, `apps/demo` and
`apps/landing`. nginx (`stable-alpine`) for the landing image.

**Primary Dependencies**: New ones only. The versions and reasons are in
research R1, R2, R10 and R26.

| Dependency | Version | Crate or package | Why |
| --- | --- | --- | --- |
| `opentelemetry` | 0.33.0 | thunderforge-server, thunderforge, thunderforge-telemetry-gateway | the API: no-op instruments until the app installs an SDK |
| `opentelemetry_sdk` | 0.33.0 (+`testing` in dev) | thunderforge, thunderforge-telemetry-gateway | providers, batch processors, the allow-list processor, in-memory exporters |
| `opentelemetry-otlp` | 0.33.0, `http-proto` + `reqwest-client` + `reqwest-rustls` + `trace,metrics,logs`, no defaults | thunderforge, thunderforge-telemetry-gateway | OTLP/HTTP on the workspace's reqwest 0.13 |
| `opentelemetry-proto` | 0.33.0, `gen-tonic-messages` + `trace,logs,metrics` + `with-serde`, no defaults | thunderforge-telemetry-gateway | the OTLP request and response messages, decoded from protobuf and JSON (R26); already in the tree through `opentelemetry-otlp` |
| `prost` | 0.14 | thunderforge-telemetry-gateway | protobuf decode and encode; the version `opentelemetry-proto` 0.33 uses |
| `serde_json` | existing workspace version | thunderforge-telemetry-gateway | OTLP/JSON decode |
| `axum`, `tower`, `tower-http` (`cors`, `limit`, `trace`), `tokio`, `reqwest`, `clap` (`derive`, `env`) | existing workspace versions | thunderforge-telemetry-gateway | the service, its layers, the upstream client and its flags |
| `regex` (dev) | existing workspace version | thunderforge-telemetry-policy | the `PUBLIC_METRIC_NAME_FILTER` test |
| `tracing-opentelemetry` | 0.34.0 | thunderforge | the layer beside Bunyan |
| `opentelemetry-appender-tracing` | 0.33.0 | thunderforge | the log bridge, on the operator tier only |
| `uuid` feature `v4` | 1.26.0 (existing) | thunderforge-server | a random instance id; v7 embeds a time |
| `web-vitals` | 6.2.3, Apache-2.0 | packages/telemetry | LCP, INP, CLS, FCP and TTFB, about 2 KB brotli |
| `nginx/nginx-prometheus-exporter` | newest 1.x image | the cluster sidecar | nginx's `stub_status` as Prometheus series |

Rejected: the OTel JS SDK (bundle size, R10), `grpc-tonic`, `gzip-http` and
the semantic-conventions crate (R1), Vitest for the package (R11),
`opentelemetry-proto`'s default `full` feature (tonic, R26), a user-agent
parsing crate and a GeoIP database (R29, R30).

**Storage**: One row in the existing `instance_settings`. There is no
migration. The gateway keeps its rate buckets in memory only.

**Testing**:

- **Rust**, with in-memory exporters (`opentelemetry_sdk` `testing`):
  - `cargo test -p thunderforge-telemetry-policy`, written first: the
    lists, `tier_for`, caps, the instance id check, the labels, the token
    bucket under a fixed clock, and `print_instruments`;
  - `cargo test -p thunderforge-telemetry-gateway`: the router in process
    against a fake upstream (SC-014);
  - `cargo test -p thunderforge`: settings, the three states, the
    startup line and the SC-011 leak test;
  - `cargo test -p thunderforge-server`: served config, instance id,
    instruments equal to the atomics, the GraphQL extension, the pool gauges,
    the event-name table, and the `demo_router` header.
- **`packages/telemetry`**, with `node --test`: the allow-list, redaction
  through the port, caps and folding, sampling, GPC/DNT, the 60 KB split, the
  dead-endpoint queue, and the OTLP/JSON encoding.
- **e2e**, the new `telemetry` slice, `pnpm e2e:telemetry`:
  - standalone: the demo e2e (every spec, because `support.ts` changes) and a
    new landing e2e;
  - integration: `apps/web/e2e/telemetry-*.spec.ts`.
- **Offline**: `make observability-check` (kustomize, dashboards parse, the
  SC-008 script, `promtool`) and `scripts/check-landing-nginx.sh`.

**Target Platform**: the self-hosted server image (Linux), browsers, the
landing image (nginx), and the owner's k3s cluster (`thunderforge-dev` and
`monitoring`).

**Project Type**: a web service, a telemetry gateway service, three web
apps, one new shared browser package and one new policy crate.

**Performance Goals**:

- The browser chunk is at most 25 KB brotli and loads after `load` (SC-007).
- No server request waits on export (FR-003).
- Observable instruments are read once per export interval (60 s), so the
  delivery and subscription paths are unchanged (FR-010).
- The gateway answers a refused request without touching the upstream, and
  a full gateway answers `503` at once (FR-044).

**Constraints**:

- Rust files stay at 1000 lines or fewer (`main.rs` is at 865), so all new
  code is in `telemetry/` modules.
- `make lint` runs on the host and wasm32.
- No client-side database.
- Components never call telemetry for world state (AGENTS.md §2).
- No built bundle names an endpoint.
- Every test and e2e stack runs with `TELEMETRY=false`.
- E2E gates on slices only, never the full suite.
- Spec 083 is being implemented in the same tree. 086 edits none of its
  files: the demo's funnel hooks go in `record()` in
  `apps/demo/src/backend/events.ts`, not in `handlers/dice.ts` or
  `actors.ts`. T046 changes `apps/demo/e2e/support.ts`, which 083's
  `rolls-across-tabs.spec.ts` uses, so it waits until 083's demo changes
  have landed.

**Scale/Scope**: about 58 new files and about 33 edited. There are 9 user
stories (5 at P1), 7 dashboards, 15 alert rules, 23 server instruments and
4 gateway instruments.

## Constitution Check

| Principle | How this plan meets it |
| --- | --- |
| I. ECS owns simulation, React owns chrome | The engine is unchanged. `engine.load` and `engine.frames` read `mountEngine`'s stage callbacks and `stats.ts`'s mirror. No system calls telemetry. |
| II. Plugin-modular engine | No engine plugin changes. |
| III. Ownership and authorisation at the data boundary | `telemetryStatus` is an admin-only query. The served config holds nothing private: the instance id is anonymous by construction (FR-007). |
| IV. ADRs and specs before divergent implementation | ADR-114 records the default. This plan corrects the spec in place (R7, R8, R11, R15, R23, R24) before any code. The gateway amendment (R25) changed the spec, the disclosure and the tasks together, before any of its code. |
| V. Verify before claiming done | Each phase ends with a named proof command. The last code phase runs every gate. |
| VI. Every feature is proven by its own slice | There is a new `telemetry` slice. The gateway has no browser surface, so it is proven by its in-process integration tests (SC-014), not by a slice. The neighbours are `feedback`, `resumable-downloads`, `rolls` and `worlds`, plus whatever `e2e:which --diff` names. `world_events.rs` is cross-cutting (R21), and the named slices answer it; the full suite is not run, by the owner's rule. |
| VII. Telemetry is on, anonymous, and the operator's to redirect | `TELEMETRY=true` is the default in both images. `tier_for` is the one decision. The anonymous allow-lists are constants with enumerating tests (FR-009), in one crate that the gateway enforces again. The disclosure says what the gateway adds and that the IP is used only in memory. There is one variable to redirect and one to turn off, with no rebuild. The disclosure is in all five places, with a string test. Tests run with `TELEMETRY=false`. |
| Agnostic core plus adapters (CLAUDE.md §3) | `thunderforge-server` sees only the API crate. The SDK is in the app. `thunderforge-telemetry-policy` is the pure core the server and the gateway adapt to their own types; `apps/telemetry-gateway` is a thin binary. `packages/telemetry` has a core with `TelemetrySink` and `Redactor` ports, an OTLP adapter and a no-op sink. The apps stay thin. |
| Runtime feature flags (CLAUDE.md §1) | `TELEMETRY` and the endpoints are read at start, and the browser reads the served file. The gateway's rates, hops and upstream are flags with environment fallbacks. There is no compile-time switch. |
| Load shedding (AGENTS.md §3) | The gateway drops, never queues: `load_shed` over a concurrency limit, and a non-blocking upstream semaphore (R31). |

There are no violations.

## Project Structure

### Documentation (this feature)

```text
specs/086-full-telemetry/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R31
├── data-model.md        # the instance id row, settings, browser session
├── quickstart.md
├── contracts/
│   ├── served-config-and-csp.md     # telemetry.json, tier table, connect-src, nginx maps, telemetryStatus
│   ├── server-instruments.md        # 23 instruments, spans, the policy crate's allow-lists, server.error
│   ├── browser-events.md            # package surface, events, limits, session, call sites
│   ├── collector-count-connector.md # Flux snippet, browser series, two alerts
│   ├── observability-apply.md       # deploy/k8s/observability, make observability, SC-008 script
│   └── telemetry-gateway.md         # the gateway's surface, labels, drops, policy crate, image, Flux manifests
└── tasks.md
```

### Source Code (repository root)

```text
apps/thunderforge/
├── Cargo.toml                         # otel 0.33 SDK, otlp, tracing-opentelemetry 0.34, appender
└── src/
    ├── main.rs                        # registry (l.350), pool event handler (l.398), schema extension (l.559), TraceLayer (l.767), shutdown flush
    └── telemetry/                     # NEW
        ├── mod.rs
        ├── tier.rs                    # re-exports the policy crate's tier and lists; SDK glue only
        ├── settings.rs                # TelemetrySettings from the environment
        ├── install.rs                 # providers, views, layers, shutdown guard
        ├── anonymous.rs               # AllowListSpanProcessor, ServerErrorLayer
        ├── http.rs                    # TraceLayer make_span/on_request/on_response, trace_id fields
        ├── startup_line.rs            # Appendix A.3
        ├── disclosure_tests.rs        # Appendix A against README, guide, landing (#[cfg(test)])
        └── tests.rs                   # three states, SC-011 leak test (#[cfg(test)])

crates/thunderforge-telemetry-policy/  # NEW: pure, std only
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── tier.rs                        # PROJECT_TELEMETRY_ENDPOINT, Tier, tier_for (moved)
    ├── lists.rs                       # attribute allow-lists, SERVICE_NAMES, INSTRUMENTS, GATEWAY_INSTRUMENTS, print_instruments
    ├── caps.rs                        # value caps, is_instance_id
    ├── labels.rs                      # Source, source_for, reduce_user_agent, country, client_version, DropReason
    └── rate.rs                        # TokenBucket with the clock passed in

apps/telemetry-gateway/                # NEW: package thunderforge-telemetry-gateway
├── Cargo.toml                         # opentelemetry-proto 0.33, prost 0.14, axum, tower, tower-http, reqwest, clap, otel SDK
├── tests/fixtures/                    # OTLP protobuf and JSON bodies, the JSON captured from packages/telemetry
└── src/
    ├── main.rs                        # clap + env, own telemetry, serve
    ├── router.rs                      # paths, CORS, ServiceBuilder (load_shed, concurrency, timeout, body limit)
    ├── intake.rs                      # decode, policy, labels, partial_success
    ├── limits.rs                      # IP and instance bucket maps, client IP from X-Forwarded-For
    ├── upstream.rs                    # bounded reqwest client, semaphore, re-encode
    ├── metrics.rs                     # the four gateway instruments
    └── tests.rs                       # fake upstream, SC-014 (#[cfg(test)])

crates/thunderforge-server/
├── Cargo.toml                         # opentelemetry 0.33 (API); uuid +v4; thunderforge-telemetry-policy
└── src/
    ├── telemetry/                     # NEW (API only)
    │   ├── mod.rs
    │   ├── instruments.rs             # observable backplane/subscription/channel instruments, DELIVERY OnceLock
    │   ├── event_names.rs             # EVENT_CODE_* → name, scan test
    │   ├── graphql_extension.rs       # span + histogram + error counter, root-field set
    │   ├── pool_events.rs             # r2d2 HandleEvent, pool gauges
    │   ├── instance_id.rs             # system.telemetry_instance_id
    │   ├── served_config.rs           # BrowserTelemetry, served_json, connect_src, /telemetry.json handler
    │   └── status.rs                  # TelemetryStatus
    ├── lib.rs                         # pub mod telemetry
    ├── state.rs                       # AppState.telemetry: Arc<TelemetryStatus>
    ├── world_events.rs                # two counter calls in record_world_event (cross-cutting, R21)
    ├── network/listener.rs            # DELIVERY registration, WORLD_CHANNELS_REAPED
    ├── static_files/mod.rs            # demo_router: /demo/telemetry.json + connect-src header
    └── graphql/queries/admin.rs       # telemetryStatus

packages/telemetry/                    # NEW
├── package.json                       # @thunderforge/telemetry; web-vitals 6.2.3
├── tsconfig.json
└── src/
    ├── index.ts  config.ts  session.ts  allowList.ts  queue.ts  telemetry.ts  privacy.ts  ua.ts
    ├── otlp/index.ts  otlp/encode.ts
    ├── browser.ts                     # DOM collectors (vitals, long tasks, nav, errors, pagehide)
    ├── boot.ts                        # config fetch, load wait, dynamic import
    ├── vite.ts                        # servedTelemetry(), connectSrcFor()
    └── *.test.ts                      # node --test

apps/web/src/
├── main.tsx                           # bootTelemetry after startLogCapture
├── App.tsx                            # AppErrorBoundary around the routes
├── components/AppErrorBoundary.tsx    # NEW
├── telemetry/index.ts                 # NEW: the app's chunk: createTelemetry + otlpHttpSink + redact + collectors
├── telemetry/routes.ts                # NEW: route template page views
├── api/graphqlClient.ts               # traceparent beside withCsrf (l.318)
├── engine/bevy/index.ts               # stage hooks for loadTelemetry (resumable-downloads owns it)
├── engine/bevy/loadTelemetry.ts       # NEW
├── engine/bevy/framesSummary.ts       # NEW
├── pages/admin/SettingsPage.tsx       # renders TelemetryPanel
└── pages/admin/TelemetryPanel.tsx     # NEW: Appendix A.4

apps/demo/
├── vite.config.mts                    # sealedPage connect-src widened; servedTelemetry(); preview.headers
├── index.html                         # meta description (l.10)
├── src/telemetry.ts                   # NEW: boot, demo_opened, chrome hooks
├── src/guard/install.ts               # pass POST to <origin>/v1/{logs,traces} when enabled
├── src/DemoNotice.tsx                 # disclosure line, "Run your own"
├── src/main.tsx                       # boots src/telemetry.ts
├── src/backend/events.ts              # record() calls the tap
├── src/backend/telemetryTap.ts        # NEW: token_moved, dice_rolled, demo.action
├── src/backend/notInDemo.ts           # demo.not_in_demo
└── e2e/support.ts, e2e/demo.spec.ts, e2e/telemetry.spec.ts (NEW)

apps/landing/
├── vite.config.mts                    # servedTelemetry(); feedback-redaction alias
├── nginx.conf.template                # maps, telemetry.json, connect-src, JSON log, stub_status
├── package.json                       # e2e script
├── playwright.config.ts               # NEW
├── e2e/telemetry.spec.ts              # NEW
├── src/telemetry.ts                   # NEW
├── src/main.tsx, src/App.tsx          # boot; render WhatWeMeasure
├── src/sections/WhatWeMeasure.tsx     # NEW (#telemetry)
├── scripts/check-telemetry-size.mjs   # NEW: SC-007, run by build
└── src/Nav.tsx, src/sections/{Hero,MapLegend,Support,Footer}.tsx  # data-cta attributes, #telemetry link

Cargo.toml                             # workspace members: apps/telemetry-gateway, crates/thunderforge-telemetry-policy
Dockerfile                             # landing + server stages: TELEMETRY env defaults; telemetry-gateway stage
.cargo/config.toml                     # [env] TELEMETRY = "false"
apps/web/playwright.config.ts, scripts/{e2e-parallel,journeys,torture}.mjs   # TELEMETRY: "false"
scripts/e2e/slices.json, package.json   # telemetry slice, e2e:telemetry
scripts/check-observability.mjs        # NEW (SC-008)
scripts/check-landing-nginx.sh         # NEW (R16)
Makefile                               # observability, observability-check, telemetry-gateway-image, push-telemetry-gateway
deploy/k8s/observability/              # NEW: see contracts/observability-apply.md
README.md, docs/guides/telemetry.md (NEW), docs/CONTRIBUTING.md
specs/074-a-world-to-try/spec.md       # the amendment (FR-032)
```

**Structure Decision**: Telemetry is a cross-cutting port, so each side has
one home for it:

- `crates/thunderforge-server/src/telemetry/` for what the server *measures*,
  which is API only;
- `apps/thunderforge/src/telemetry/` for where it *goes*, which is the SDK;
- `crates/thunderforge-telemetry-policy` for what may leave and what the
  public route accepts, read by the server and the gateway alike;
- `apps/telemetry-gateway` for the public intake;
- `packages/telemetry` for every browser.

The apps add only call sites:

- `main.rs` gains about 15 lines;
- each browser app gains one `telemetry.ts` and its hooks at the existing
  seams.

`deploy/k8s/observability/` is new, and it is the repository's first
`deploy/` directory.

## Complexity Tracking

| Choice | Why it is needed | Simpler alternative rejected because |
| --- | --- | --- |
| A hand-written OTLP/JSON encoder | SC-007's 25 KB brotli | The OTel JS SDK is several times that before logs |
| A span processor that filters attributes | The collector does not filter span attributes (R6), and Principle VII needs an allow-list in code | Trusting each call site to record only safe fields cannot be tested as one list |
| nginx `map` chains for `telemetry.json` | The landing image must switch without a rebuild and without writing to its root filesystem | An entrypoint script that writes the file means a writable root filesystem and a second code path |
| `make observability` with `kubectl patch` | The landing Deployment's base is in no repository | Kustomize or Flux cannot patch a base they do not hold |
| A separate gateway service and a policy crate | Source labels, per-IP and per-instance limits, and content checks on the public route (R25) | Widening the collector's CORS fixes preflights only; the collector can neither label the source nor rate-limit per sender |
