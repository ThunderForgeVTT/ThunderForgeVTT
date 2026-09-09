# Security audit — 2026-09-09

A sweep across injection, XSS, SSRF, authorisation, session handling,
dependencies and the admin surface. Written down because the *reasoning* is the
durable part: the findings get fixed, but "we checked X and here is why it is
fine" is what stops the next person re-deriving it or, worse, assuming it.

Everything below was checked against the code, not inferred from the design.

---

## What held up

These were attacked deliberately and did not give.

| Area | Why it holds |
|---|---|
| **SQL injection** | No interpolated SQL anywhere. Every `sql_query` passes `$1` parameters (`pg_notify`, `pg_advisory_xact_lock`); everything else is Diesel's typed builder. |
| **Zip-slip** | `systems.rs` uses `enclosed_name()` **and** re-checks `starts_with(extract_path)` after joining. Two independent defences; either alone would do. |
| **XSS** | Two `dangerouslySetInnerHTML` sites, both sanitised. Lore: comrak → ammonia, server-side. Session notes: DOMPurify client-side, including the author's own live preview — with a comment naming the threat (a compromised GM stored-XSSing every player who opens the panel). |
| **Cookies** | Session cookie `HttpOnly` + `SameSite=Strict`. CSRF cookie deliberately readable (double-submit) and still `Strict`. Both asserted in `session.rs`'s tests. |
| **CSRF** | `require_csrf_for_session` middleware: ensures the cookie on any request carrying a session, enforces double-submit on state-changing methods. Server-side, on every request. |
| **Lore sync SSRF** | Takes **no URL**. It is a GitHub App installation grant; host bases come from `GITHUB_API_BASE`/`GITHUB_WEB_BASE`, which are operator environment, not user input. |
| **Anonymous share reads** | Rate-limited (`share_rate_limit`), and a revoked share is deliberately indistinguishable from a code that never existed. |

---

## Findings, and what was done

### 1. A dead dependency carrying two critical advisories — **fixed**

`websocket 0.27.1` was declared in `thunderforge-server`'s manifest and
**nothing imported it**. The engine uses `gloo_net`; the server uses axum's own
WebSocket support. It pulled in:

- `hyper 0.10.16` → **RUSTSEC-2021-0079** (9.1, `Transfer-Encoding` integer
  overflow) and RUSTSEC-2021-0078 (request smuggling);
- `traitobject 0.1.1` → **RUSTSEC-2020-0027** (9.8, assumes fat-pointer
  layout);
- a long tail of unmaintained `tokio-*` 0.1 crates, `net2`, `safemem`,
  `rand_os`.

The manifest already recorded that `openssl` entered the lock "only through the
legacy `websocket 0.27.1` chain" — the cost was known and the cause was not
followed up.

**Removed. `cargo audit` went 8 → 4, and both criticals are gone from the tree
entirely.** The rarest kind of finding: highest severity, one-line fix.

### 2. FR-009's client half was unmet — **fixed**

Spec 036 FR-009: *"An ended session MUST be refused on its next request, and
its client MUST be told to sign in again rather than left showing state it can
no longer refresh."*

The first half was true. The second was not: **nothing in the web app reacted
to a 401.** Session state was read once at mount and never revalidated, so a
client whose session was ended elsewhere kept rendering everything it had.
That matters more now that ending a session from another device is something a
person can actually do — the session list exists to be used.

Fixed in the **transport** (`api/sessionExpiry.ts` + `graphqlClient`), not the
router. Reasoning, because it is the interesting part:

- Routing is not where interaction happens. A play field sits on one route for
  hours and makes hundreds of requests; a route-change hook would miss every
  one of them and fire pointlessly on navigations that call nothing.
- A router hook must run *before* render, so it either blocks on the network or
  is advisory. Blocking is what breaks the offline play field that specs 028
  and 036 US3c deliberately protect.
- Every request goes through one transport. Reacting there sees everything,
  needs no per-route wiring, costs no extra requests, and cannot break offline
  because it only reacts to answers that arrived.

On a 401: drop the session, discard the world cache (so a session no longer
accepted leaves no decrypted content readable), and broadcast the existing
cross-tab sign-out so sibling tabs do not each have to fail a request of their
own.

### 3. 33 production npm advisories — **29 removed**

`shadcn` — a **scaffolding CLI**, never imported by any source file — was
declared in `dependencies` rather than `devDependencies`. Its tree carried most
of the high-severity findings (`js-yaml`, `fast-uri`, `brace-expansion`,
`browserslist`). Moved to dev.

`react-router` was genuinely production and genuinely affected (unauthenticated
DoS, open redirect via backslash in `<Link>`). Bumped 7.14 → 7.18.

**33 → 4.** The remaining four are `@tailwindcss/postcss` → `postcss`/`nanoid`:
a build-time CSS toolchain processing our own source, never shipped to a
browser, and not attacker-reachable. Left deliberately — moving it risks
breaking a production build for no security gain.

### 4. SSRF via an admin-set URL — **guarded**

`userinfo_url` is settable through the admin GraphQL surface and is then
**fetched by the server with the provider access token attached**. Unguarded,
that is a request the instance makes to any address an administrator names, and
the interesting targets are internal: `169.254.169.254` returns cloud instance
credentials; loopback reaches admin interfaces bound there on purpose.

`authorization_url` and `token_url` are **not** affected — they are env-sourced
and re-asserted at startup (ADR-041), so the API cannot change them.

"Only an admin can set it" is weaker than it sounds: an administrator of a
ThunderForge instance is not necessarily trusted with the machine it runs on —
on anything hosted they are usually different people — and an admin account is
a thing that gets taken over. The guard turns that from an infrastructure
compromise back into a ThunderForge problem.

`thunderforge_axum_oidc::url_guard` refuses non-HTTP schemes, literal private,
loopback, link-local and unspecified addresses in v4 and v6, IPv4-mapped v6
(`::ffff:169.254.169.254`), and metadata hostnames by name. Loopback is
permitted only under `cfg!(debug_assertions)`, so a release binary does not
contain the allowance and no environment variable can enable it — the same
two-locks reasoning as `rate_limit_disabled`.

**What it does not promise:** it does not resolve DNS. A hostname resolving to
an internal address gets through, and DNS rebinding defeats any check made at
validation time rather than at connect time. Closing that means refusing the
socket in the HTTP client, which is a larger change and is recorded here rather
than implied away.

### 5. Three authorisation idioms — **partly structural, partly recorded**

Every operator-scoped route and resolver was guarded. But by three different
mechanisms, applied one handler at a time:

- `verify_admin_request(&state, &cookies)` in a REST handler;
- `admin_user(ctx)?` in a GraphQL resolver;
- `authenticated_user(ctx)?` with `is_admin` passed into an `_impl` that
  refuses — `resolve_moderation_case`, which the first pass of this audit
  **missed**, because it does not mention admin anywhere a grep would find.

Nothing was open. But an audit that can miss a guard can miss a *missing* one,
and "correct because each author remembered" is a run of luck rather than a
property.

**Done:** `/authentication/admin/` moved into `auth::admin_router()`, wrapped in
`require_admin_user` as a layer that refuses before a handler is entered, with
`admin_routes_tests` failing if an admin path is registered elsewhere — or if a
non-admin path lands in the guarded router, where it would silently become
administrator-only.

**Not done:** GraphQL still authorises per resolver by convention. A schema-wide
assertion — every resolver reaching an operator-scoped table requires admin —
is the equivalent guarantee and does not exist yet. Worth building; it is the
same shape as the route test, one level up.

---

## Still open

| | |
|---|---|
| 4 Rust advisories | All inside `aws-sdk-s3`'s pinned `h2 0.3` / `rustls 0.21`. Already at the latest published SDK. Client-role only: the `h2` advisory describes a server-side DoS, the `rustls-webpki` ones are certificate-validation edge cases against a configured endpoint. Recheck when the SDK moves. |
| 4 npm advisories | Build-time CSS toolchain. Not shipped. |
| DNS rebinding on outbound URLs | See finding 4. Needs connect-time refusal in the HTTP client. |
| GraphQL authorisation guarantee | See finding 5. |
| Timing side channels | Not examined. `recovery.rs` deliberately verifies every unspent code with no early exit, which suggests the concern is understood where it was thought about; nothing systematic was measured. |
