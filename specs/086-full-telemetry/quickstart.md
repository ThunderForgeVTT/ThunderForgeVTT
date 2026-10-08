# Quickstart: Full Telemetry

How to see each part work, and the commands that prove it. The shapes are in
[contracts/](contracts/), and the reasons are in [research.md](research.md).

## Real game (self-hosted server)

1. **Default on, anonymous.** Run `make dev` (or the image) with no telemetry
   variable. Then:
   - The startup output has one line:
     `telemetry: server → https://telemetry.thunderforge.dev (anonymous), browsers → https://telemetry.thunderforge.dev (anonymous). …`
   - `curl -s localhost:30000/telemetry.json` gives `enabled: true`,
     `tier: "anonymous"` and an `instanceId`.
   - Restart, and the `instanceId` is the same.
   - Admin, then Settings, shows **Telemetry**, with the anonymous text,
     both rows and the install id.
2. **Redirected.** Start a local collector, then run
   `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318 THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=http://127.0.0.1:4318 make dev`.
   The collector can be the image `otel/opentelemetry-collector-contrib`,
   with the `debug` exporter.
   - The startup line names `127.0.0.1:4318 (full)` twice.
   - The collector shows `thunderforge.backplane.polls` within 60 s, Bunyan
     log records, and spans with `world.id`.
   - `/telemetry.json` says `tier: "operator"`.
3. **Off.** Run `TELEMETRY=false make dev`.
   - The startup line reads `telemetry: off (TELEMETRY=false). …`.
   - `/telemetry.json` is `{"enabled":false}`.
   - The browser's network panel shows no `telemetry` chunk and no `/v1/`
     request.
4. **Half redirected.** Set only `OTEL_EXPORTER_OTLP_ENDPOINT`. The startup
   line and the admin panel show the server as `full` to your collector, and
   browsers as `anonymous` to the project.

## Demo

- Run `pnpm -F @thunderforge/demo dev`. The dev server serves
  `{"enabled":false}` unless
  `THUNDERFORGE_PREVIEW_TELEMETRY='{"enabled":true,"endpoint":"http://127.0.0.1:4318","sampleRate":1,"tier":"operator"}'`
  is set.
- With it set:
  1. Open the demo, wait for the map, drag a token, roll, click **View as
     player**, then **Run your own**. The collector shows seven `funnel`
     records, in order, with one `session_id`.
  2. The notice says "Anonymous usage counts go to this server's operator;
     what you type does not." That is the operator tier's wording.
  3. With the browser's GPC on, only `error` records arrive.
- `curl -sI localhost:30000/demo/` (server-served) shows
  `Content-Security-Policy: connect-src 'self' data: blob: https://telemetry.thunderforge.dev`.

## Landing

- Run `scripts/check-landing-nginx.sh`. It builds the `landing` stage and
  checks `/telemetry.json`, `/demo/telemetry.json`, both `connect-src`
  values, one JSON access-log line for `/?utm_source=x` (no query, address or
  UA), and that `stub_status` is not reachable on 8080. It does this for the
  default, `TELEMETRY=false`, and a redirected endpoint.
- Run `pnpm -F @thunderforge/landing build`. It fails if the telemetry chunk
  is over 25 KB brotli (SC-007).

## Cluster

Run `make observability-check` offline, then `make observability` (see
tasks.md, Phase 12). Then in Grafana, under ThunderForge:

- **Backplane** shows non-zero poll rates from `vtt-dev`.
- **Landing** shows `nginx_up 1`.

## Proof commands

| What | Command |
| --- | --- |
| Rust, the app | `cargo test -p thunderforge telemetry` |
| Rust, the server crate | `cargo test -p thunderforge-server telemetry` and `cargo test -p thunderforge-server static_files` |
| The browser package | `pnpm -F @thunderforge/telemetry test` |
| Web units | `pnpm -F @thunderforge/web test` |
| Demo units | `pnpm -F @thunderforge/demo test` |
| The slice | `pnpm e2e:telemetry` (the whole demo e2e, the landing e2e, then `apps/web/e2e/telemetry-*.spec.ts`) |
| Neighbours | `pnpm e2e:feedback`, `pnpm e2e:resumable-downloads`, `pnpm e2e:rolls`, `pnpm e2e:worlds`, then every slice `pnpm e2e:which --diff` names. It prints FULL SUITE because `world_events.rs` is cross-cutting; the named slices answer that (R21). Never the full suite |
| Lint | `make lint` |
| Cluster, offline | `make observability-check`, then `scripts/check-landing-nginx.sh` |
