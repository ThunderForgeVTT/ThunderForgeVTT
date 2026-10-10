#!/usr/bin/env node
/**
 * Every name a dashboard or an alert reads is a name something sends (spec
 * 086 SC-008, contracts/observability-apply.md).
 *
 * A panel over a series nobody exports draws a flat line, and an alert over
 * one never fires. Neither fails loudly, so this check does: it builds the
 * set of names that exist, from the same sources the sender and the gateway
 * use, and fails on any PromQL metric or LogQL label value outside it.
 *
 * # The names that exist
 *
 *  1. the server's and the gateway's instruments, as Prometheus series,
 *     printed by `cargo test -p thunderforge-telemetry-policy` (so the
 *     conversion rule R4 lives in one place);
 *  2. the browser allow-list and event names, read from
 *     `packages/telemetry/src/allowList.ts`;
 *  3. the collector `count` connector's two series, `nginx_*`, `target_info`
 *     and `up`;
 *  4. for a LogQL query over the landing's access log (a stream selector
 *     naming `container="nginx"`), the stream labels Alloy sets and the
 *     fields of `landing_json`, read from `apps/landing/nginx.conf.template`.
 *
 * It reads the dashboards, `prometheus-rules.yaml` and the Loki ruler's
 * `loki-rules/*.yaml`.
 *
 * Exits 1 naming the file, the panel or alert, and the name.
 */

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const OBS_DIR = resolve(
  ROOT,
  process.env.OBS_DIR ?? "deploy/k8s/observability",
);

/** The connector's series (contracts/collector-count-connector.md). */
const CONNECTOR_SERIES = [
  "thunderforge_browser_events_total",
  "thunderforge_browser_errors_total",
];
const ALWAYS = new Set(["target_info", "up", ...CONNECTOR_SERIES]);
const PREFIXES = ["nginx_"];
const BROWSER_SERVICES = new Set([
  "thunderforge-landing",
  "thunderforge-demo",
  "thunderforge-web",
]);

/** PromQL words that are not metric names. */
const PROMQL_WORDS = new Set(
  (
    "sum min max avg group stddev stdvar count count_values bottomk topk quantile " +
    "by without on ignoring group_left group_right bool and or unless offset " +
    "rate irate increase delta idelta deriv predict_linear resets changes " +
    "histogram_quantile histogram_count histogram_sum histogram_fraction " +
    "abs ceil floor round exp ln log2 log10 sqrt clamp clamp_min clamp_max " +
    "absent absent_over_time present_over_time avg_over_time min_over_time " +
    "max_over_time sum_over_time count_over_time last_over_time quantile_over_time " +
    "stddev_over_time stdvar_over_time label_replace label_join sort sort_desc " +
    "time timestamp vector scalar day_of_month day_of_week hour minute month year inf nan"
  ).split(" "),
);

function instrumentSeries() {
  const out = execFileSync(
    "cargo",
    [
      "test",
      "-p",
      "thunderforge-telemetry-policy",
      "--quiet",
      "--",
      "--nocapture",
      "--exact",
      "lists::tests::print_instruments",
    ],
    { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  const names = [...out.matchAll(/^prometheus: (\S+)$/gm)].map((m) => m[1]);
  if (names.length === 0) {
    throw new Error("print_instruments printed no series");
  }
  return names;
}

function quotedList(source, name) {
  const at = source.indexOf(`export const ${name} = [`);
  if (at < 0) throw new Error(`allowList.ts has no ${name}`);
  const body = source.slice(at, source.indexOf("] as const", at));
  return [...body.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** Metric names in a PromQL expression: identifiers not inside braces,
 *  quotes or `[...]`, not a keyword, and not a label in `by (...)`. */
export function promqlMetrics(expr) {
  const stripped = expr
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/\{[^}]*\}/g, "")
    .replace(/\[[^\]]*\]/g, "")
    // `offset 10m`: a duration, not a metric called `m`.
    .replace(/\boffset\s+-?(?:\d+[smhdwy]+)+/gi, "")
    .replace(
      /\b(by|without|on|ignoring|group_left|group_right)\s*\([^)]*\)/g,
      "",
    );
  const names = new Set();
  for (const m of stripped.matchAll(/[A-Za-z_:][A-Za-z0-9_:]*/g)) {
    const word = m[0];
    const next = stripped.slice(m.index + word.length).trimStart();
    if (PROMQL_WORDS.has(word.toLowerCase())) continue;
    if (next.startsWith("(")) continue; // a function we do not list
    if (/^\d/.test(word)) continue;
    names.add(word);
  }
  return [...names];
}

/** `service_name`, `event_name` and other label filters in a LogQL query,
 *  plus `| key="value"` attribute filters after the stream selector. */
export function logqlFilters(expr) {
  const filters = [];
  for (const m of expr.matchAll(
    /([A-Za-z_][A-Za-z0-9_.]*)\s*(=~|!=|!~|=)\s*"([^"]*)"/g,
  )) {
    filters.push({ key: m[1], op: m[2], value: m[3] });
  }
  return filters;
}

/** The fields of the landing's `landing_json` access-log format. */
export function accessLogFields(template) {
  const at = template.indexOf("log_format landing_json");
  if (at < 0) return [];
  const body = template.slice(at, template.indexOf(";", at));
  return [...body.matchAll(/"([a-z_]+)":/g)].map((m) => m[1]);
}

/** What Alloy labels a pod's log stream with, and LogQL's own error label. */
const STREAM_LABELS = new Set([
  "namespace",
  "pod",
  "container",
  "node",
  "stream",
  "__error__",
]);
const ACCESS_LOG = /container\s*=\s*"nginx"/;

/** The `expr:` values of a PrometheusRule file, with their alert names. */
export function ruleExprs(yaml) {
  const out = [];
  const lines = yaml.split("\n");
  let alert = "(record)";
  for (let i = 0; i < lines.length; i++) {
    const a = lines[i].match(/^\s*-?\s*(alert|record):\s*(\S+)/);
    if (a) alert = a[2];
    const e = lines[i].match(/^(\s*)expr:\s*(.*)$/);
    if (!e) continue;
    if (e[2] === "|" || e[2] === ">") {
      const indent = e[1].length;
      const body = [];
      for (i++; i < lines.length; i++) {
        const l = lines[i];
        if (l.trim() && l.search(/\S/) <= indent) {
          i--;
          break;
        }
        body.push(l);
      }
      out.push({ alert, expr: body.join("\n") });
    } else {
      out.push({ alert, expr: e[2] });
    }
  }
  return out;
}

function main() {
  const series = new Set([...ALWAYS, ...instrumentSeries()]);
  const allowList = readFileSync(
    join(ROOT, "packages/telemetry/src/allowList.ts"),
    "utf8",
  );
  const attributes = new Set(quotedList(allowList, "ALLOWED_ATTRIBUTES"));
  const events = new Set(quotedList(allowList, "EVENT_NAMES"));
  const logFields = new Set(
    accessLogFields(
      readFileSync(join(ROOT, "apps/landing/nginx.conf.template"), "utf8"),
    ),
  );
  if (logFields.size === 0)
    throw new Error("no landing_json log_format in nginx.conf.template");
  const known = (name) =>
    series.has(name) || PREFIXES.some((p) => name.startsWith(p));

  const problems = [];
  const checkPromql = (where, expr) => {
    for (const name of promqlMetrics(expr)) {
      if (!known(name)) problems.push(`${where}: unknown series ${name}`);
    }
  };
  const checkLogql = (where, expr) => {
    if (ACCESS_LOG.test(expr)) {
      for (const { key } of logqlFilters(expr)) {
        if (!STREAM_LABELS.has(key) && !logFields.has(key))
          problems.push(
            `${where}: ${key} is neither a stream label nor a landing_json field`,
          );
      }
      for (const m of expr.matchAll(/\b(?:unwrap|by\s*\()\s*([a-z_, ]+)/g)) {
        for (const key of m[1]
          .split(",")
          .map((k) => k.trim())
          .filter(Boolean))
          if (!STREAM_LABELS.has(key) && !logFields.has(key))
            problems.push(
              `${where}: ${key} is neither a stream label nor a landing_json field`,
            );
      }
      for (const m of expr.matchAll(/\|\s*([a-z_]+)\s*[<>]=?\s*\d/g)) {
        if (!logFields.has(m[1]))
          problems.push(`${where}: ${m[1]} is not a landing_json field`);
      }
      return;
    }
    for (const { key, op, value } of logqlFilters(expr)) {
      const regex = op.endsWith("~");
      const values = regex ? value.split("|") : [value];
      if (key === "service_name") {
        for (const v of values)
          if (!BROWSER_SERVICES.has(v) && v !== "thunderforge")
            problems.push(`${where}: unknown service_name ${v}`);
      } else if (key === "event_name") {
        for (const v of values)
          if (!events.has(v))
            problems.push(`${where}: unknown event_name ${v}`);
      } else if (
        !attributes.has(key) &&
        !attributes.has(key.replaceAll("_", ".")) &&
        key !== "job"
      ) {
        problems.push(`${where}: attribute ${key} is not on the allow-list`);
      }
    }
  };

  const dashDir = join(OBS_DIR, "dashboards");
  const dashboards = readdirSync(dashDir).filter((f) => f.endsWith(".json"));
  for (const file of dashboards) {
    const rel = relative(ROOT, join(dashDir, file));
    let doc;
    try {
      doc = JSON.parse(readFileSync(join(dashDir, file), "utf8"));
    } catch (e) {
      problems.push(`${rel}: does not parse: ${e.message}`);
      continue;
    }
    const vars = new Set((doc.templating?.list ?? []).map((v) => v.name));
    for (const v of ["prometheus", "loki", "tempo"]) {
      if (!vars.has(v)) problems.push(`${rel}: no \${${v}} variable`);
    }
    const walk = (panels) => {
      for (const panel of panels ?? []) {
        walk(panel.panels);
        const uid = panel.datasource?.uid;
        if (uid && !String(uid).startsWith("${")) {
          problems.push(`${rel} "${panel.title}": fixed datasource uid ${uid}`);
        }
        for (const t of panel.targets ?? []) {
          const where = `${rel} "${panel.title}"`;
          const type = t.datasource?.type ?? panel.datasource?.type;
          if (t.expr && type === "loki") checkLogql(where, t.expr);
          else if (t.expr) checkPromql(where, t.expr);
        }
      }
    };
    walk(doc.panels);
  }

  const rulesPath = join(OBS_DIR, "prometheus-rules.yaml");
  const rules = ruleExprs(readFileSync(rulesPath, "utf8"));
  for (const { alert, expr } of rules) {
    checkPromql(`${relative(ROOT, rulesPath)} ${alert}`, expr);
  }

  const lokiDir = join(OBS_DIR, "loki-rules");
  let lokiRules = 0;
  for (const file of readdirSync(lokiDir).filter(
    (f) => f.endsWith(".yaml") && f !== "kustomization.yaml",
  )) {
    const path = join(lokiDir, file);
    for (const { alert, expr } of ruleExprs(readFileSync(path, "utf8"))) {
      checkLogql(`${relative(ROOT, path)} ${alert}`, expr);
      lokiRules++;
    }
  }

  if (problems.length > 0) {
    for (const p of problems) console.error(`observability: ${p}`);
    process.exit(1);
  }
  console.log(
    `observability: ${dashboards.length} dashboards, ${rules.length} Prometheus and ${lokiRules} Loki alert expressions read only names that are sent`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
