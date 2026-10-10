# Contract: Browser events, `packages/telemetry`

Every browser record is one OTLP log record or one span, encoded as OTLP/JSON
by the package's own adapter (R10). Loki receives a log record's attributes
as structured metadata, with dots turned into underscores (R5).

## The package's surface

```ts
// packages/telemetry/src/index.ts: the core, which knows no OTLP and no DOM
export interface TelemetryConfig { enabled: boolean; endpoint?: string; sampleRate?: number;
  environment?: string; tier?: "anonymous" | "operator"; instanceId?: string }
export function parseConfig(json: unknown): TelemetryConfig | null;   // null = off
export interface TelemetrySink { send(batch: Batch, opts: { keepalive: boolean }): Promise<void> }  // the port
export type Redactor = (text: string) => string;                          // the port
export interface Telemetry {
  event(name: EventName, attrs?: Attrs): void;        // never throws
  error(source: ErrorSource, error: unknown): void;   // never throws
  span(name: SpanName, start: number, end: number, attrs?: Attrs, parent?: SpanRef): SpanRef;
  funnel(step: FunnelStep, attrs?: Attrs): void;      // once per session
  traceparent(): string | null;                       // null when this session is unsampled
  flush(keepalive: boolean): void;
}
export function createTelemetry(opts: { service: ServiceName; version: string; config: TelemetryConfig;
  sink: TelemetrySink; redact: Redactor; storage: Pick<Storage, "getItem" | "setItem">;
  now: () => number; random: () => number; privacy: { gpc: boolean; dnt: boolean } }): Telemetry;
export const noopTelemetry: Telemetry;

// packages/telemetry/src/otlp/index.ts: the adapter
export function otlpHttpSink(endpoint: string, fetchImpl: typeof fetch): TelemetrySink;

// packages/telemetry/src/browser.ts: the DOM collectors, loaded inside the lazy chunk
export function startCollectors(t: Telemetry, opts: { routeTemplate: () => string }): void;  // vitals, long tasks, nav timing, onerror, unhandledrejection, pagehide

// packages/telemetry/src/boot.ts: what an app imports eagerly, about 1 KB
export function bootTelemetry(opts: { configUrl: string; fetchImpl?: typeof fetch;
  load: () => Promise<(cfg: TelemetryConfig) => Telemetry> }): Promise<Telemetry>;  // resolves to noopTelemetry when off

// packages/telemetry/src/vite.ts: dev and preview only
export function servedTelemetry(opts: { paths: string[] }): Plugin;
export function connectSrcFor(cfg: TelemetryConfig | null): string;
```

`bootTelemetry`:

1. reads the config;
2. waits for `load`;
3. checks GPC and DNT;
4. dynamic-imports the app's chunk (FR-017).

Until it resolves, the apps' hooks call `noopTelemetry`. Events from before it
resolved are not queued, apart from the funnel step: a step reached before
then is kept in `sessionStorage` and sent when the chunk arrives.

## Resource attributes (FR-019, FR-019a)

| Key | Value |
| --- | --- |
| `service.name` | `thunderforge-landing`, `thunderforge-demo` or `thunderforge-web` |
| `service.version` | the build's `__APP_VERSION__` |
| `deployment.environment` | anonymous: `production` (landing image) or `self-hosted`; operator: the config's `environment` |
| `thunderforge.tier` | `anonymous` or `operator` |
| `thunderforge.instance.id` | the config's `instanceId`, only when present |
| `browser.family`, `browser.major` | from `navigator.userAgentData`, or a 20-line UA parse |
| `os.family` | `windows`, `macos`, `linux`, `android`, `ios`, `chromeos`, `other` |
| `device.mobile` | bool |
| `viewport.bucket` | `<480`, `<768`, `<1024`, `<1440`, `<1920`, `>=1920` |
| `device.memory.bucket` | `<=1`, `2`, `4`, `>=8`, `unknown` |
| `device.cores.bucket` | `<=2`, `4`, `8`, `>=12`, `unknown` |

## Common record attributes

- `event.name` is always present.
- `session.id` is 32 hex characters.
- `t.ms` is the integer milliseconds since the session start, measured with
  `performance.now()`.
- `route` is a route template.

## Events

The attribute allow-list is exactly the union of the columns below plus the
common ones. `ALLOWED_ATTRIBUTES` in `src/allowList.ts` is that union, and a
test enumerates it.

| `event.name` | When | Attributes | Sampled? |
| --- | --- | --- | --- |
| `page_view` | each route change, and the first load | `route`, `referrer.origin`, `utm.source`, `utm.medium`, `utm.campaign` (landing only), `nav.type` | no |
| `funnel` | once per step per session | `step` (below), `entry` (`landing` or `direct`, on `demo_opened`), `cta` (on `cta_clicked`) | no |
| `demo.action` | each accepted demo action | `action` = `token_moved`, `wall_drawn`, `door_toggled`, `light_placed`, `shape_drawn`, `dice_rolled`, `scene_changed`, `map_imported`, `start_over`, `view_switched` | no |
| `demo.not_in_demo` | `reportNotInDemo(what)` | `root_field` (what was refused, a schema field name; `unknown` if it is not one) | no |
| `cta_clicked` | a CTA link | `cta` = `try_demo`, `sponsor`, `github`, `run_your_own`; `placement` = `nav`, `hero`, `map_legend`, `star_chart`, `support`, `demo_notice` | no |
| `scroll_depth` | the deepest landing section reached, sent at `pagehide` | `section` = `hero`, `dream`, `dice`, `map`, `numbers`, `stance`, `stars`, `self-host`, `support`, `footer` | no |
| `error` | boundary, `onerror`, `unhandledrejection` | `error.source` = `boundary`, `onerror`, `unhandledrejection`, `engine`; `error.type`; `error.message` (redacted, 512); `error.stack` (redacted, frames as `path:line`, 4096); `error.count` (folded repeats) | no |
| `engine.load_failed` | `webgl2Unavailable()` returns a reason | `reason` (that function's closed set), `stage` | no |
| `web_vital` | the `web-vitals` callbacks, finalised at `pagehide` | `metric` = `LCP`, `INP`, `CLS`, `FCP`, `TTFB`; `value` (double); `rating` | yes |
| `long_task` | each `longtask` entry over 50 ms, at most 100 per session | `duration.ms` | yes |
| `nav_timing` | once, after `load` | `dns.ms`, `connect.ms`, `ttfb.ms`, `dom.ms`, `load.ms`, `transfer.bytes` | yes |
| `engine.frames` | `pagehide` with the board open (web and demo) | `fps.p5`, `fps.p50`, `frame_ms.p95`, `tokens.bucket` (`0`, `1-10`, `11-50`, `51-200`, `>200`) | yes |
| `telemetry.internal` | with the next batch, when the counter is above zero | `internal_errors`, `dropped` | no |
| `sheet_import.step` | each step of bringing a sheet in (spec 048) | `step` = `opened`, `read`, `reviewed`, `applied`, `declined`, `failed`; `reason` on `failed` only = `encrypted`, `too_large`, `too_many_pages`, `unrecognised`, `unreadable`, `plan_changed` | no |

The funnel steps, in order, are:

1. `landing_viewed`
2. `demo_opened`
3. `map_loaded`
4. `token_moved`
5. `dice_rolled`
6. `view_switched`
7. `cta_clicked`

`cta_clicked` is both an event, sent on every click, and the seventh funnel
step, sent on the first click only.

## Spans (sampled sessions only)

| Span | Children | Attributes |
| --- | --- | --- |
| `page.load` | none | `route`, `nav.type` |
| `engine.load` | `download`, `compile`, `start` | `bytes`, `resumed`, `service.name`. The engine's stages are `downloading`, then `starting`, then the first frame, from `mountEngine`'s stage callbacks |
| `graphql.request` | none. The server's span joins it through `traceparent` | `graphql.operation.type` and `graphql.root_field` (the first root field, parsed from the document) |

`traceparent` is `00-<32 hex trace id>-<16 hex span id>-01`. It is added in
`apps/web/src/api/graphqlClient.ts` beside `withCsrf` (line 318), and only when
`telemetry.traceparent()` returns a value.

## Limits (FR-020)

| Limit | Value |
| --- | --- |
| error events per session | 50. Repeats of one message fold into `error.count` on the first event, and the count is re-sent at flush |
| events per session | 2,000 |
| queue | 200 records. When full, the oldest is dropped and `dropped` is counted |
| flush | every 5 s, on `visibilitychange` to hidden, and on `pagehide` (with `keepalive: true`) |
| batch body | at most 60 KB. A larger batch is split. A single record over 60 KB is dropped and counted |
| send | once per batch. A failure is dropped and is never reported as an error |
| GPC (`navigator.globalPrivacyControl`) or DNT (`navigator.doNotTrack === "1"`) | only `error` and `engine.load_failed` (an error) are sent. No spans, and no sampling decision is made |

## Session (`sessionStorage`, key `thunderforge.telemetry`)

```json
{ "id": "<32 hex>", "start": 1759830000000, "steps": ["demo_opened", "map_loaded"], "sampled": true }
```

- `start` is the wall-clock time the session began. It is used only to
  re-base `performance.now()` across the viewer switch's reload, never sent.
- `sampled` is decided once per session. A tab with no `sessionStorage`
  (when it throws) uses an in-memory session.
- The key is never written to `localStorage`, and no cookie is set.

## Where each app calls it

| App | Moment | Where (AGENTS.md §2: not in components) |
| --- | --- | --- |
| landing | page view, `landing_viewed`, scroll depth, CTA | `apps/landing/src/telemetry.ts`, started from `main.tsx`; CTAs through one delegated `click` listener on `[data-cta]` anchors |
| demo | `demo_opened` | `apps/demo/src/telemetry.ts`, at boot (`entry` from `document.referrer`'s path being `/` on the same origin) |
| demo | `map_loaded` | the engine's first frame with a map, through the existing engine probe event in `apps/web/src/engine/bevy/index.ts` |
| demo | `token_moved`, `dice_rolled`, `demo.action` | `apps/demo/src/backend/telemetryTap.ts`, called from `record()` in `apps/demo/src/backend/events.ts`, which every accepted mutation passes through |
| demo | `view_switched`, `run_your_own` | `DemoNotice.tsx` calls a function from `apps/demo/src/telemetry.ts`. A link click is chrome, not world state, so it is not a store command |
| demo | `demo.not_in_demo` | `reportNotInDemo` in `backend/notInDemo.ts` |
| web | page view by route template | `apps/web/src/telemetry/routes.ts`, from the router's matches |
| web | error boundary | `apps/web/src/components/AppErrorBoundary.tsx`, around `App`'s routes |
| web | `engine.load` and `engine.load_failed` | `apps/web/src/engine/bevy/loadTelemetry.ts`, subscribed to `mountEngine`'s stages |
| web | `engine.frames` | `apps/web/src/engine/bevy/framesSummary.ts`, which samples `stats.ts` once a second into a fixed 600-entry ring |
| web | `sheet_import.step` | `apps/web/src/pages/world/actor/import/telemetry.ts`, called by the review's step changes, recording through this package's `telemetry` |
