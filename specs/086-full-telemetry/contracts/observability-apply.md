# Contract: `deploy/k8s/observability` and `make observability`

## Directory

```text
deploy/k8s/observability/
├── kustomization.yaml            # namespace-less; resources + configMapGenerator
├── dashboards/
│   ├── kustomization.yaml        # namespace: monitoring; configMapGenerator per dashboard
│   ├── server.json
│   ├── graphql.json
│   ├── backplane.json
│   ├── database.json
│   ├── world-events.json
│   ├── landing.json
│   └── demo-funnel.json
├── prometheus-rules.yaml         # PrometheusRule, namespace thunderforge-dev, release: kube-prometheus-stack
└── landing/
    ├── podmonitor.yaml           # PodMonitor, namespace thunderforge-dev, release: kube-prometheus-stack
    └── exporter-sidecar.yaml     # strategic-merge patch for deployment/thunderforge-landing (applied by kubectl patch, not by kustomize)
```

`landing/exporter-sidecar.yaml` is not a kustomize resource. It is a patch for
a Deployment whose base is in no repository (R9). `kustomization.yaml` lists
only the dashboards, `prometheus-rules.yaml` and `landing/podmonitor.yaml`.

### The dashboards' ConfigMaps

Each dashboard becomes a ConfigMap with:

- `generatorOptions.labels: { grafana_dashboard: "1" }`;
- `generatorOptions.annotations: { grafana_folder: ThunderForge }`;
- `generatorOptions.disableNameSuffixHash: true`, so a re-apply replaces the
  map rather than adding one.

Every dashboard declares the datasource variables `prometheus`, `loki` and
`tempo` (`type: datasource`) and uses `${prometheus}`, `${loki}` and
`${tempo}`. It never names a fixed UID.

### The exporter sidecar

```yaml
spec:
  template:
    spec:
      containers:
        - name: nginx-exporter
          image: nginx/nginx-prometheus-exporter:1.5.1
          args: ["--nginx.scrape-uri=http://127.0.0.1:8081/stub_status"]
          ports: [{ name: metrics, containerPort: 9113 }]
          resources: { requests: { cpu: 5m, memory: 16Mi }, limits: { memory: 32Mi } }
          securityContext: { runAsNonRoot: true, readOnlyRootFilesystem: true, allowPrivilegeEscalation: false }
```

Pin the image to the newest 1.x tag on Docker Hub at implementation time, and
record the tag here.

### The PodMonitor

The PodMonitor selects `app: thunderforge-landing`, scrapes
`podMetricsEndpoints: [{ port: metrics, interval: 30s }]`, and carries the
label `release: kube-prometheus-stack`.

## The Makefile target

```make
OBS_DIR ?= deploy/k8s/observability
OTEL_IN_CLUSTER ?= http://otel-collector.monitoring.svc.cluster.local:4318

observability: ## Apply dashboards, alert rules and the landing exporter (thunderforge-dev is not Flux-managed)
	kubectl --context $(KUBE_CONTEXT) apply -k $(OBS_DIR)
	kubectl --context $(KUBE_CONTEXT) -n $(KUBE_NAMESPACE) patch deployment $(LANDING_DEPLOY) \
	  --type strategic --patch-file $(OBS_DIR)/landing/exporter-sidecar.yaml
	kubectl --context $(KUBE_CONTEXT) -n $(KUBE_NAMESPACE) set env deployment/$(DEPLOY) \
	  OTEL_EXPORTER_OTLP_ENDPOINT=$(OTEL_IN_CLUSTER)

observability-check: ## Offline checks: kustomize build, dashboards parse, SC-008, promtool
	kubectl kustomize $(OBS_DIR) > /dev/null
	node scripts/check-observability.mjs
	promtool check rules $(OBS_DIR)/prometheus-rules.yaml  # skipped with a note when promtool is absent
```

The target uses the Makefile's existing `KUBE_CONTEXT`, `KUBE_NAMESPACE`,
`DEPLOY` and `LANDING_DEPLOY`. Every command is idempotent, and re-running it
is how a change is shipped.

`THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` is left at the default on
`thunderforge`. vtt-dev's browsers report to the public route, which the
telemetry gateway answers for any origin and labels `owner_site`
([telemetry-gateway.md](telemetry-gateway.md)). The server's export goes to
the in-cluster collector on the operator tier.

## `scripts/check-observability.mjs` (SC-008)

1. Runs `cargo test -p thunderforge-telemetry-policy --quiet -- print_instruments --nocapture --exact`
   and reads the Prometheus names it prints: the server's `INSTRUMENTS` and
   the gateway's `GATEWAY_INSTRUMENTS`.
2. Reads `ALLOWED_ATTRIBUTES` and the event names from
   `packages/telemetry/src/allowList.ts`.
3. Adds the connector's series from
   [collector-count-connector.md](collector-count-connector.md), plus
   `nginx_*`, `target_info` and `up`.
4. Parses every dashboard JSON and `prometheus-rules.yaml`.
   - From each PromQL expression it extracts the metric names, and fails on
     one not in the set.
   - From each LogQL expression it extracts `service_name`, `event_name` and
     the attribute filters, and fails on one not in the allow-list.
5. Exits 1 with the offending file, panel and name.
