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

## Serving the demo from a plain static host

Our server and the landing's nginx send the demo's `connect-src` header,
built from the served config, so a redirected collector is allowed without
a rebuild. A plain static host that serves the built demo by itself sends
no such header, and then only the reporter's own checks limit where the
page posts. If you host the demo that way, have the host send
`Content-Security-Policy: connect-src 'self' data: blob: <your collector's origin>`
itself.
