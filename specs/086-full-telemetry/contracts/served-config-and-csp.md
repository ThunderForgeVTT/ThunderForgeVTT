# Contract: The served browser config and the demo's `connect-src`

The runtime switch for browsers. There are three producers, and all of them
agree on the content:

- the server, through `crates/thunderforge-server/src/telemetry/served_config.rs`;
- the landing's nginx, through `apps/landing/nginx.conf.template`;
- the dev and preview servers, through `packages/telemetry/src/vite.ts`.

## The file

`GET /telemetry.json` and `GET /demo/telemetry.json` answer
`200 application/json` with `Cache-Control: no-store`.

When telemetry is on:

```json
{
  "enabled": true,
  "endpoint": "https://telemetry.thunderforge.dev",
  "sampleRate": 1.0,
  "environment": "self-hosted",
  "tier": "anonymous",
  "instanceId": "4f6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d11"
}
```

When it is off:

```json
{"enabled":false}
```

The off answer has no other keys. In particular it carries no endpoint and
no instance id.

| Field | Server source | nginx source | Default |
| --- | --- | --- | --- |
| `enabled` | `TELEMETRY` (false, 0, no or off in any case means off) | `TELEMETRY` | `true` |
| `endpoint` | `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` | the same | `https://telemetry.thunderforge.dev` |
| `sampleRate` | `THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE`, clamped to 0..1 (unparseable means 1.0) | the same, not clamped (documented) | `1.0` |
| `environment` | `THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT` | fixed `production` | `self-hosted` |
| `tier` | `tier_for(endpoint)` | a `map` with the same normalisation | `anonymous` |
| `instanceId` | `instance_settings` row `system.telemetry_instance_id` | absent | (generated) |

`OTEL_SDK_DISABLED` does not touch this file. It stops only the server's own
export.

### How a client reads it

1. Before `load`, the client fetches the config with `credentials: "omit"`
   and `cache: "no-store"`. The demo uses the guard's own static fetch.
2. A non-200 answer, a body that does not parse, a missing `enabled`, an
   endpoint that is not an `http(s)` URL, or `enabled: false` all mean off.
   The chunk is not imported.
3. `endpoint` is an origin with an optional path prefix. The client posts to
   `${endpoint}/v1/logs` and `${endpoint}/v1/traces`, with any trailing slash
   on `endpoint` removed first.
4. On the anonymous tier, the client sets `deployment.environment` itself:
   `production` for `thunderforge-landing`, and for the demo when it is served
   by the landing's nginx, and `self-hosted` otherwise. It uses `environment`
   only on the operator tier (FR-019a). The landing image's file says
   `production`. A server-served file says what the server was given.

## The tier's normalisation

`tier_for` in Rust and the nginx map both use this table. A Rust table test
and the nginx check script each run it.

| Input | Tier |
| --- | --- |
| `https://telemetry.thunderforge.dev` | anonymous |
| `https://telemetry.thunderforge.dev/` | anonymous |
| `https://TELEMETRY.thunderforge.dev` | anonymous |
| `https://telemetry.thunderforge.dev:443` | anonymous |
| `HTTPS://telemetry.thunderforge.dev:443/` | anonymous |
| `not a url`, or an empty string | anonymous (a typo only ever sends less) |
| `http://telemetry.thunderforge.dev` | operator (another scheme is another destination) |
| `https://telemetry.thunderforge.dev/v1` | operator (another path) |
| `https://otel.example.org` | operator |
| `http://otel-collector.monitoring.svc.cluster.local:4318` | operator |

## The `connect-src` header

There is one builder on each side, and each uses the same config as the file:

- on the server, `served_config::connect_src(&BrowserTelemetry) -> String`;
- in Vite, `connectSrcFor(config)`;
- in nginx, `$telemetry_connect`.

| Config | Header |
| --- | --- |
| on, endpoint `https://telemetry.thunderforge.dev` | `Content-Security-Policy: connect-src 'self' data: blob: https://telemetry.thunderforge.dev` |
| on, endpoint `https://otel.example.org/otlp` | `Content-Security-Policy: connect-src 'self' data: blob: https://otel.example.org` |
| off | `Content-Security-Policy: connect-src 'self' data: blob:` |

Where it is sent:

- **Server.** `static_files::demo_router` adds the header to every response
  under `/demo` and `/demo/assets`, through a `SetResponseHeaderLayer` built
  once at start.
- **Landing nginx.** `location /demo/` and `location = /demo` send it.
  `location /`'s existing policy gains the telemetry origin in its
  `connect-src` and nothing else (FR-027).
- **Vite.** The dev server and `preview` send it through `server.headers` and
  `preview.headers`.

### The `<meta>` policy changes, not only loses a directive

`sealedPage()`'s `<meta>` has `default-src 'self' data: blob:`. If
`connect-src` were simply dropped from the `<meta>`, `default-src` would
still govern `fetch`, and the telemetry origin would be blocked whatever the
header said.

So the `<meta>` keeps a `connect-src`, widened to
`connect-src 'self' data: blob: https: http:`. The header narrows it to the
one origin, because a browser enforces both policies and the stricter wins.

On a plain static host that sends no header, the guard is what limits posts
(open item 6). Every other directive in the `<meta>` is unchanged.

## nginx: the template's additions

```nginx
# 1. Is it on?  ${TELEMETRY} is defined by the image (ENV TELEMETRY=true).
map "${TELEMETRY}" $telemetry_on {
  default               1;
  ~*^(false|0|no|off)$  0;
}

# 2. The tier, by the same normalisation as tier_for.
map "${THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT}" $telemetry_tier {
  default                                                   operator;
  ~*^https://telemetry\.thunderforge\.dev(:443)?/?$         anonymous;
  ~^[^:]*$                                                  anonymous;   # not a URL
}

# 3. The endpoint's origin, for connect-src.
map "${THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT}" $telemetry_origin_raw {
  ~^(?<o>https?://[^/]+)  $o;
  default                 "";
}
map $telemetry_on $telemetry_origin {
  1       $telemetry_origin_raw;
  default "";
}

# 4. The body.
map $telemetry_on $telemetry_json {
  1       '{"enabled":true,"endpoint":"${THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT}","sampleRate":${THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE},"environment":"production","tier":"$telemetry_tier"}';
  default '{"enabled":false}';
}

# 5. connect-src for the demo.
map $telemetry_origin $telemetry_connect {
  ""      "connect-src 'self' data: blob:";
  default "connect-src 'self' data: blob: $telemetry_origin";
}
```

```nginx
location = /telemetry.json      { default_type application/json; add_header Cache-Control "no-store" always; return 200 $telemetry_json; }
location = /demo/telemetry.json { default_type application/json; add_header Cache-Control "no-store" always; return 200 $telemetry_json; }
location /demo/ { add_header Content-Security-Policy $telemetry_connect always; ...; try_files $uri $uri/ /demo/index.html; }
```

The `Dockerfile`'s `landing` stage gains:

```dockerfile
ENV TELEMETRY=true \
    THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://telemetry.thunderforge.dev \
    THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE=1.0
```

The `server` stage gains `ENV TELEMETRY=true` beside its existing block. The
binary's default is already on. The line documents it in `docker inspect`.

## The admin query

```graphql
type TelemetryStatus {
  enabled: Boolean!            # TELEMETRY
  serverExporting: Boolean!    # false when TELEMETRY=false or OTEL_SDK_DISABLED=true
  serverTier: String!          # "anonymous" | "full" | "off"
  serverEndpoint: String       # null when not exporting
  browserEnabled: Boolean!
  browserTier: String!         # "anonymous" | "full" | "off"
  browserEndpoint: String      # null when off
  instanceId: String!
}

extend type AdminQuery { telemetryStatus: TelemetryStatus! }   # admin only
```

On the wire the tier is `full`, not `operator`, to match Appendix A.3's
words. `operator` stays the name in code.

The spec's `telemetryStatus { enabled tier serverEndpoint browserEndpoint
instanceId }` is a subset. The server and browser tiers are separate because
the spec's "half redirected" edge case needs both.
