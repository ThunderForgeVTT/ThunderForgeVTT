/**
 * What a browser record may carry (spec 086, contracts/browser-events.md).
 *
 * Every attribute passes through `filterAttributes` before it is queued, so a
 * key that is not listed here never reaches a batch. The list is the union of
 * the events table's columns plus the common keys. The gateway holds the same
 * list as `BROWSER_RECORD_ATTRIBUTES` in `crates/thunderforge-telemetry-policy`,
 * and a Rust test reads this file to prove the two agree, so keep the array
 * below one quoted key per line.
 */

export const ALLOWED_ATTRIBUTES = [
  "event.name",
  "session.id",
  "t.ms",
  "route",
  "referrer.origin",
  "utm.source",
  "utm.medium",
  "utm.campaign",
  "nav.type",
  "step",
  "entry",
  "cta",
  "placement",
  "action",
  "root_field",
  "section",
  "error.source",
  "error.type",
  "error.message",
  "error.stack",
  "error.count",
  "reason",
  "stage",
  "metric",
  "value",
  "rating",
  "duration.ms",
  "dns.ms",
  "connect.ms",
  "ttfb.ms",
  "dom.ms",
  "load.ms",
  "transfer.bytes",
  "fps.p5",
  "fps.p50",
  "frame_ms.p95",
  "tokens.bucket",
  "internal_errors",
  "dropped",
] as const;

/** What a browser span may carry. The gateway's `BROWSER_SPAN_ATTRIBUTES`. */
export const SPAN_ATTRIBUTES = [
  "session.id",
  "route",
  "nav.type",
  "bytes",
  "resumed",
  "graphql.operation.type",
  "graphql.root_field",
] as const;

export type AttributeKey = (typeof ALLOWED_ATTRIBUTES)[number];
export type SpanAttributeKey = (typeof SPAN_ATTRIBUTES)[number];

export const EVENT_NAMES = [
  "page_view",
  "funnel",
  "demo.action",
  "demo.not_in_demo",
  "cta_clicked",
  "scroll_depth",
  "error",
  "engine.load_failed",
  "web_vital",
  "long_task",
  "nav_timing",
  "engine.frames",
  "telemetry.internal",
] as const;

export type EventName = (typeof EVENT_NAMES)[number];

/** Events sent only in a sampled session. */
export const SAMPLED_EVENTS: ReadonlySet<EventName> = new Set<EventName>([
  "web_vital",
  "long_task",
  "nav_timing",
  "engine.frames",
]);

/** The events a GPC or DNT session still sends: errors only. */
export const ERROR_EVENTS: ReadonlySet<EventName> = new Set<EventName>([
  "error",
  "engine.load_failed",
]);

export const FUNNEL_STEPS = [
  "landing_viewed",
  "demo_opened",
  "map_loaded",
  "token_moved",
  "dice_rolled",
  "view_switched",
  "cta_clicked",
] as const;

export type FunnelStep = (typeof FUNNEL_STEPS)[number];

export const SPAN_NAMES = [
  "page.load",
  "engine.load",
  "download",
  "compile",
  "start",
  "graphql.request",
] as const;

export type SpanName = (typeof SPAN_NAMES)[number];

export const SERVICE_NAMES = [
  "thunderforge-landing",
  "thunderforge-demo",
  "thunderforge-web",
] as const;

export type ServiceName = (typeof SERVICE_NAMES)[number];

export type AttrValue = string | number | boolean;
export type Attrs = Record<string, AttrValue | undefined>;

const RECORD_KEYS: ReadonlySet<string> = new Set(ALLOWED_ATTRIBUTES);
const SPAN_KEYS: ReadonlySet<string> = new Set(SPAN_ATTRIBUTES);

function filterWith(
  keys: ReadonlySet<string>,
  attrs: Attrs | undefined,
): Record<string, AttrValue> {
  const out: Record<string, AttrValue> = {};
  if (!attrs) return out;
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || !keys.has(key)) continue;
    if (typeof value === "number" && !Number.isFinite(value)) continue;
    out[key] = value;
  }
  return out;
}

/** Drops every key a record may not carry, and every undefined value. */
export function filterAttributes(
  attrs: Attrs | undefined,
): Record<string, AttrValue> {
  return filterWith(RECORD_KEYS, attrs);
}

/** The same, for a span. */
export function filterSpanAttributes(
  attrs: Attrs | undefined,
): Record<string, AttrValue> {
  return filterWith(SPAN_KEYS, attrs);
}
