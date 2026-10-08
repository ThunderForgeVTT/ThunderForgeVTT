# Contract: Browser alerts through the collector's `count` connector

This change lives in the owner's Flux repository, not here. It is applied by
T088. This repository depends on the series names below, and SC-008's checker
knows them as the connector's series, not as instruments.

## The snippet to add to the collector's config

The collector is `otel-collector` in `monitoring`, contrib 0.162.0.

```yaml
connectors:
  count/browser:
    logs:
      thunderforge.browser.events:
        description: Browser telemetry records, by app and event.
        conditions:
          - 'resource.attributes["service.name"] == "thunderforge-landing" or resource.attributes["service.name"] == "thunderforge-demo" or resource.attributes["service.name"] == "thunderforge-web"'
        attributes:
          - key: event.name
            default_value: unknown
          - key: step
            default_value: none
      thunderforge.browser.errors:
        description: Browser error records, by app and source.
        conditions:
          - 'attributes["event.name"] == "error" or attributes["event.name"] == "engine.load_failed"'
        attributes:
          - key: error.source
            default_value: none

service:
  pipelines:
    logs/public:            # existing; add the connector as a second exporter
      exporters: [otlphttp/loki, count/browser]
    logs:                   # existing in-cluster pipeline; the same
      exporters: [otlphttp/loki, count/browser]
    metrics/browser:        # new; the connector's output, in the cluster only
      receivers: [count/browser]
      processors: [batch]
      exporters: [prometheus]
```

`service.name` is a resource attribute. The `prometheus` exporter turns it
into `job`, because there is no `service.namespace`. So there is no
`service_name` label to set.

The connector's output does not go through `filter/public`, because it is a
separate metrics pipeline in the cluster. Its names start with
`thunderforge.` anyway, so moving it behind the filter later changes nothing.

## The series

| Series | Labels | Meaning |
| --- | --- | --- |
| `thunderforge_browser_events_total` | `job` (= the app's `service.name`), `event_name`, `step` | every browser record, so the funnel counts are also in Prometheus |
| `thunderforge_browser_errors_total` | `job`, `error_source` | errors and engine load failures |

The label values are bounded by the browser allow-list in
[browser-events.md](browser-events.md). `step` is one of seven values, or
`none`.

## The two alerts

These go in `prometheus-rules.yaml`, after the series exist (T090):

```yaml
- alert: ThunderForgeBrowserErrorSpike
  expr: sum by (job) (rate(thunderforge_browser_errors_total[10m])) > 3 * sum by (job) (rate(thunderforge_browser_errors_total[6h] offset 10m)) and sum by (job) (increase(thunderforge_browser_errors_total[10m])) > 20
  for: 10m
  labels: { severity: warning }
  annotations:
    summary: "{{ $labels.job }}: browser errors are 3x their 6-hour rate. Open the Demo funnel and errors dashboard, Errors by message."
- alert: ThunderForgeDemoFunnelStepSilent
  expr: sum by (step) (increase(thunderforge_browser_events_total{job="thunderforge-demo",event_name="funnel",step=~"demo_opened|map_loaded"}[1h])) == 0 and on() sum(increase(thunderforge_browser_events_total{job="thunderforge-landing",event_name="page_view"}[1h])) > 20
  for: 30m
  labels: { severity: warning }
  annotations:
    summary: "Landing has visitors but nobody reached {{ $labels.step }} in the demo for an hour. Check the demo loads, then the Demo funnel and errors dashboard."
```

## Why not the Loki ruler

The Loki ruler would need a rules sidecar or a mount, and an
`alertmanager_url`. That is two Flux changes and a second alerting path. See
research R8.
