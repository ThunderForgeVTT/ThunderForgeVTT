# Tasks: Full Telemetry

**Input**: Design documents from `specs/086-full-telemetry/`
**Prerequisites**: plan.md, spec.md, research.md (R1–R31), data-model.md,
contracts/served-config-and-csp.md, contracts/server-instruments.md,
contracts/browser-events.md, contracts/collector-count-connector.md,
contracts/observability-apply.md, contracts/telemetry-gateway.md,
quickstart.md

**083 status (T001, 2026-10-09):** `git status --short apps/demo` and `git diff --cached --name-only` are clean; spec 083 is merged, so the [083] tasks may start.

**Spec 083 is being implemented in this tree.** 086 edits none of 083's
files. The demo's funnel hooks go in `record()` in
`apps/demo/src/backend/events.ts`, not in `handlers/dice.ts` or `actors.ts`.
Tasks marked **[083]** change `apps/demo/e2e/support.ts`, which every demo
spec uses, including 083's `rolls-across-tabs.spec.ts`. They start only when
`git status --short apps/demo` shows no 083 change, so the demo e2e run that
proves them proves 083's spec too.

**Tests**: Tests come first in every phase, and must fail before the code
that passes them. Every test and e2e stack runs with `TELEMETRY=false`
(FR-036). A telemetry test turns telemetry on for itself, with in-memory
exporters (Rust) or `page.route` (Playwright). Nothing leaves the machine.

**E2E**: Proven by slices only. The gate is `pnpm e2e:telemetry`, plus each
slice that `pnpm e2e:which --diff` names. That command prints FULL SUITE,
because `world_events.rs` and `apps/thunderforge/schema.graphql` are
cross-cutting (R21). The named slices answer it. No task runs the full suite.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel: a different file, with no dependency on an
  unfinished task.
- **[Story]**:
  - US1: demo drop-off;
  - US2: visitors' errors;
  - US3: server health and alerts;
  - US4: landing traffic;
  - US5: visitors told, nothing typed leaves;
  - US6: anonymous by default, redirect, off;
  - US7: engine load and slow actions traced;
  - US8: the operator is told;
  - US9: the owner's endpoint labels the source and drops spam.

---

## Phase 1: Setup

- [X] T001 Confirm spec 083's state with `git status --short apps/demo` and `git diff --cached --name-only`. Record in this file's header which 083 files are still open. Until they are clean, the [083] tasks wait, and nothing else here touches `apps/demo/e2e/`.
- [X] T002 [P] Create `packages/telemetry/package.json` and `packages/telemetry/tsconfig.json`, modelled on `packages/downloads`.
  - `package.json`: name `@thunderforge/telemetry`, `"type": "module"`, exports `.`, `./otlp`, `./browser` and `./vite`, `"test": "node --test src/*.test.ts src/**/*.test.ts"`, dependency `web-vitals` `6.2.3` (R10, R11).
  - Run `pnpm install`, and confirm that `pnpm-workspace.yaml` already covers `packages/*`.
- [X] T003 [P] Add the Rust dependencies (R1, R2), each crate declaring its own, as the workspace does today:
  - create `crates/thunderforge-telemetry-policy` (an empty `lib.rs`, no dependencies, `regex` under `[dev-dependencies]`) and add it to the root `Cargo.toml`'s `members` (R25);
  - `crates/thunderforge-server/Cargo.toml`: `opentelemetry = "0.33.0"`, `thunderforge-telemetry-policy` by path, and add `v4` to `uuid`'s features;
  - `apps/thunderforge/Cargo.toml`: `opentelemetry = "0.33.0"`, `opentelemetry_sdk = "0.33.0"` (no `rt-tokio`, R1), `opentelemetry-otlp = { version = "0.33.0", default-features = false, features = ["http-proto", "reqwest-client", "reqwest-rustls", "trace", "metrics", "logs"] }`, `tracing-opentelemetry = "0.34.0"`, `opentelemetry-appender-tracing = "0.33.0"`, `thunderforge-telemetry-policy` by path, and `opentelemetry_sdk` with `testing` under `[dev-dependencies]`.
  - Check with `cargo tree -d -p thunderforge`: there must be no second `reqwest` or `tracing-subscriber`.
- [X] T004 Add the `telemetry` slice.
  - `scripts/e2e/slices.json`: `own: ["telemetry"]`. The paths are `packages/telemetry/**`, `apps/thunderforge/src/telemetry/**`, `crates/thunderforge-server/src/telemetry/**`, `apps/web/src/telemetry/**`, `apps/web/src/components/AppErrorBoundary.tsx`, `apps/web/src/engine/bevy/{loadTelemetry,framesSummary}.ts`, `apps/demo/src/telemetry.ts`, `apps/demo/src/backend/telemetryTap.ts`, `apps/landing/src/telemetry.ts`, `apps/landing/e2e/**`, `apps/landing/nginx.conf.template`, `deploy/k8s/observability/**`, and `scripts/check-{observability.mjs,landing-nginx.sh}`. The standalone command is `pnpm -F @thunderforge/telemetry test && pnpm -F @thunderforge/demo e2e && pnpm -F @thunderforge/landing e2e`.
  - Root `package.json`: `e2e:telemetry` and `e2e:telemetry:integration` (`node ./scripts/e2e-parallel.mjs --shards=1 --slice=telemetry`), next to `e2e:feedback`.
  - Verify that `pnpm e2e:which packages/telemetry/src/index.ts` names `telemetry`.

---

## Phase 2: Foundational (blocks every story)

**Purpose**: Telemetry off in every test stack, the browser package's core,
the server crate's API-only module, the tier, the settings and the instance id.

### Telemetry off in every test stack (FR-036, SC-013)

- [X] T005 [P] Add `TELEMETRY = "false"` to `[env]` in `.cargo/config.toml`, with a comment that `[env]` never overrides a variable already set, so a test that wants telemetry sets it itself.
- [X] T006 [P] Add `TELEMETRY: "false"` next to `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` in each of these:
  - `apps/web/playwright.config.ts:154`;
  - `scripts/e2e-parallel.mjs:631`;
  - `scripts/journeys.mjs:530`;
  - `scripts/torture.mjs:286`.

### `packages/telemetry`: the core, the OTLP adapter and the Vite plugin (FR-015 to FR-020)

- [X] T007 [P] Write failing `node --test` tests in `packages/telemetry/src/`.
  - `config.test.ts`: `parseConfig` gives off for a non-200 answer, an unparseable body, a missing `enabled`, an endpoint that is not `http(s)`, and `{"enabled":false}`. It strips a trailing slash.
  - `allowList.test.ts`: enumerates `ALLOWED_ATTRIBUTES` and `EVENT_NAMES` exactly, and an unknown attribute is dropped before it is queued.
  - `privacy.test.ts`: GPC or DNT limits the session to `error`.
  - `session.test.ts`: the `sessionStorage` key is `thunderforge.telemetry`, `sampled` is decided once, and `funnel(step)` sends each step once.
- [X] T008 [P] Write failing tests in `packages/telemetry/src/`.
  - `queue.test.ts`: the queue holds 200 and drops the oldest. The caps are 50 errors and 2,000 events, and a repeated error is folded. A dead endpoint keeps the queue bounded.
  - `errors.test.ts`: the `Redactor` port is applied, `error.message` is cut at 512 and `error.stack` at 4096, and frames are reduced to path and line with no query string.
  - `otlp/encode.test.ts`: the OTLP/JSON shape of a log and of a span, the resource attributes of FR-019 and FR-019a for each tier, and a 60 KB split.
- [X] T009 Implement the core so those tests pass:
  - `config.ts`, `session.ts`, `allowList.ts`, `privacy.ts`, `ua.ts` (the browser family and major version, the OS family, the mobile flag, and the viewport, memory and core-count buckets);
  - `queue.ts`;
  - `telemetry.ts`, with `createTelemetry`, `noopTelemetry`, and the `TelemetrySink` and `Redactor` ports, as `contracts/browser-events.md` gives them.

  None of these imports a DOM global at module scope, so that `node --test` loads them.
- [X] T010 Implement `packages/telemetry/src/otlp/encode.ts` and `otlp/index.ts` (`otlpHttpSink`). It posts `/v1/logs` and `/v1/traces` with `credentials: "omit"` and `keepalive`, and flushes every 5 s, on `visibilitychange` to hidden, and on `pagehide` (FR-020).
- [X] T011 Implement `packages/telemetry/src/browser.ts` (`startCollectors`) and `boot.ts` (`bootTelemetry`).
  - `startCollectors` covers `error`, `unhandledrejection`, page views, navigation timing, and the flush hooks. Web vitals (`web-vitals`) and long tasks run only in sampled sessions.
  - `bootTelemetry` fetches the config before `load` (`credentials: "omit"`, `cache: "no-store"`), decides GPC/DNT, waits for `load`, and only then calls the app's `import()` loader.
  - `src/index.ts` exports the public surface of `contracts/browser-events.md`.
- [X] T012 [P] Write `packages/telemetry/src/vite.test.ts`, then implement `src/vite.ts`.
  - `servedTelemetry()` serves `telemetry.json` (and `/demo/telemetry.json`) in dev and preview from `THUNDERFORGE_PREVIEW_TELEMETRY`, and `{"enabled":false}` when that is unset.
  - `connectSrcFor(config)` gives the three rows of the contract's connect-src table, and the plugin sets `preview.headers` and `server.headers` from it (R14).

### Server crate: API only (FR-005, FR-007, FR-026)

- [X] T013 [P] Write failing tests in `crates/thunderforge-server/src/telemetry/instance_id.rs` (`#[cfg(test)]`, against the `thunderforge_test` database, as the `settings/` tests do):
  - the id is created once and parses as a UUIDv4;
  - a second call returns the same value;
  - a value already present is not touched, even one that is not a UUID;
  - two concurrent calls return one value;
  - the row is invisible to `settings::resolver`'s unrecognised-rows report.
- [X] T014 Implement `crates/thunderforge-server/src/telemetry/instance_id.rs` (`ensure_instance_id`): `INSERT ... ON CONFLICT (key) DO NOTHING`, then `SELECT`, with key `system.telemetry_instance_id` (R23). Add `pub mod telemetry` with `telemetry/mod.rs` to `crates/thunderforge-server/src/lib.rs`. Call it in `apps/thunderforge/src/main.rs` after migrations, whatever `TELEMETRY` is.
- [X] T015 [P] Write failing table tests in `crates/thunderforge-server/src/telemetry/served_config.rs` for `BrowserTelemetry::served_json()` and `connect_src()`. They cover every row of `contracts/served-config-and-csp.md`'s field and connect-src tables, including off with no other keys and a sample rate clamped to 0..1.
- [X] T016 Implement the following:
  - a re-export of the policy crate's `Tier` (no decision logic) in `crates/thunderforge-server/src/telemetry/mod.rs`;
  - `BrowserTelemetry` in `served_config.rs`;
  - `TelemetryStatus` in `status.rs`;
  - `AppState.telemetry: Arc<TelemetryStatus>` in `crates/thunderforge-server/src/state.rs`.

  Update every `AppState` constructor in tests with a `TelemetryStatus::off_for_tests()`.

### The policy crate and the app: tier and settings (FR-002, FR-006, FR-009, FR-038)

- [X] T017 [P] Write failing tests in `crates/thunderforge-telemetry-policy/src/{tier,lists}.rs`.
  - `tier_for` runs over the normalisation table in `contracts/served-config-and-csp.md`.
  - `ANONYMOUS_SPAN_ATTRIBUTES`, `ANONYMOUS_RESOURCE_ATTRIBUTES`, `SERVER_ERROR_ATTRIBUTES` and `INSTRUMENTS` are each enumerated exactly.
  - Every `INSTRUMENTS` name matches `PUBLIC_METRIC_NAME_FILTER` (`^(thunderforge\.|http\.server\.|db\.client\.)`).
- [X] T018 Implement `crates/thunderforge-telemetry-policy/src/{lib,tier,lists}.rs` as `contracts/server-instruments.md` and `contracts/telemetry-gateway.md` give them, with `print_instruments`, the `#[test]` that prints each instrument's Prometheus name for `scripts/check-observability.mjs`. Then add `apps/thunderforge/src/telemetry/{mod,tier}.rs`, where `tier.rs` only re-exports the crate's tier and lists and adapts them to the SDK's types. Add `mod telemetry;` to `apps/thunderforge/src/main.rs`. No list is written twice.
- [X] T019 [P] Implement `apps/thunderforge/src/telemetry/settings.rs` (`TelemetrySettings::from_env`) with table tests:
  - `TELEMETRY` false, 0, no or off in any case;
  - `OTEL_SDK_DISABLED`;
  - the general and per-signal endpoints, where any per-signal endpoint that is not the project's makes every signal operator (data-model.md);
  - `THUNDERFORGE_BROWSER_TELEMETRY_*` into `BrowserTelemetry`.

  Tests pass an environment map, never `std::env::set_var`.

**Checkpoint**: `pnpm -F @thunderforge/telemetry test`, `cargo test -p thunderforge-telemetry-policy`, `cargo test -p thunderforge-server telemetry` and `cargo test -p thunderforge telemetry` pass. `TELEMETRY=false` is in every stack.

---

## Phase 3: User Story 6 — Anonymous by default, redirected, or off (P1) 🎯 MVP

**Goal**: A server with no telemetry variable exports only the anonymous tier
to the project. One variable redirects it with full detail, and one turns it
off. Browsers follow the served `telemetry.json`. No rebuild is needed for
any of these.

**Independent Test**:

- `cargo test -p thunderforge telemetry` proves the three states and the
  SC-011 leak test, and `cargo test -p thunderforge-server static_files`
  proves the served file and the header.
- `pnpm e2e:telemetry` runs `apps/web/e2e/telemetry-off.spec.ts` and
  `telemetry-redirected.spec.ts`.

- [X] T020 [P] [US6] Write failing tests in `apps/thunderforge/src/telemetry/tests.rs` (`#[cfg(test)]`), with in-memory span, metric and log exporters injected into `install`. They cover SC-001:
  - `TELEMETRY=false` installs no provider and builds no exporter;
  - with no variable, every exporter targets `PROJECT_TELEMETRY_ENDPOINT`, there is no log bridge, the resource is exactly `ANONYMOUS_RESOURCE_ATTRIBUTES`, and the sampler is `parentbased_traceidratio` 0.1;
  - with `OTEL_EXPORTER_OTLP_ENDPOINT` set, that endpoint only, with the log bridge and the standard detectors;
  - `OTEL_SDK_DISABLED=true` installs nothing, while `/telemetry.json` is unchanged.
- [X] T021 [P] [US6] Write the SC-011 leak test in `apps/thunderforge/src/telemetry/tests.rs`.
  - Seed admin `canary-7f3a@example.org`, and a world, actor, scene and token named `zq-canary-7f3a`.
  - Send a chat line and a mutation carrying the canary, and log an `ERROR` whose message holds the email.
  - On the anonymous tier, assert that no exported record holds the email, the canary, the hostname or any seeded id, and that every span and resource attribute is on the policy crate's lists.
  - On the operator tier, assert that `world.id` (or the existing `world_id` field) is present.
  - *Done as a pipeline test (2026-10-09):* the canary request is played through `tracing` (the HTTP and GraphQL spans, a chat line, and the `ERROR`) into the installed providers, rather than through seeded rows. The server's GraphQL span (T042) does not exist yet; when it does, this test is where a seeded run belongs.
- [X] T022 [US6] Implement `apps/thunderforge/src/telemetry/anonymous.rs`.
  - `AllowListSpanProcessor` keeps the span name, status, duration and `ANONYMOUS_SPAN_ATTRIBUTES`. It drops other attributes, links, and every event except the redacted `exception`.
  - `ServerErrorLayer` turns each `ERROR` event into a `server.error` log record with `error.type`, `error.message` (redacted, 512 characters) and a backtrace reduced to crate paths and lines (4096 characters). It redacts through `crates/thunderforge-server/src/feedback/redaction.rs`'s rule set from `config/feedback-redaction.json`.
- [X] T023 [US6] Implement `apps/thunderforge/src/telemetry/install.rs`.
  - It builds the tracer, meter and logger providers per tier, with batch processors and bounded queues, and never blocks start (FR-003). The anonymous resource is built by hand, and the operator tier reads the standard `OTEL_*` variables.
  - It returns a guard that flushes on shutdown.
  - In `apps/thunderforge/src/main.rs`, wire it into the registry at line 350, beside `JsonStorageLayer` and `BunyanFormattingLayer`, and drop the guard after the server's graceful shutdown.
- [X] T024 [US6] Serve the config.
  - Add the `/telemetry.json` handler (`200`, `application/json`, `Cache-Control: no-store`) in `crates/thunderforge-server/src/telemetry/served_config.rs`.
  - Mount it at `/telemetry.json` and `/demo/telemetry.json`.
  - In `static_files::demo_router` (`crates/thunderforge-server/src/static_files/mod.rs:104`), add a `SetResponseHeaderLayer` with `connect_src()`, built once.
  - Test with router `oneshot` for the default, redirected and off states (R15).
  - In `apps/thunderforge/src/main.rs`, build `BrowserTelemetry` from `TelemetrySettings` and `tier_for`, so that `tier_for` stays the only decision.
- [X] T025 [US6] Boot telemetry in the web app.
  - `apps/web/src/telemetry/index.ts` is the app's lazy chunk: `createTelemetry`, `otlpHttpSink`, `redact` from `apps/web/src/services/feedbackRedaction.ts`, and `startCollectors`.
  - `apps/web/src/telemetry/routes.ts` reports page views by route template, never by path.
  - `apps/web/src/main.tsx` calls `bootTelemetry` after `startLogCapture`, with `() => import("./telemetry")`.
  - Add `apps/web/src/telemetry/__tests__/routes.test.ts`.
- [X] T026 [P] [US6] Write `apps/web/e2e/telemetry-off.spec.ts`. On the ordinary stack (`TELEMETRY=false`), `/telemetry.json` is `{"enabled":false}`, no `telemetry` chunk is requested, and no request leaves the instance's origin.
- [X] T027 [P] [US6] Write `apps/web/e2e/telemetry-redirected.spec.ts`.
  - Route `**/telemetry.json` to an enabled operator config naming `https://otel.example.org`, and answer `https://otel.example.org/v1/*` with `204`.
  - Assert that a page view arrives there with `thunderforge.tier=operator` and the instance id, and that nothing goes to `telemetry.thunderforge.dev`.
  - A second test routes an anonymous config naming `https://telemetry.invalid`, and asserts that `deployment.environment=self-hosted` ignores the config's `environment` (FR-019a).

**Checkpoint**: The three states, the leak test, and both web specs pass.
`pnpm e2e:telemetry` is green.

---

## Phase 4: User Story 8 — An operator is told, wherever they look (P1)

**Goal**: The disclosure is in the startup line, the admin panel, the
README and the guide, with Appendix A's words.

**Independent Test**:

- `cargo test -p thunderforge startup_line` and the disclosure string test
  pass.
- `pnpm -F @thunderforge/web test TelemetryPanel` passes.
- `apps/web/e2e/telemetry-admin-panel.spec.ts` passes under
  `pnpm e2e:telemetry`.

- [X] T028 [P] [US8] Write failing tests in `apps/thunderforge/src/telemetry/startup_line.rs`. They check Appendix A.3's text for each state: on, redirected, half redirected (server only, then browsers only), and off. They also check that `main` logs it exactly once (SC-012).
- [X] T029 [US8] Implement `startup_line.rs`, and log the line at `INFO` once in `apps/thunderforge/src/main.rs`, after `install`.
- [X] T030 [US8] Add `telemetry_status` to `AdminQuery` in `crates/thunderforge-server/src/graphql/queries/admin.rs`, returning `TelemetryStatus` as the contract gives it (wire tier `full`, not `operator`).
  - Test that a non-admin is refused.
  - Regenerate `apps/thunderforge/schema.graphql` and the web's generated types with the repository's existing schema export.
  - `schema.graphql` is cross-cutting. The slices named in Phase 12 answer it.
- [X] T031 [US8] Add `apps/web/src/pages/admin/TelemetryPanel.tsx`, filled by a plain GraphQL fetch hook that exposes `refetch()` (AGENTS.md §2), and render it from `apps/web/src/pages/admin/SettingsPage.tsx`.
  - It is read-only, with Appendix A.4's text, both rows and the install id.
  - It names the variables to set and offers no toggle.
  - `apps/web/src/pages/admin/__tests__/TelemetryPanel.test.tsx` compares its strings with Appendix A.4.
- [X] T032 [P] [US8] Write the written disclosure.
  - Add a **Telemetry** section to `README.md` with Appendix A.1.
  - Write `docs/guides/telemetry.md` with Appendix A.2 in full (FR-034), including the plain static host note of open item 6.
  - In `docs/CONTRIBUTING.md`, state the naming and cardinality rules, and that the anonymous allow-list is a Principle VII change.
- [X] T033 [US8] Add `apps/thunderforge/src/telemetry/disclosure_tests.rs` (`#[cfg(test)]`). It reads Appendix A from `specs/086-full-telemetry/spec.md`, plus `README.md` and `docs/guides/telemetry.md`, with `include_str!`. It asserts that each place holds its part verbatim, apart from markup and the angle-bracket values, and that `startup_line`'s templates equal A.3 (SC-012).
- [X] T034 [US8] Write `apps/web/e2e/telemetry-admin-panel.spec.ts`. An admin sees **Telemetry** with `off` on the test stack, both rows and an install id, and a non-admin cannot reach it.

**Checkpoint**: All four places agree with Appendix A, and the test that
compares them is green.

---

## Phase 5: User Story 3 — The owner sees the server's health, and is told when it breaks (P1)

**Goal**: The backplane, subscription, GraphQL, HTTP, pool and world-event
numbers become instruments. Five dashboards and the server's alert rules
read them.

**Independent Test**:

- `cargo test -p thunderforge-server telemetry` proves that the instruments
  equal the atomics, the extension's span and labels, the pool gauges and
  the name table.
- `make observability-check` passes offline.

- [X] T035 [P] [US3] Write failing tests in `crates/thunderforge-server/src/telemetry/instruments.rs`, using an in-memory `MeterProvider` (dev-dependency `opentelemetry_sdk` with `testing`). They check that every FR-010 series is reported, and that each value equals its atomic after the atomics are bumped (SC-002).
- [X] T036 [US3] Implement `instruments.rs` with observable counters and gauges over the existing atomics, and the `static DELIVERY: OnceLock<Arc<DeliveryMetrics>>`.
  - Register `DELIVERY` in `crates/thunderforge-server/src/network/listener.rs` where the listener builds its metrics.
  - Add `static WORLD_CHANNELS_REAPED: AtomicU64`, bumped by the reaper (R19).
  - Leave `spawn_metrics_reporter`'s stderr lines unchanged.
- [X] T037 [P] [US3] Write the failing scan test in `crates/thunderforge-server/src/telemetry/event_names.rs`. It reads `world_events.rs` with `include_str!`, finds every `EVENT_CODE_*` (29 today), and asserts that each has a name.
- [X] T038 [US3] Implement the name table in `event_names.rs`.
  - Add the two counter calls (`thunderforge.world_events`, `thunderforge.world_event_record_failures`) in `record_world_event` (`crates/thunderforge-server/src/world_events.rs:344`).
  - Add `thunderforge.rolls{event, visibility}` where `crates/thunderforge-server/src/graphql/mutations_roll.rs` records a roll, using `Visibility::as_str` (`everyone`, `gm_eyes`, `gm_only`).
  - *Done centrally (2026-10-09):* `record_world_event` counts codes 36 and 37 as rolls, so a roll recorded by any path, rerolls included, is counted once, and `mutations_roll.rs` is unchanged.
- [X] T039 [P] [US3] Write failing tests in `crates/thunderforge-server/src/telemetry/graphql_extension.rs`. A mutation gives one span named `graphql.mutation <field>` and one histogram point with `root_field=<field>`. They also check:
  - a field not in the schema gives `unknown`, and two root fields give `root_fields=multiple`;
  - `FORBIDDEN`, `UNAUTHENTICATED` and `NOT_IN_DEMO` give `outcome=refused`;
  - an error with no code counts as `code=internal`;
  - the client's operation name is on the span and in no metric label (SC-002, R20);
  - a subscription's span covers only its setup.
- [X] T040 [US3] Implement `graphql_extension.rs`, and register it at `Schema::build` in `apps/thunderforge/src/main.rs:559`.
- [X] T041 [P] [US3] Implement `crates/thunderforge-server/src/telemetry/pool_events.rs`, with tests.
  - An r2d2 `HandleEvent` records `thunderforge.db.pool.checkout_wait` and `checkout_timeouts` (R18).
  - Observable gauges read `pool.state()`.
  - Set it with `Builder::event_handler` at `apps/thunderforge/src/main.rs:398`.
  - *Deferred:* the operator-only span event on a checkout. Every one would be a Bunyan line too; the histogram carries the wait.
- [X] T042 [US3] Implement `apps/thunderforge/src/telemetry/http.rs` and use it for the `TraceLayer` at `apps/thunderforge/src/main.rs:767`.
  - `make_span` gives an OTel server span with `http.route` (the matched template), the method and the status, and declares `trace_id` and `span_id` as `Empty`. `on_request` records them, so that Bunyan carries them (FR-004, R17).
  - It records `thunderforge.http.server.duration{route, method, status_class}`.
  - The W3C `traceparent` of an incoming request becomes the parent.
  - Test that a Bunyan line written inside a request holds `trace_id`.
  - *As built:* the duration is a `from_fn` middleware beside the `TraceLayer`, whose `OnResponse` never sees the request. Every OTel span is at INFO on `SPAN_TARGET`; `telemetry/bunyan.rs` hides their START and END lines and skips their fields (`SPAN_FIELDS`), so a line gains only `trace_id` and `span_id` (SC-001).
- [X] T043 [US3] Re-run and extend the SC-011 leak test (`apps/thunderforge/src/telemetry/tests.rs`) now that the GraphQL, HTTP and roll instruments exist. The anonymous tier holds only allow-listed attributes, and the operator tier holds `world.id`.
  - *Done in `apps/thunderforge/src/telemetry/pipeline_tests.rs`:* the real HTTP span, GraphQL extension, `world_event.record` span and recorders, into the installed providers.
- [X] T044 [US3] Create `deploy/k8s/observability/` as `contracts/observability-apply.md` gives it:
  - `kustomization.yaml`, and `dashboards/kustomization.yaml` with a `configMapGenerator` labelled `grafana_dashboard: "1"`, annotated `grafana_folder: ThunderForge`, in `monitoring`;
  - `dashboards/{server,graphql,backplane,database,world-events}.json`, each using `${prometheus}`, `${loki}` and `${tempo}`, with no fixed UID;
  - `prometheus-rules.yaml`, labelled `release: kube-prometheus-stack`, with FR-030's nine server alerts (the two landing alerts are added in US4, and the two gateway alerts in US9, T101).
- [X] T045 [US3] Add `scripts/check-observability.mjs` (SC-008, as the contract gives it, reading `print_instruments` from `cargo test -p thunderforge-telemetry-policy`), and the `observability` and `observability-check` targets in `Makefile`, using the existing `KUBE_CONTEXT`, `KUBE_NAMESPACE`, `DEPLOY` and `LANDING_DEPLOY`, and the new `OBS_DIR` and `OTEL_IN_CLUSTER`.
  - `promtool` is skipped with a note when it is absent.
  - Run `make observability-check`.

**Checkpoint**: The server tests pass, `make observability-check` passes, and
`pnpm e2e:rolls` and `pnpm e2e:worlds` stay green.

---

## Phase 6: User Story 1 — The owner sees where demo visitors drop off (P1)

**Goal**: The seven funnel steps, from the landing through the demo to the
call to action, are each sent once per session.

**Independent Test**: `pnpm -F @thunderforge/demo e2e telemetry` passes for
SC-003, and the whole demo e2e still asserts `outside` equal to `[]`.

- [X] T046 [US1] [083] Change `openDemo` in `apps/demo/e2e/support.ts:17`, and `demo.spec.ts`'s own copy, as FR-033 says.
  - Route the telemetry origin's `/v1/*` to `204`, and keep the bodies in a new `telemetry` list.
  - Every other outside request stays in `outside`, which stays asserted `[]` in all 15 places.
- [X] T047 [US1] Change the demo's config and page.
  - In `apps/demo/vite.config.mts`, widen `sealedPage()`'s `<meta>` `connect-src` to `'self' data: blob: https: http:`, leaving every other directive unchanged (R24).
  - Add `servedTelemetry()` from `@thunderforge/telemetry/vite`, which sets `preview.headers` and `server.headers`.
  - Change `apps/demo/index.html:10`'s description so it no longer says that nothing leaves the browser.
- [X] T048 [US1] In `apps/demo/src/guard/install.ts`, pass a `POST` to `<telemetry origin>/v1/logs` or `/v1/traces` through to the real `fetch`, and only when the config is enabled and names that origin. Every other cross-origin request is still refused (`refuse("Reaching another website")`, line 143). Add a unit test beside the guard's existing tests.
- [X] T049 [US1] Add `apps/demo/src/telemetry.ts`.
  - It reads the config with the guard's own static fetch, boots, and sends `demo_opened` with `entry=landing|direct`, from `document.referrer`'s origin.
  - `map_loaded` is sent on the engine's first frame with the scene's map drawn.
  - `view_switched` is sent from `switchView` in `apps/demo/src/DemoNotice.tsx:34`.
  - Call it from `apps/demo/src/main.tsx`.
  - Done as `telemetry.ts` (eager: the config read through the guard's `staticFetch`, and the click/switch calls) and `telemetryChunk.ts` (lazy: the reporter, `demo_opened`, `map_loaded`). `map_loaded` is the engine's first-frame report (`onGridSnapChanged`, or `getEngineState().started` when the engine was up first) while the active scene has a map, so no engine change was needed.
- [X] T050 [US1] Add `apps/demo/src/backend/telemetryTap.ts`, called from `record()` in `apps/demo/src/backend/events.ts:53`, which every accepted mutation passes through. No edit to `handlers/*` is needed.
  - It sends `token_moved` on the first token event, and `dice_rolled` on the first `EVENT.rollMade`.
  - It counts `demo.action` by kind for tokens, walls, doors, lights, shapes, rolls, scene changes, map imports and start-over, with no content.
  - Have `apps/demo/src/backend/notInDemo.ts` send `demo.not_in_demo` with the refused root field.
  - Add `telemetryTap.test.ts`. The seed must not trigger `token_moved`.
  - Known approximation: the demo records any token change as token "updated", so an HP edit before a drag also counts as `token_moved`.
- [X] T051 [US1] Add the calls to action.
  - `DemoNotice` gains **Run your own**, linking to `/#self-host`.
  - `apps/demo/src/telemetry.ts` sends `cta_clicked` with `cta=run_your_own`.
  - Add `data-cta` to the landing's links: **Try the demo** in `apps/landing/src/sections/Hero.tsx:385` and `MapLegend.tsx:231`, **GitHub** in `apps/landing/src/Nav.tsx:27`, and **Sponsor** in `Nav.tsx:39` and `sections/Support.tsx:24`.
- [X] T052 [US1] Add `apps/landing/src/telemetry.ts`.
  - It sends `landing_viewed` and `cta_clicked` from a delegated click listener on `[data-cta]`.
  - Call it from `apps/landing/src/main.tsx`.
  - In `apps/landing/vite.config.mts`, add `servedTelemetry()`, and alias `@thunderforge/feedback-redaction` to `apps/web/src/services/feedbackRedaction.ts` (R12).
  - A landing click sends only the `cta_clicked` event, not the funnel step: the step is the demo's **Run your own**, and a **Try the demo** click taking it first would end the funnel before the demo began. `data-placement` sits beside `data-cta` on all nine links (hero, nav, map legend, star chart, support).
- [X] T053 [US1] In `apps/landing/nginx.conf.template`, add the maps, `location = /telemetry.json` and `location = /demo/telemetry.json`, and the `connect-src` header on `location /demo/` and `= /demo`. Also add the telemetry origin to `location /`'s `connect-src`, as `contracts/served-config-and-csp.md` gives them.
  - In the `Dockerfile`, the `landing` stage gains `ENV TELEMETRY=true` and the two `THUNDERFORGE_BROWSER_TELEMETRY_*` defaults, and the `server` stage gains `ENV TELEMETRY=true`.
- [X] T054 [US1] Write `apps/demo/e2e/telemetry.spec.ts`.
  - Route the document, rewriting its CSP header to `connectSrcFor` for `https://telemetry.invalid`. Route `**/telemetry.json` to an enabled config, and answer `/v1/*` with `204` (R14).
  - Play the whole funnel: open from a landing referrer, wait for the map, drag a token, roll, switch the view, and click **Run your own**.
  - Assert seven `funnel` records, in order, once each, under one `session.id` (SC-003). Every request outside the demo's files goes to `/v1/logs` or `/v1/traces`.
  - It asserts the demo's six steps (`demo_opened` to `cta_clicked`), in order, once each, under one session; `landing_viewed` is the landing's, proved by T055, and in production the two share the session through `sessionStorage` on one origin. The spec waits for `demo_opened` to be posted before the next full page load, so a keepalive post racing the unload cannot make it flaky.
- [X] T055 [P] [US1] Add the landing's e2e.
  - Add `apps/landing/playwright.config.ts`, modelled on `apps/demo/playwright.config.ts`, with its own preview port.
  - Add `"e2e": "pnpm run build && playwright test"` to `apps/landing/package.json`.
  - Write `apps/landing/e2e/telemetry.spec.ts`: `landing_viewed`, and `cta_clicked` for each `data-cta`, with telemetry routed as in the demo spec.
- [X] T056 [US1] Add `deploy/k8s/observability/dashboards/demo-funnel.json`.
  - The funnel panels: the seven steps, step-to-step conversion, and the median time to each step.
  - `demo.action` by kind, and `demo.not_in_demo` by root field.
  - LogQL uses `{service_name="thunderforge-demo"} | event_name="funnel"` (R5).
  - Run `make observability-check`.
  - The median time uses `t.ms` (ms since the session began) through `unwrap t_ms`; conversion uses the count connector's `thunderforge_browser_events_total` by `step`.
- [X] T057 [P] [US1] Amend `specs/074-a-world-to-try/spec.md` at the Why (line 27), the decision (line 76) and SC-003, each with a note pointing to spec 086. SC-003 reads as FR-032 gives it.

**Checkpoint**: `pnpm -F @thunderforge/demo e2e` (every spec) and
`pnpm -F @thunderforge/landing e2e` are green.

---

## Phase 7: User Story 2 — The owner sees the errors visitors hit (P1)

**Goal**: Render throws, uncaught errors, rejections and WebGL2 failures
arrive redacted, folded and capped.

**Independent Test**: The `AppErrorBoundary` unit test (SC-006), the demo
spec's error test, and `apps/web/e2e/telemetry-errors.spec.ts`.

- [X] T058 [P] [US2] Write the failing test `apps/web/src/components/__tests__/AppErrorBoundary.test.tsx`. A child that throws in render shows the boundary's fallback, and the boundary posts one `error` event through a fake sink, with an email in the message redacted.
- [X] T059 [US2] Implement `apps/web/src/components/AppErrorBoundary.tsx`, which reports through the telemetry chunk when it is loaded and through nothing otherwise. Wrap the routes in `apps/web/src/App.tsx`.
- [X] T060 [US2] Send `engine.load_failed` with `webgl2Unavailable`'s reason (`apps/web/src/engine/bevy/index.ts:475`), through a stage hook that `apps/web/src/telemetry/index.ts` subscribes to. The engine code calls no telemetry API. `resumable-downloads` owns this file.
  - Done as `engine/bevy/loadSignals.ts`, a dependency-free signal bus that `mountEngine` emits to and that replays its last few signals, because the chunk arrives after a no-WebGL2 failure. The reason is the closed `no_webgl2`, never the visitor-facing words; `stage` is `probe`. The demo's chunk subscribes too. T074 reuses the same bus.
- [X] T061 [P] [US2] Write `apps/web/e2e/telemetry-errors.spec.ts`.
  - An uncaught error and an unhandled rejection, raised with `page.evaluate`, each post one `error` event, with the email in the message redacted.
  - The same error thrown 500 times posts one record with a count, within the 50-error cap.
- [X] T062 [US2] Add an error test to `apps/demo/e2e/telemetry.spec.ts`: a thrown error is posted redacted, with a stack reduced to path and line.
- [X] T063 [US2] Add the errors panels to `deploy/k8s/observability/dashboards/demo-funnel.json`: errors by message, a sample of redacted stacks, and engine failures by reason, for `thunderforge-demo` and `thunderforge-web`. Run `make observability-check`.

**Checkpoint**: The boundary test, `telemetry-errors.spec.ts` and the demo
spec are green, and `pnpm e2e:feedback` stays green.

---

## Phase 8: User Story 4 — The owner sees landing traffic and how fast it feels (P2)

**Goal**: The landing reports page views, scroll depth and vitals. nginx
logs JSON with no address, and its exporter feeds the **Landing**
dashboard.

**Independent Test**: `scripts/check-landing-nginx.sh` (SC-009), the landing
e2e, the 25 KB check in `pnpm -F @thunderforge/landing build` (SC-007), and
`make observability-check`.

- [X] T064 [US4] Change `apps/landing/nginx.conf.template` for FR-027.
  - Add a `log_format` with `escape=json`, holding the time, method, `$uri`, status, bytes, request time, `$upstream_cache_status`, and the referrer reduced to its origin by a `map`. It holds no address, `X-Forwarded-For` or user agent, and `/healthz` stays unlogged.
  - Add `stub_status` on `127.0.0.1:8081` only.
- [X] T065 [US4] Add `scripts/check-landing-nginx.sh` (R16). It builds the `Dockerfile`'s `landing` stage and runs it three times: with the defaults, with `TELEMETRY=false`, and with a redirected endpoint. It checks both `telemetry.json` files, both `connect-src` values, that the access-log line for `/?utm_source=x` parses as JSON with no query, address or user agent, and that `stub_status` is not reachable on 8080. It skips with a note when Docker is not running.
- [X] T066 [US4] In `apps/landing/src/telemetry.ts`, add:
  - page views with the referrer's origin and `utm_source`, `utm_medium` and `utm_campaign`;
  - scroll depth by section reached, with an `IntersectionObserver` over the section ids (`dice`, `map`, `self-host`, `telemetry` and the rest);
  - web vitals and long tasks through `startCollectors`.

  Extend `apps/landing/e2e/telemetry.spec.ts` for the page view's UTM attributes and the scroll depth.
- [X] T067 [US4] Add `apps/landing/scripts/check-telemetry-size.mjs`, run at the end of `apps/landing/package.json`'s `build`. It fails when the telemetry chunk is over 25 KB brotli, or when the initial document requests a different number of scripts than before (SC-007).
- [X] T068 [P] [US4] Add `deploy/k8s/observability/landing/exporter-sidecar.yaml`, a strategic-merge patch adding `nginx/nginx-prometheus-exporter` with the newest 1.x image pinned by tag and digest. It scrapes `http://127.0.0.1:8081/stub_status` and serves port `metrics` (9113). Add `landing/podmonitor.yaml`, in `thunderforge-dev`, labelled `release: kube-prometheus-stack`. Include the PodMonitor, not the patch, in the kustomization.
- [X] T069 [US4] Add `deploy/k8s/observability/dashboards/landing.json`.
  - From nginx: requests by path and status, bytes, the `/gh/` cache status, and connections.
  - From Loki: page views, referrers, UTM sources, CTA clicks, scroll depth, and web vitals at p75 by page.

  Add `ThunderForgeLandingDown` and `ThunderForgeLanding5xx` to `prometheus-rules.yaml`. Run `make observability-check`.

  Done (2026-10-09):
  - T064 logs `$request_uri` without its query, not `$uri`: after `try_files`, `$uri` is `/index.html` for every page.
  - T065 passes with `--template-only` (the template in stock nginx). The full `landing` image build was not run here; `LANDING_IMAGE=<tag>` checks a built one.
  - T066 sends `utm.*` on the first page view only, reduced to `[A-Za-z0-9._-]`, 64 characters. Sections are marked with `data-section`.
  - T067's baseline is the built `index.html`: one script and one modulepreload, neither the telemetry chunk (6.9 KB brotli).
  - T068 pins the exporter at `1.5.3` by its index digest.
  - T069: `ThunderForgeLanding5xx` is a Loki ruler rule, `loki-rules/thunderforge-landing.yaml`, not a Prometheus one. stub_status carries no status codes and no gateway series reaches Prometheus, so the access log is the only record of a 5xx. fluxified's `loki.yaml` projects the `loki-thunderforge-rules` ConfigMap beside the audit rules (T105). The dashboard's per-path, status, bytes and cache panels read the access log in Loki too. `check-observability.mjs` checks those queries against Alloy's stream labels and `landing_json`'s fields, and reads the Loki rules.

**Checkpoint**: `scripts/check-landing-nginx.sh`, the landing build and its
e2e, and `make observability-check` pass.

---

## Phase 9: User Story 5 — A visitor is told what is collected, and nothing they type leaves (P2)

**Goal**: **What we measure** is on the landing and in the demo. The canary
never appears in a body, and GPC or DNT limits a session to errors.

**Independent Test**: The canary and GPC tests in
`apps/demo/e2e/telemetry.spec.ts`, and the landing's **What we measure**
test.

- [X] T070 [US5] Add `apps/landing/src/sections/WhatWeMeasure.tsx`, at `id="telemetry"`, with Appendix A.5's text. Render it in `apps/landing/src/App.tsx` after `SelfHost`, and link it from `apps/landing/src/sections/Footer.tsx`. Extend `apps/thunderforge/src/telemetry/disclosure_tests.rs` to compare its text with A.5.
- [X] T071 [US5] Add the notice's telemetry line to `apps/demo/src/DemoNotice.tsx`, set from the served config:
  - anonymous: "Anonymous usage counts go to ThunderForge; what you type does not.", linking to `/#telemetry`;
  - operator: "go to this server's operator";
  - off: nothing.

  Add a unit test for the three.
- [X] T072 [US5] Add SC-004 and SC-005 tests to `apps/demo/e2e/telemetry.spec.ts`.
  - The canary typed into chat, used as a token name, and used as an uploaded file's name is in no telemetry body.
  - With the config off, no telemetry request is made and no chunk loads.
  - With a redirected endpoint, every post goes there.
  - With `Sec-GPC: 1` (`navigator.globalPrivacyControl` stubbed with `addInitScript`), only `error` is sent.
- [X] T073 [P] [US5] Extend `apps/landing/e2e/telemetry.spec.ts`: `#telemetry` holds A.5's headings, and GPC limits the landing to `error`.

**Checkpoint**: The demo and landing e2e are green, and the disclosure test
covers all five places.

---

## Phase 10: User Story 7 — Engine loads and slow actions can be traced (P3)

**Goal**: An `engine.load` trace with its stages, an `engine.frames`
summary, and `traceparent` on sampled GraphQL requests, joined to the
server's spans.

**Independent Test**: The unit tests for `loadTelemetry`, `framesSummary`
and `graphqlClient`, and `apps/web/e2e/telemetry-engine.spec.ts`.

- [X] T074 [P] [US7] Add `apps/web/src/engine/bevy/loadTelemetry.ts` with a unit test. It turns `mountEngine`'s stage callbacks (`apps/web/src/engine/bevy/index.ts:522`) into an `engine.load` span with `download`, `compile` and `start` children. Add the stage hooks to `index.ts` without a telemetry import.
  - Done. `engine.load` has `download`, `compile` and `start`; `compile` is what is left of the wasm instantiation once the download is taken out, because the browser does not report the two apart. `resumed` (a download picked up from the cache) is left off: the resumable-download path reports no stage of its own. The stages arrive as `loadSignals.ts` signals stamped with `at` when emitted, so the span's times are the stages' own and not when telemetry heard of them.
- [X] T075 [P] [US7] Add `apps/web/src/engine/bevy/framesSummary.ts` with a unit test. It turns `apps/web/src/engine/bevy/stats.ts`'s mirror into one `engine.frames` record per minute, with frame-rate percentiles and no ids, in sampled sessions only.
  - Done. One `engine.frames` record per minute is the contract; it is also sent at `pagehide`, so a visit shorter than a minute still reports.
- [X] T076 [US7] In `apps/web/src/api/graphqlClient.ts`, next to `withCsrf` (line 318), add `traceparent` in sampled sessions only. Test that it is absent when telemetry is off or unsampled.
  - Done through `apps/web/src/api/requestTracing.ts`, a registry the telemetry chunk fills, so `graphqlClient.ts` imports no telemetry code. `Telemetry.begin()` returns an open span (its `traceparent` and `end`), and the browser's `graphql.request` span is the parent the server's span joins.
- [X] T077 [US7] Write `apps/web/e2e/telemetry-engine.spec.ts`. With an enabled, fully sampled routed config, mounting the board posts an `engine.load` trace with three child spans, and a GraphQL request carries a `traceparent` whose trace id matches a posted span.
- [X] T078 [US7] Add the engine load time and frame-rate panels to `deploy/k8s/observability/dashboards/demo-funnel.json`, and the Tempo link from a slow field in `graphql.json`. Run `make observability-check`.
  - Done. `graphql.json` also gains a Tempo link from the browser's `graphql.request` spans, beside the one from a slow field.

**Checkpoint**: `pnpm e2e:telemetry` and `pnpm e2e:resumable-downloads` are
green.

---

## Phase 11: User Story 9 — The owner's endpoint labels the source and drops spam (P2)

**Goal**: `apps/telemetry-gateway` answers CORS for any origin, adds the
source labels, drops spam by reason, sheds load, and forwards the rest to
the collector's `otlp/public` receiver. It shares one policy crate with the
server (FR-037 to FR-046).

**Independent Test**: `cargo test -p thunderforge-telemetry-policy` and
`cargo test -p thunderforge-telemetry-gateway`. The gateway's tests run the
router in process against a fake upstream; no cluster and no e2e slice is
needed (SC-014).

- [X] T091 [P] [US9] Write failing tests in `crates/thunderforge-telemetry-policy/src/{lists,caps,labels,rate}.rs`, deterministic, as `contracts/telemetry-gateway.md`'s Tests section lists them:
  - `SERVICE_NAMES`, the browser resource, record and span lists, `GATEWAY_LABELS` and `GATEWAY_INSTRUMENTS` are each enumerated exactly;
  - `BROWSER_RECORD_ATTRIBUTES` equals `ALLOWED_ATTRIBUTES` in `packages/telemetry/src/allowList.ts`, read with `include_str!` (SC-015);
  - `attribute_allowed` and `metric_allowed` for each place and service;
  - `cap_for`, `is_instance_id`, `instance_id_required`, `source_for`, `client_version`, `reduce_user_agent` (one fixture string per family) and `country`;
  - `TokenBucket` under a fixed `Instant` advanced by hand, including the `Retry-After` value;
  - `DropReason::as_str()` gives the ten names.
- [X] T092 [US9] Implement `crates/thunderforge-telemetry-policy/src/{caps,labels,rate}.rs` and the rest of `lists.rs` until T091 passes. The crate stays `std` only.
- [X] T093 [P] [US9] Create `apps/telemetry-gateway` (package `thunderforge-telemetry-gateway`) and add it to the root `Cargo.toml`'s `members`.
  - `Cargo.toml`: `opentelemetry-proto = { version = "0.33.0", default-features = false, features = ["gen-tonic-messages", "trace", "logs", "metrics", "with-serde"] }`, `prost = "0.14"`, `serde_json`, `axum`, `tower` (`limit`, `load-shed`, `timeout`), `tower-http` (`cors`, `limit`, `trace`), `tokio`, `reqwest`, `clap` (`derive`, `env`), `tracing`, the otel 0.33 SDK and OTLP exporter as `apps/thunderforge` has them, and `thunderforge-telemetry-policy` by path (R26).
  - `src/main.rs`: the flags and environment of the contract's Configuration table, the gateway's own telemetry to `OTEL_EXPORTER_OTLP_ENDPOINT` (default `http://otel-collector.monitoring:4318`), and `axum::serve`.
  - Check `cargo tree -d -p thunderforge-telemetry-gateway`: no second `prost`, `reqwest` or `tower`.
- [X] T094 [US9] Capture fixtures in `apps/telemetry-gateway/tests/fixtures/`: one JSON body per signal from `packages/telemetry`'s OTLP/JSON encoder, one protobuf body per signal from the server's exporter (an in-memory export re-encoded with `opentelemetry-proto`), and one hostile body per drop reason. Write a fixture test that each honest body decodes and re-encodes to the same content (R26).
  - Done. The browser bodies come from `packages/telemetry`'s encoder (`tests/fixtures/capture-browser.mts`); the server bodies from the real `opentelemetry-otlp` exporters posting to a local listener (the ignored `capture_server_fixtures` test). The browser sends no metrics, so the JSON metrics case is the server's protobuf body re-encoded as OTLP/JSON.
- [X] T095 [US9] Write failing integration tests in `apps/telemetry-gateway/src/tests.rs` (`#[cfg(test)]`): the router through `tower::ServiceExt::oneshot`, a fake upstream axum router on `127.0.0.1:0` that records every request, an in-memory meter and a captured `tracing` subscriber. They cover every item of SC-014:
  - the preflight for `https://game.example.org` and for `Origin: null`;
  - each label for `owner_site`, `self_hosted_browser` and `server`, with and without `CF-IPCountry`;
  - `X-Forwarded-For: 203.0.113.77` and the full `User-Agent` appear in no byte the fake received, in no captured log line and in no metric attribute;
  - one test per drop reason, asserting the status, what the fake received and `dropped{reason}` at one;
  - `429` with `Retry-After` under a fake clock, and the fast `503` when the fake holds every upstream slot;
  - the upstream's `partial_success` is passed back.
- [X] T096 [US9] Implement `apps/telemetry-gateway/src/router.rs`: the three paths and `/healthz`, `CorsLayer` for any origin without credentials, and the `ServiceBuilder` of R31 (`load_shed`, `concurrency_limit`, `timeout`, `RequestBodyLimitLayer` at 4 MiB), with a `TraceLayer` that records method, matched path, status and latency only.
  - Done. `CorsLayer` answers a preflight `200`; a middleware outside it rewrites that to the contract's `204`. A response mapper sits between CORS and the trace layer, because CORS builds its preflight from a default body the trace layer's body has not got. Graceful shutdown on SIGTERM as well as Ctrl-C.
- [X] T097 [US9] Implement `apps/telemetry-gateway/src/intake.rs`: decode by content type, apply the policy per resource and record, strip and count unlisted attributes, set the labels of FR-041, and build the `partial_success` answer.
  - Done. Scope, link and exemplar attributes are cleared, not checked against a list (no list names any); the span name and a log's `event_name` are capped at `CAP_DEFAULT`.
- [X] T098 [US9] Implement `apps/telemetry-gateway/src/limits.rs`: the client IP from `X-Forwarded-For` at the trusted hop count, hashed with a per-process `RandomState`, and the bounded IP and instance bucket maps with idle eviction (FR-042, FR-043, R28).
- [X] T099 [US9] Implement `apps/telemetry-gateway/src/{upstream,metrics}.rs`: one `reqwest::Client` with the connect and total timeouts, a semaphore taken with `try_acquire`, a fresh protobuf request with only `Content-Type`, and the four gateway instruments (FR-044, FR-045). T095 passes.
  - Done. `dropped` counts once per request per reason, not once per record, so a hostile batch of a thousand records is one count. 26 tests pass; `cargo tree -d` shows no second `prost`, `reqwest` or `tower`.
- [X] T100 [US9] Add the `telemetry-gateway` stage to `Dockerfile` (its own cargo-chef cook and build, release only, `debian:bookworm-slim`, non-root), placed before `server` so `server` stays the default stage, and add `TELEMETRY_GATEWAY_IMAGE`, `telemetry-gateway-image` and `push-telemetry-gateway` to `Makefile`, with help lines beside `push-landing`. Check `docker build --target telemetry-gateway .` and that the image's `--help` lists every flag.
  - Done. `docker build --target telemetry-gateway .` builds a 132 MB image; `--help` lists every flag of the contract with its environment name, and the image runs as 65532 on a read-only root, answering `/healthz` and a foreign origin's preflight (`204`).
- [X] T101 [US9] Add the **Public telemetry intake** row to `deploy/k8s/observability/dashboards/server.json`, and `ThunderForgeTelemetryIntakeFailing` and `ThunderForgeTelemetrySpam` to `prometheus-rules.yaml`, as the contract gives them. `scripts/check-observability.mjs` reads the gateway's series from `print_instruments`. Run `make observability-check`.
  - Done. `make observability-check`: 7 dashboards, 12 Prometheus and 1 Loki alert expressions read only names that are sent (promtool is not installed here).
- [X] T102 [P] [US9] In `docs/CONTRIBUTING.md`, describe the gateway and the policy crate: the one list, the labels, the drop reasons, and that a release adding an instrument ships its gateway with or before it (R27).

**Checkpoint**: `cargo test -p thunderforge-telemetry-policy`,
`cargo test -p thunderforge-telemetry-gateway`, `cargo test -p thunderforge
telemetry`, `make observability-check` and the image build pass.

---

## Phase 12: Polish & Proof

- [ ] T079 Run `make lint` (host and wasm32), and fix what it reports. Every Rust file stays at 1000 lines or fewer (`check-file-length`).
- [ ] T080 Run `cargo test -p thunderforge`, `cargo test -p thunderforge-server`, `cargo test -p thunderforge-telemetry-policy`, `cargo test -p thunderforge-telemetry-gateway`, `pnpm -F @thunderforge/telemetry test`, `pnpm -F @thunderforge/web test` and `pnpm -F @thunderforge/demo test`.
- [ ] T081 Run `pnpm e2e:telemetry`.
- [ ] T082 Run `pnpm e2e:feedback`, `pnpm e2e:resumable-downloads`, `pnpm e2e:rolls` and `pnpm e2e:worlds`. Then run every slice that `pnpm e2e:which --diff` names, except its FULL SUITE line (R21), which those slices answer. Record the slices run and their results in this file.
- [ ] T083 Run `make observability-check` and `scripts/check-landing-nginx.sh`.
- [ ] T084 Walk `quickstart.md`'s Real game, Demo, Landing and Telemetry gateway sections by hand against a local collector, and fix any step that does not hold.
- [ ] T085 Mark every task `[x]`, and set spec.md's status.

---

## Phase 13: Cluster apply (last; the owner runs these)

**Purpose**: Ship the images, then the dashboards, rules, exporter and
in-cluster endpoint, then the gateway, then the Flux changes.

- [ ] T086 Ship both images with the existing targets, so the server exports and the landing serves `telemetry.json`:

  ```sh
  make push
  make push-landing
  ```

- [ ] T087 Check and apply the observability manifests:

  ```sh
  make observability-check
  make observability KUBE_CONTEXT=<your context>
  kubectl --context <your context> -n thunderforge-dev rollout status deployment/thunderforge-landing
  kubectl --context <your context> -n thunderforge-dev rollout status deployment/thunderforge
  kubectl --context <your context> -n monitoring get configmap -l grafana_dashboard=1
  kubectl --context <your context> -n thunderforge-dev get prometheusrule,podmonitor
  ```

  Grafana's **ThunderForge** folder holds the seven dashboards. **Backplane** shows poll rates from vtt-dev within 2 minutes, and **Landing** shows `nginx_up 1`.
  - Partly done (2026-10-09, context `default`). `make observability-check` passes, and `kubectl apply -k deploy/k8s/observability` created `loki-thunderforge-rules`, the seven dashboard ConfigMaps, the `thunderforge-landing` PodMonitor and the `thunderforge` PrometheusRule (with T090's alerts). Not done, because it waits on T086: the landing's exporter sidecar patch, the app's `set env`, and the two rollouts. The app was not redeployed.
- [X] T105 In the Flux repository, mount the Loki ruler's ThunderForge rules (T069). In `overlays/system/monitoring/loki.yaml`, the `audit-rules` volume becomes a projected volume of `loki-audit-rules` and `loki-thunderforge-rules` (`optional: true`), still at `/etc/loki/rules/fake`. Check that `ThunderForgeLanding5xx` is listed by the ruler (`/loki/api/v1/rules`).
  - Done in fluxified e14c2bc. After `kubectl apply -k deploy/k8s/observability` created `loki-thunderforge-rules`, the ruler lists `ThunderForgeLanding5xx` (`/prometheus/api/v1/rules`) without a restart.
- [X] T088 In the Flux repository, add the `count` connector from `contracts/collector-count-connector.md` to the public and in-cluster logs pipelines (R8). Confirm that `thunderforge_browser_events_total` appears in Prometheus.
  - Done in fluxified e14c2bc; the merged config passed `otelcol-contrib validate` first. A test post through the gateway gave `thunderforge_browser_events_total` and `thunderforge_browser_errors_total` in Prometheus. The exporter expires a series five minutes after its last count, so a quiet hour shows none.
- [X] T103 Push the gateway image and deploy it, as `contracts/telemetry-gateway.md`'s Shipping section gives it:

  ```sh
  make push-telemetry-gateway
  ```

  - In the Flux repository, add `overlays/services/otel-collector/telemetry-gateway.yaml` (the Deployment and the `telemetry-gateway` Service on 4318, in `monitoring`) and list it in that `kustomization.yaml`. The route is not changed yet.
  - From a test pod, post through the shared gateway and read what reaches the gateway: set `TELEMETRY_GATEWAY_TRUSTED_HOPS` to the hop that holds the client address (R28), and record whether `CF-IPCountry` arrives (R29). Record both answers in research.md.
  - The image and the Deployment are done: `mbround18/thunderforgevtt:telemetry-gateway` (sha256:42315040…), fluxified e14c2bc, 2/2 Running. From a test pod, browser traces and logs got 200, a hostile batch got 200 and was dropped (`unknown_service` counted), and the preflight got 204. After T089, a capture inside a gateway pod gave `X-Forwarded-For: <client>, <Cloudflare edge>`, with anything the client sent to the left of both, and `CF-IPCountry` on every request. `TELEMETRY_GATEWAY_TRUSTED_HOPS` is `2` (fluxified 2a0d05b), and research.md R28 and R29 record both.
- [X] T089 In the Flux repository, switch the public route to the gateway (R7, R25, open item 9). This replaces widening the receiver's CORS.
  - `overlays/services/otel-collector/route.yaml`: the backendRef becomes `telemetry-gateway:4318`, and the header comment says the gateway answers CORS, labels and rate-limits.
  - `helmrelease.yaml`: remove the `otlp/public` receiver's `cors` block. `filter/public`, `transform/public` and `public-service.yaml` stay.
  - Check: a preflight from `https://game.example.org` gets `Access-Control-Allow-Origin: *`, and a post from vtt-dev still arrives, now labelled `owner_site`.
  - Done in fluxified 4f5857d, with the owner's approval; the shared gateway and its listeners are unchanged. A preflight from `https://game.example.org` and from vtt-dev both get `204` with `Access-Control-Allow-Origin: *`. Real Chromium pages on vtt-dev.thunderforge.dev and example.com posted traces and logs and got `200`. The deployed vtt-dev and landing builds predate this spec and send no browser telemetry yet, so live traffic waits for T086. The landing's CSP (`connect-src 'self'`) also blocks a post from thunderforge.dev until then.
- [X] T104 Confirm SC-015 in the cluster: a post from an origin other than the project's reaches Loki with `thunderforge.source="self_hosted_browser"`, and the **Public telemetry intake** row shows accepted and dropped batches. Run `make observability KUBE_CONTEXT=<your context>` if T101's rules are not yet applied.
  - Loki holds the posts with `thunderforge_source="self_hosted_browser"` (origin host `game.example.org`) and `owner_site` (vtt-dev), each with `thunderforge_country="US"`, as structured metadata, not stream labels. `thunderforge_telemetry_gateway_accepted_total` counts both sources, and `_dropped_total` counts `unknown_service`. A fixture whose timestamps are more than a week old is accepted by the gateway and then refused by Loki as too old, so a test post needs fresh timestamps. The two gateway pods' counters reach Prometheus as one series with no pod label, so the row shows whichever pod reported last, not their sum. This is an open finding.
- [X] T090 Once the connector's series exist, add `ThunderForgeBrowserErrorSpike` and `ThunderForgeDemoFunnelStepSilent` from `contracts/collector-count-connector.md` to `deploy/k8s/observability/prometheus-rules.yaml`. Then run `make observability-check` and `make observability KUBE_CONTEXT=<your context>`.
  - Done. `check-observability.mjs` read `offset 10m` as a metric called `m`; it now strips an offset's duration. 14 Prometheus alert expressions pass.

---

## Dependencies & Execution Order

- **Setup (Phase 1)** comes first. T002 and T003 run in parallel.
- **Foundational (Phase 2)** blocks every story.
  - The package tasks (T007 to T012) and the Rust tasks (T013 to T019) are independent of each other.
- **US6 (Phase 3)** is the MVP, and the base of every later story:
  - T023 is what every server instrument exports through;
  - T024 is what every browser reads;
  - T025 is the web's chunk.
- **US8 (Phase 4)** needs T023, so that the startup line knows the state, and T016.
- **US3 (Phase 5)** needs T023. Its dashboards need T044 before US1, US2, US4 and US7 add theirs.
- **US1 (Phase 6)** needs T011 and T024, and T046 waits for 083 ([083]).
- **US2 (Phase 7)** needs T025. T062 needs T054.
- **US4 (Phase 8)** needs T053 and T052.
- **US5 (Phase 9)** needs T054 and T055. Its canary test needs T050.
- **US7 (Phase 10)** needs T025 and T042 (`traceparent` is joined on the server).
- **US9 (Phase 11)** needs T017 and T018 (the policy crate), T011 (the browser's `allowList.ts` and OTLP encoder, for T091 and T094), and T044 and T045 (for T101). It does not need any other story, so it can run beside US1 to US7 once those are done.
  - T092 needs T091. T095 needs T093 and T094. T096 to T099 need T095, and T099 finishes it.
  - T100 needs T093. T101 needs T099.
- **Polish (Phase 12)** comes after every story it covers.
- **Cluster (Phase 13)** comes after Phase 12.
  - T090 needs T088.
  - T103 needs T100.
  - T089 needs T103 and the hop count it records. It replaces the old CORS widening.
  - T104 needs T089 and T101.

## Parallel Opportunities

- Phase 1: T002 ∥ T003.
- Phase 2: T005 ∥ T006 ∥ T007 ∥ T008 ∥ T013 ∥ T015 ∥ T017. Then the package line (T009 → T010 → T011) ∥ the Rust line (T014, T016, T018, T019).
- US6: T020 ∥ T021. Then T026 ∥ T027.
- US8: T028 ∥ T032.
- US3: T035 ∥ T037 ∥ T039 ∥ T041.
- US1 and US4 touch the same landing files, so they run in sequence. US2 and US7 touch the web app's separate files, so they run in parallel after US6.
- US9: T091 ∥ T093 ∥ T102. The gateway touches no file another story touches, apart from `server.json`, `prometheus-rules.yaml` and `Makefile` (T101, T100), so it runs beside US1 to US7 once its dependencies are in.
- Two agents in this tree serialise their commits and stage explicit paths. Never `git add -A`.

## Implementation Strategy

1. **MVP = Setup + Foundational + US6.**
   - The server exports anonymously by default, to an operator's collector when redirected, and nothing when off.
   - Browsers follow the served file.
   - The leak test proves the allow-list.

   Stop and check: `cargo test -p thunderforge telemetry`, and `pnpm e2e:telemetry`.
2. **Then US8**, so the default is disclosed before any visible data flows. **Then US3**, the server's numbers and alerts.
3. **Then US1 and US2**, the demo funnel and the errors: the owner's two questions.
4. **Then US4, US5 and US7.**
5. **Then US9**, the gateway, so the public route can be switched before self-hosted browsers report.
6. **Then Polish**, which proves the whole by slices.
7. **Then Phase 13**, which the owner runs.
