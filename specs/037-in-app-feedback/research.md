# Phase 0 Research: In-App Feedback, Delivered as GitHub Issues

Fifteen decisions. Each records what the codebase already does, what was
chosen, and what was rejected — because several of these have a plausible wrong
answer that satisfies the requirement's words and breaks its promise. FR-012
and FR-018 are the two where that gap is widest, and they are R6 and R3.

---

## R1 — One credential resolver, three scopes, and the check that must not travel

**What is there today**: `src/server/src/repo_host.rs` hard-codes five
variable names as module constants —

```rust
pub const APP_ID_ENV: &str = "SYNC_GITHUB_APP_CLIENT_ID";
pub const APP_SLUG_ENV: &str = "SYNC_GITHUB_APP_SLUG";
pub const APP_PRIVATE_KEY_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY";
pub const APP_PRIVATE_KEY_FILE_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY_FILE";
pub const APP_PRIVATE_KEY_BASE64_ENV: &str = "SYNC_GITHUB_APP_PRIVATE_KEY_BASE64";
```

— and `registration_from_env()` reads exactly those, collecting **every**
problem rather than the first so an operator fixes their configuration in one
pass. `read_private_key()` takes the three declared forms in a fixed
precedence: `_FILE` (Docker and systemd secrets win over a stale inline value),
then `_BASE64` (a declared encoding, whose failure is reported against *that*
variable), then the inline PEM through `normalise_pem`, which expands literal
`\n` and accepts base64 **only when what comes out contains `BEGIN`** — a test
caught that `"not a key at all"` is valid base64.

**Decision**: keep exactly that function and give it a scope. Introduce

```rust
pub enum AppScope { Global, Sync, Feedback }
pub fn registration_for(scope: AppScope) -> Result<ScopedApp, Vec<RegistrationProblem>>
```

where each scope names its own five variables by prefix (`GLOBAL_GITHUB_APP_*`,
`SYNC_GITHUB_APP_*`, `FEEDBACK_GITHUB_APP_*`), resolution is **per field** —
the specific prefix wins for each value it sets, the global prefix fills the
rest (FR-025) — and `ScopedApp` carries a `sources: Vec<(field, prefix)>` so
FR-029's "which value came from where" is a value the surface reads rather than
a sentence somebody writes. `registration_from_env()` stays, delegating to
`registration_for(AppScope::Sync)`, so FR-024 and spec 040's FR-024 ("existing
environment configuration MUST keep working unchanged") are true because
nothing changed, not because somebody was careful.

**The one behaviour that must not travel**: `registration_from_env()` currently
pushes `RegistrationProblem::GitBinaryMissing` when
`crate::lore_sync::git::git_is_available()` is false. That is correct for lore
sync, which drives `git` directly, and **wrong for every other scope** —
feedback creates an issue over HTTPS and needs no git at all. Carrying it
across would make a container without git report the feedback destination as
misconfigured while it works perfectly. The git check moves out of the shared
resolver and into the sync scope's own diagnostic.

**Alternatives rejected**:

- *A second copy of the function with different constants.* Four private-key
  forms, a precedence rule and a base64 trap that already needed a test to get
  right, duplicated — and the copy would drift on the first fix.
- *A generic `GITHUB_APP_*` with a subsystem suffix.* Renames the working
  variables. FR-024 forbids it, and an operator with a running deployment is
  the person who would pay.
- *Resolving whole credential sets rather than fields.* A feedback app with a
  slug and no key would silently fall back to the global app's identity **and**
  the global app's key — a different application than either the operator set.
  Per-field resolution with a recorded source is what makes FR-029's
  "half-configured is never silently completed" observable instead of asserted.

---

## R2 — Issue creation already exists, and it lives in the server, not the crate

**What is there today**: `crates/thunderforge-repo-host` is deliberately pure —
its manifest says "Deliberately no `axum`, `diesel` or `reqwest` dependency". It
signs the app JWT (`jsonwebtoken` 10 with the `aws_lc_rs` backend, RS256, `iss`
= the client ID), builds URL **strings** (`repository_url`,
`installations_url`, `installation_repositories_url`) and parses two payloads.
It speaks no HTTP.

Every HTTP call lives in `src/server/src/repo_host.rs`, and **issue creation is
already there**: `open_issue(installation_id, owner, name, title, body)` at
line 621 POSTs `{api_base}/repos/{owner}/{name}/issues` and returns the
`html_url`. `github::REQUESTED_PERMISSIONS` already asks for `issues:write`
alongside `contents:write`, so no operator has to re-consent to an installation
for feedback to use it.

**Decision**: feedback does **not** need issue creation added. It needs
`open_issue` to take a scope, because today it calls `registration_from_env()`
internally and is therefore hard-wired to the sync app. It gains a
scope-carrying sibling; `open_issue` keeps its signature and passes
`AppScope::Sync`. Three things do need adding, all small and all in
`repo_host.rs`: **labels on creation** (FR-020), **a search-before-create
lookup** (R4), and **a Contents API write** for attachments (R15).

**What is wrong with `open_issue` today and must be fixed for this use**: it
never checks `response.status()`. It parses the body and infers failure from a
missing `html_url`, reporting `created["message"]`. For lore sync's occasional
disassociation notice that is survivable. For a retry loop it is not: a 403
secondary-rate-limit response and a 422 validation error are permanently
different situations and this code cannot tell them apart, so a retry would
either hammer the host or abandon a submission that would have succeeded.
FR-019 and FR-021 both need the status code.

**Alternatives rejected**:

- *Add an issues client to `thunderforge-repo-host`.* It would be the crate's
  first HTTP dependency, and the crate's whole value is that it is a pure
  signing-and-URL library with no runtime. Adding `reqwest` there to avoid
  eight lines in the server is a bad trade.
- *Let a `feedback` module build its own request.* `lore_sync`'s module docs
  argue this exactly once and land the other way: "the structural fact that
  would matter to a second host is *where the HTTP lives*, not which strings it
  contains." One module speaks to the host. That module is `repo_host.rs`.

---

## R3 — The submission is the record; the tracker is a destination

**FR-018 decides the architecture more than any other line in the spec.** It
says the instance records the submission *before* delivery is attempted, which
means the durable object is the submission and GitHub is somewhere it is later
copied to. The alternative — call GitHub in the resolver and store the issue
URL — makes GitHub the store, and then FR-030 ("with no destination configured
at all, submissions MUST still be kept") has nowhere to keep anything.

**Decision**: `feedback_submissions` is written inside the mutation's
transaction, along with its attachment rows, and the mutation returns success
at that point. Delivery is a separate pass, driven by a background task, that
reads submissions in state `pending` and moves them to `delivered` or leaves
them `pending` with another attempt recorded.

**What that task looks like — and it is not new infrastructure**:
`src/server/src/lore_sync/schedule.rs` is the same problem already solved, and
solved in the shape `src/app/src/main.rs` uses five times (a `spawn_*_task` in
the library, called from the binary, owning its own schedule and staying off
every hot path). Its own docs say why nothing more is used: "a queue or an
external scheduler would be a new deployment component for a loop the process
can hold itself." Feedback copies four things from it verbatim in shape:

| `lore_sync/schedule.rs` | feedback |
|---|---|
| `TICK_SECONDS: u64 = 30` | `TICK_SECONDS: u64 = 30` |
| `BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600]` | the same array, same reasoning: it ends rather than growing forever |
| `pub fn due_now(conn, now) -> Result<Vec<Due>, String>` | `pub fn due_now(...)`, pulled out for exactly the reason given there — a rule enforced by an `if` inside a loop inside a spawned task is a rule nothing can test |
| `spawn_lore_sync_task(state)` spawned unconditionally at `src/app/src/main.rs:485` | `spawn_feedback_delivery_task(state)` beside it, also unconditional — gating the spawn on configuration would make an operator restart after configuring |

**Alternatives rejected**:

- *Deliver inline and store only on failure.* Two code paths to the same
  destination, of which the failure path is the one nobody exercises. It also
  makes a submission's latency the tracker's latency, and SC-001's thirty
  seconds is a person's budget, not GitHub's.
- *A `pg_notify` wake-up instead of a tick.* The pubsub path exists
  (`src/server/src/pubsub.rs`), and it would make a submission arrive sooner —
  but a notify that is missed while the process restarts leaves an item
  waiting for something that already fired. A tick recovers by itself. A notify
  can be added later *on top of* the tick as an optimisation; it cannot replace
  it.
- *An external queue.* A new deployment component, and the argument against it
  is already written in `schedule.rs` for a feature with the same shape.

---

## R4 — "Exactly once" when the destination has no idempotency key

**The problem, stated precisely**: FR-019 forbids duplicates after a retry, and
GitHub's issue-creation endpoint accepts no idempotency key. The failure that
produces a duplicate is not a bug in the retry logic — it is the request that
*succeeded at the host* and whose response never came back. The instance cannot
distinguish that from a request that never arrived.

**Decision**: three mechanisms, in order, and none of them is "retry carefully".

1. **A delivery key that is visible at the destination.** Every submission gets
   an opaque `delivery_key` (a UUIDv7, generated at record time, never reused).
   It is written into the issue body as a single trailing line — the same
   technique `lore_sync/binding.rs` already uses to make an issue identifiable
   as ours by reading it back rather than by remembering it.
2. **Search before create, but only after an ambiguous failure.** A first
   attempt creates directly. Any attempt after a *transport* failure or a 5xx
   — the two outcomes where the host may have acted — first issues
   `GET /search/issues?q=<delivery_key>+repo:owner/name` and adopts the
   existing issue if one is found. A 4xx never triggers the search, because a
   4xx means the host declined and created nothing.
3. **Single-flight by state, in the database.** `due_now` selects only
   submissions whose latest delivery attempt has a non-null `finished_at`,
   exactly as `lore_sync`'s selection refuses a connection whose latest run has
   a null `outcome` ("starting a second pass for one connection would have two
   processes writing one clone"). An in-flight attempt is not due.

**The residual case, recorded rather than hidden**: search indexing at GitHub
is not immediate, so an attempt retried within seconds of an ambiguous failure
can miss its own issue. This is why the first backoff step is thirty seconds
rather than immediate, and why the search is a *lookup by a key we control*
rather than by title — a duplicate, if one ever occurs, carries the same
`delivery_key` as its twin and is therefore findable and closable, which is the
honest form of "exactly once" against a host that does not offer it.

**Alternatives rejected**:

- *Trust the HTTP status alone.* The response that never arrives is precisely
  the case with no status.
- *Store the issue number optimistically before the call.* There is no number
  to store until the host allocates one.
- *A lock table or advisory lock.* The state machine already expresses
  single-flight, and a lock adds a second thing that can be stale after a
  crash. `lore_sync` reached the same conclusion.

---

## R5 — The browser log buffer: bounded, in memory, and never written down

**What is there today**: no console interception, no `window.onerror` handler,
no `unhandledrejection` handler and no client log store anywhere in
`apps/web/src`. There is exactly one error boundary,
`apps/web/src/appearance/PackSurfaceBoundary.tsx`, and it is deliberately not
at the root — its own header records that `apps/web` "had no error boundary at
all… so this is new machinery rather than an existing one to reuse", and
explains why a root boundary was rejected. So a React render throw today
reaches `console.error` and nothing else, which is precisely the line a bug
report wants.

**Decision**: a module — `apps/web/src/services/feedbackLogBuffer.ts` — that
patches `console.error`, `console.warn` and `console.info`, and registers
`window.onerror` and `unhandledrejection`, at app start. Every entry is
**redacted at capture** (R6), then pushed into a fixed-capacity ring: **500
entries, 128 KB total, whichever is reached first**, oldest evicted. The ring
lives in a module-scoped array and is **never** written to `localStorage`,
`sessionStorage`, IndexedDB or OPFS.

Three properties, each answering a specific worry:

- **Not a memory leak**: the capacity is fixed, so a session that logs for
  eight hours holds the same bytes as one that logs for eight seconds. Each
  entry is truncated to 2 KB before it is stored, so one enormous line cannot
  consume the budget alone.
- **Not a privacy problem at rest**: because it is never persisted, closing the
  tab destroys it. A shared machine does not inherit the previous person's
  logs, and no storage-clearing obligation is created.
- **Countable when it overflows**: the ring keeps `droppedCount` and
  `droppedOldestAt`, which is what FR-011 needs — "when anything is dropped for
  size the person MUST be told what was kept" requires a number, not a shrug.

**Why patching `console` rather than a logging façade**: the errors worth
having are the ones nobody wrote a call for — a React render throw, a rejected
`fetch` in `graphqlClient.ts`, a wasm panic surfaced by Bevy. Those reach
`console.error` and `window.onerror`, and a façade would capture only the lines
somebody remembered to route through it.

**Alternatives rejected**:

- *An unbounded array trimmed on submit.* The leak is real on a four-hour
  session, and it is the sessions that last four hours whose logs are wanted.
- *Persisting the buffer so it survives a reload.* It would turn a bug report
  into a store of other people's screens on a shared machine, and a reload is
  usually the thing the person did *because* it broke — the interesting lines
  are the ones before it, which is what R13's draft survival covers instead.
- *Sampling or levels.* Bounded capacity already produces the same effect
  without a rule about which errors matter, decided before anyone saw them.

---

## R6 — Redaction happens at capture, and the server refuses rather than rewrites

**This is FR-012, and it is the requirement to defend.** The checklist says it
plainly: a plan that redacts server-side after the person approved a different
thing has satisfied the words and broken the promise. The promise is that
**the preview is the truth**.

**Decision**, in three parts:

1. **Redaction is a pure function applied at capture time**, before a line
   enters the ring buffer — `redact(line: string): string` in
   `apps/web/src/services/feedbackRedaction.ts`. The buffer therefore never
   holds a secret at any moment, so there is no window in which a memory dump,
   a later code change, or a second reader of the buffer could find one. What
   the review renders is the buffer; what is submitted is the buffer. They are
   the same bytes because they are the same array.
2. **The rules are patterns over shapes, not names**: `Authorization: Bearer …`
   and any `Bearer` token, `Cookie`/`Set-Cookie` headers, `document.cookie`
   contents, anything matching the session cookie's name, JWT-shaped
   `xxx.yyy.zzz` triples, `?token=`/`&key=`/`&signature=` query parameters
   (which is how a scoped storage URL leaks — `rustfs::scoped_write_policy`
   hands out presigned credentials), AWS/STS key shapes, PEM blocks, and the
   submitter's own email address as the client knows it (FR-013). Each is
   replaced with a visible marker — `[redacted: bearer token]` — never deleted,
   so the person sees that something was removed and what kind.
3. **The server validates and refuses; it never edits.** The submission
   mutation runs the *same* rule set over the approved payload. A match is a
   **refusal** (`FEEDBACK_CONTAINS_SECRET`) that names the kind and keeps the
   draft, not a silent rewrite. Refusing is what preserves the promise: an
   edit would mean the person saw something different from what was sent, which
   is the exact failure FR-012 exists to prevent. It also means a client
   tampered with to skip redaction cannot post a token through this path.

**What is deliberately not redacted**: the message the person typed. Spec.md's
Edge Cases settle it — "what a person deliberately types is theirs, and the
review step is where they see it" — and the server's refusal applies to
attachments, not to prose. A person who types their own password into the
message box is doing something the product should not silently rewrite.

**How the rules are kept honest**: the redaction rule set is shared between the
client filter and the server validator by being **generated from one list**.
`apps/web/src/services/feedbackRedaction.ts` and the server's validator both
read `config/feedback-redaction.json`, so a rule added on one side cannot
disagree with the other. A rule that exists on the server only would produce
refusals the person could not have anticipated; a rule that exists on the
client only would be no rule at all.

**Alternatives rejected**:

- *Redact on the server after approval.* Satisfies FR-012's letter. Breaks its
  point, and the checklist says so explicitly.
- *Redact only in the preview renderer.* Then the buffer still holds the token
  and the submitted payload is the buffer — the preview would be a lie in the
  helpful direction, which is the worst kind.
- *Server rewrites instead of refusing.* The person approved bytes; different
  bytes arriving is a broken promise even when the difference is an
  improvement.

---

## R7 — The screenshot comes from the browser's capture picker, not from the canvas

**What is there today**: no screenshot code and no capture library
(`html2canvas`, `toDataURL`, `getDisplayMedia`, `captureStream` — none appear
anywhere in `apps/web/src` or `apps/web/package.json`).

**The constraint that decides it**: the play field is a WebGL canvas that
**Bevy/winit inserts itself** — `apps/web/src/engine/bevy/useCanvasEngine.ts`
says so in a comment, and the React tree only ever queries for it. Nothing
configures its WebGL context, which means `preserveDrawingBuffer` is at its
default of `false`, and `canvas.toDataURL()` outside the drawing frame returns
a blank image. A screenshot built that way would silently produce an empty
rectangle where the map was — the failure mode that looks like it worked.

**This is not a guess; it is already recorded twice.**
`apps/web/e2e/canvas-authoring.spec.ts:623-637` documents exactly this blocker
for a future visual-regression setup, and `apps/engine-sandbox/src/main.ts:67`
repeats it. The only places `toDataURL` appears in this repository are
Playwright fixtures, and they are there because of it.

**Decision**: `navigator.mediaDevices.getDisplayMedia()`, one frame grabbed
from the returned track, drawn to an offscreen canvas, encoded as PNG, track
stopped immediately.

Three reasons beyond the technical one:

- **It composites everything the person can see** — the WebGL canvas, the React
  panels above it, the browser's own chrome if they pick a window — which is
  what FR-015 asks for ("produced from what the person can see").
- **The browser's picker *is* the consent step.** FR-008 requires that taking a
  screenshot is the person's choice; a native, non-spoofable dialog in which
  they choose *what* to share is a stronger form of that than a checkbox this
  product renders.
- **It works on every screen**, including the ones with no engine at all — the
  login page, the compendium, an admin screen — which spec.md's Edge Cases
  require.

Refusal, dismissal, or an unavailable API all resolve to the same thing: the
offer fails, a plain line says so, and submission continues (FR-008, and the
Edge Case "offering fails; submitting does not").

**The e2e consequence, and it is small**: `apps/web/playwright.config.ts`
already carries a `launchOptions.args` list for GPU flags. Capture needs two
more — `--use-fake-ui-for-media-stream` and
`--auto-select-desktop-capture-source=Entire screen` — added to that same
array, which is a harness change and not a product branch.

**Alternatives rejected**:

- *`canvas.toDataURL()` on the Bevy canvas.* Blank, for the reason above, and
  it would also omit every panel.
- *Set `preserveDrawingBuffer: true` on the engine's context.* A permanent
  per-frame cost on every session, paid by everyone, for a feature used
  occasionally — and it still would not capture the React chrome.
- *`html2canvas` or `dom-to-image`.* A new dependency that re-renders the DOM
  approximately, and which reaches the WebGL canvas by calling `toDataURL` —
  so it lands on the same blank rectangle by a longer route.
- *A Bevy-side screenshot command through the engine SDK.* Captures the canvas
  correctly and nothing else, works only where the engine is mounted, and puts
  a product feature inside the engine for a reason unrelated to simulation
  (Principle I).

---

## R8 — Attachments are stored unshared, so they can expire

**What is there today**: `src/server/src/storage/` is an S3/RustFS layer built
for one thing — canvas image assets. `rustfs::object_key` computes
`{owner}/{world}/{scene}/{asset}.webp` and its doc comment says the path is
"Never client-supplied". `dedupe::object_holding` reuses an existing object for
byte-identical content, instance-wide, and its module docs state the load-bearing
fact:

> **Nothing in this product deletes stored objects.** `storage/rustfs.rs` has
> no delete operation at all, which is what makes a shared path safe today: a
> reference cannot dangle when references are never dropped.
>
> … **Adding object deletion means adding reference counting first.**

**The conflict**: FR-016 requires attachment retention to be "stated and
bounded". Bounded retention means deletion, and deletion is the thing this
storage layer has never done.

**Decision**, and it is the narrowest resolution available:

1. Attachments are stored in RustFS — files go where this codebase puts files —
   under a **feedback-owned key prefix** computed server-side, never
   client-supplied, on the same rule as `object_key`:
   `feedback/{submission_id}/{attachment_id}.{webp|log}`. No world id and no
   scene id, because a submission from the login page has neither. The
   screenshot goes through the existing `transcode::transcode_to_webp`, so it
   inherits `MAX_UPLOAD_BYTES` and `TranscodeError::TooLarge { max, actual }`
   ("checked before any decode work") and lands as `.webp` like every other
   image this product stores. The log bundle is `text/plain` and is the
   storage layer's **first non-image content type** — every existing write
   passes `"image/webp"` — which is worth saying out loud because it is the
   line a future reader will trip over.
2. Feedback attachments are **never deduplicated**. `dedupe::object_holding` is
   not called for them and their content hash is not written into any shared
   lookup. One row, one object, no other referrer — which is exactly the
   precondition the dedupe module names for deletion being safe.
3. `rustfs::delete_object(cfg, key)` is added — the **first** delete path in
   this codebase — and it is called only by the retention sweep, only for keys
   under the `feedback/` prefix, and only for attachments whose submission is
   past its retention. The prefix restriction is enforced in the function, not
   by its callers.
4. **Retention**: 30 days from submission, swept by the same background task
   that delivers (one loop, two jobs, no second schedule). The instance's copy
   expiring has no bearing on the tracker's copy, which is FR-016's second half
   and is stated to the person before they submit (FR-014).

Because deletion now exists in this codebase, `dedupe.rs`'s warning becomes
live for anyone who later widens it. Its module docs are updated in the same
change set to say deletion exists, that it is confined to a prefix nothing
dedupes, and that extending it to canvas assets is the change that requires the
reference counting it describes.

**Alternatives rejected**:

- *Store attachments as `bytea` in Postgres.* Bounded retention becomes a
  `DELETE` this codebase already knows how to write — genuinely simpler — but
  it puts megabyte blobs in the row store, in the same database the play field
  reads on every tick, for content that is a file by every other definition in
  this product.
- *Dedupe them like canvas assets.* Two submissions with an identical
  screenshot would share an object; expiring one would blank the other's
  evidence, in a submission nobody touched. Precisely the failure `dedupe.rs`
  predicts.
- *Never delete, and state retention as "indefinite".* Honest, and it makes
  FR-016's "bounded" false. A screenshot of somebody's table kept forever
  because deletion was inconvenient is not a defensible privacy position.

---

## R9 — Rate limiting: in the resolver, on the share limiter's shape

**What is there today**, two limiters with a documented reason for being two:
`auth_middleware.rs` keys on the request **path** and returns early unless it
contains `/authentication/`, and `graphql/share_rate_limit.rs` exists because
of that — "every GraphQL operation in this product arrives at one path, so
extending it to cover `/graphql` would rate-limit the entire application
against a threshold written for password attempts. This one sits inside the
resolver, where the operation is known." It is a sliding window in a
`OnceLock<Mutex<HashMap<String, Vec<i64>>>>`, `MAX_REQUESTS = 30` per
`WINDOW_SECONDS = 60`.

**Decision**: a third limiter of the same shape, in
`src/server/src/graphql/feedback_rate_limit.rs`, keyed on the **account id**
(FR-006 says per account, and the submitter is signed in by assumption), at
**5 submissions per 10 minutes**. It ignores
`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` for the reason `share_rate_limit.rs`
already argues: the harness sets that variable on every run, so honouring it
would switch the limiter off during the one test written to prove it works —
"a limiter nobody tests is a limiter nobody has." That module pins the rule
with a test named `the_e2e_auth_bypass_does_not_disable_this_limiter`, and this
one gets its own copy of it. (The variable is in any case
`#[cfg(debug_assertions)]`-gated in `auth_middleware.rs` — a release build
contains no bypass path at all — so honouring it would be a debug-only hole
in a privacy-adjacent control.)

**One deliberate difference from the share limiter**: it returns an
`extensions.code` of `FEEDBACK_RATE_LIMITED`. `share_rate_limit` returns a bare
`async_graphql::Error` with no code, which is fine for four resolvers whose
only response is to show the message. Here the client must distinguish "wait
and keep your draft" from every other failure, and `graphqlClient.ts` already
surfaces `codes: string[]` on `GraphQLRequestError` for exactly that —
matching on the human-readable message would break the first time it is
reworded, which the client's own comments about `ALREADY_TAKEN` already say.

**The half that is not the limiter**: FR-006's second clause — a refusal MUST
NOT discard what the person wrote — is a client property. The refusal arrives
as an error code, the dialog stays open with the draft intact, and it says when
they may try again. That is a specific e2e assertion, not a side effect.

**Alternatives rejected**:

- *Reuse the auth limiter.* Wrong key (path), wrong threshold, and disabled in
  the harness.
- *A database-backed counter.* The two existing limiters are in process and
  inherit the same single-process boundary the peer registry and presence
  already have. Feedback does not need to be the feature that fixes it.

---

## R10 — The submitter is a reference at the destination, never an address

**FR-013**: the submitter's email must not reach the destination, and a
submission must still be traceable back to its submitter *inside* the instance.

**Decision**: the issue body carries a **submitter reference** — the
submission's `id`, and nothing else about the person. The mapping from that id
to a `user_id` is a column in `feedback_submissions`, which is inside the
instance, behind the same authorization boundary as any other row. A maintainer
reading the issue can ask an operator "who is submission `018f…`?"; the tracker
cannot answer it, and neither can anyone who reads the repository.

The email address is also on R6's redaction list, so it cannot arrive through
a log line either — the two rules cover the two routes it could take.

**Alternatives rejected**:

- *A username.* Public in the issue, and on this product a username is often
  how somebody is known elsewhere.
- *A hash of the email.* A hash of a low-entropy value is a lookup table away
  from the value. It reads as privacy without being it.
- *Nothing at all.* FR-013's second half needs a handle, and the alternative to
  a reference is a maintainer who cannot ask a follow-up question — which is
  SC-009 pointing the other way.

---

## R11 — Destination visibility is read from the host, never assumed

**What is there today**: `repo_host::repository_is_public(installation_id,
owner, name)` already exists and already answers this. `lore_sync` stores its
answer as `repository_is_public` alongside `visibility_checked_at`, with a
migration comment that is exactly right for this feature too: "Observed at the
last run, not guaranteed: visibility changes at the host without telling us, so
anywhere this is shown must say when it was last seen."

**Decision**: the feedback form's pre-submission notice (FR-014) states the
destination's visibility from that call, cached on the destination record with
its observation time, refreshed by the delivery task, and rendered **with** the
observation time. When the answer is unknown — never checked, or the host is
unreachable — the notice says so and treats it as public for the purposes of
what it warns about, because that is the assumption that fails safe.

**Alternatives rejected**:

- *Assume public always.* Wrong for the self-hosted operator with a private
  repository, and spec.md forbids assuming either way.
- *Ask the operator to declare it.* A declaration goes stale silently, which is
  the failure the migration comment above is written about.

---

## R12 — There is no application version, and FR-009 requires one

**What is there today**: nothing surfaced. `grep -rn
"CARGO_PKG_VERSION\|__APP_VERSION__\|appVersion"` across `src/server/src`,
`apps/web/src` and `apps/web/vite.config.mts` returns no hits. `vite.config.mts`
has no `define` block at all; the only `import.meta.env` reads are
`VITE_SITE_URL`, `VITE_DEFAULT_OG_IMAGE` and `import.meta.env.DEV`. Versions
*exist* (`apps/web/package.json` says `1.0.0`, `src/server/Cargo.toml` says
`0.1.0`) and nothing reads them. The GraphQL `healthcheck` query returns a bare
`true`.

**Where it must not go**: `/api/status`. Its handler
(`src/app/src/main.rs:153`) is documented as "Deliberately reports only a
stable `key` per subsystem (never a real hostname, image tag, or connection
string)", and it is unauthenticated. A build identifier is exactly an image
tag, and volunteering it to anonymous callers would undo a decision somebody
made on purpose.

**Decision**: this feature creates the version surface it needs, on an
**authenticated** GraphQL query, because a report that cannot say which build
produced it is a report that costs the afternoon this feature exists to save.
Two small pieces: the server exposes `env!("CARGO_PKG_VERSION")` and a short
git SHA (`option_env!("THUNDERFORGE_GIT_SHA")` — absent in a plain
`cargo build`, present in CI, and rendered as "unknown build" rather than
omitted when absent) behind authentication; the web build defines the same pair
through a new `define` block in `vite.config.mts`. The client sends its own;
the server records its own alongside it, because a stale cached bundle against
a new server is a real and diagnosable situation that only shows up if both are
captured.

**Recorded as a scope note**: this is arguably spec 040's territory — it is
instance-level truth — but 040 owns *configuration*, and a build identifier is
not configured, it is compiled in. It is claimed here because FR-009 names it.

**Alternatives rejected**:

- *Send the User-Agent and call it context.* It identifies the browser, which
  FR-009 asks for separately, and says nothing about which ThunderForge build
  is running.
- *Wait for spec 040.* FR-009 is a P1 requirement of this feature and 040 is
  not being built yet.

---

## R13 — The draft survives in the tab, not on the server

**FR-005**: an unsent draft survives the form being dismissed and reopened
**within the same session**.

**Decision**: the draft — kind, message, and which attachments are ticked, but
**not** the attachment bytes — is held in `sessionStorage` under one key,
written on change (debounced), cleared on successful submission. Not
`localStorage`, because "within the same session" is what the requirement says
and a draft that outlives the tab is an unasked-for record of what somebody was
about to report, on a machine that may be shared.

The screenshot bytes and the log snapshot are **not** stored, on R5's
reasoning. Reopening the form re-offers the screenshot and re-reads the live
buffer, which is also more useful: the logs at reopen include whatever happened
since.

**Where the context comes from, since there is nowhere to read it**: there is
**no `WorldContext` and no `useWorld` hook** in this app. `worldId` comes from
`useParams()` and is threaded as a prop (`WorldPage.tsx:173`), and the world
record — including `gameSystemId` — is local `useState` inside `WorldPage`
(`:241`). A launcher mounted globally cannot reach either. So FR-009's context
is assembled from what a global component *can* see: `useLocation()` for the
screen, `useParams()` for `worldId` when the route has one, `useAuth()` for the
account, R12's two versions, and `navigator.userAgent` for the browser. The
game system is resolved **server-side** from `worldId` in the mutation, which
is better than plumbing it anyway — the server does not have to trust the
client for a fact it can look up, and a submission from a screen with no world
simply carries neither (spec.md's first Edge Case).

**Alternatives rejected**:

- *A server-side draft.* A round trip and a table for something that must not
  outlive a tab, and it would make an unsent draft a thing the instance holds
  about a person.
- *`localStorage`.* Survives the session, which contradicts the requirement and
  creates the shared-machine problem R5 avoided.
- *Introducing a `WorldContext` so the launcher can read the system.* A
  refactor of every world surface, in service of a field the server can derive
  from an id it already receives.

---

## R14 — Reading state back is a poll on the pass that already runs

**US5 / FR-022**: a submitter sees the current state of what they sent. The
spec's assumption is narrow and worth keeping narrow: "'manages your issues'
means reading state back and, where a maintainer acts, reflecting it — not
editing issues on the submitter's behalf."

**Decision**: the delivery task, on the same tick, refreshes the state of
delivered submissions whose issue is still recorded as open — one
`GET /repos/{owner}/{name}/issues/{number}` per submission, oldest-refreshed
first, bounded per tick, and only for submissions younger than the retention
window. `open` and `closed` are the only two states stored, because they are
the only two the tracker has and inventing a third would be this product
guessing at a maintainer's meaning.

**Alternatives rejected**:

- *Webhooks.* They require the instance to be reachable from the internet,
  which a self-hosted table behind a home router is not, and they need a
  secret, an endpoint and a signature check — a second delivery mechanism to
  keep correct so that a status label updates sooner.
- *Fetch on view.* The person opening their list pays a GitHub round trip per
  item, and an unreachable host makes their own list fail to load.
- *Reflect labels, assignees and comments.* Out of scope by spec.md, and each
  is a thing a maintainer wrote for maintainers.

---

## R15 — Attachments at the destination: the body carries the logs, the repository carries the image

**The problem**: US3 scenario 2 asks that the logs and any screenshot be "part
of the issue and can be moved around and worked with as ordinary issue
content". GitHub's REST API creates an issue from a title and a body and
**has no supported attachment-upload endpoint** — the one the web UI uses is
undocumented and unavailable to an App. So "attach" has to mean something
concrete, and it means two different things for the two attachment kinds.

**Decision (logs — inline)**: the redacted log bundle goes into the issue body
inside a collapsed `<details>` block wrapping a fenced code block. An issue
body is capped at **65,536 characters**, so the body carries at most **48 KB**
of log and states plainly how many entries and bytes were withheld — the same
sentence FR-011 already requires for the ring buffer's own eviction, now
answering a second question with one mechanism. The complete bundle is
delivered by the path below.

**Decision (screenshot, and the full log file — committed)**: both are written
into the destination repository through the Contents API
(`PUT /repos/{owner}/{name}/contents/{path}`) on a dedicated
**`feedback-attachments`** branch, at `attachments/{submission_id}/{name}`, and
the issue body links them. This needs no new permission:
`github::REQUESTED_PERMISSIONS` already asks for `contents:write` (for lore
sync) alongside `issues:write`, so an existing installation covers it.

**How the body links them is decided by R11's visibility, not by a guess**:

- **Public repository** — embedded as an image,
  `![screenshot](https://raw.githubusercontent.com/{owner}/{name}/feedback-attachments/{path})`,
  which renders inline in the issue and is "ordinary issue content" in the
  fullest sense.
- **Private repository** — a plain link to the blob page,
  `https://github.com/{owner}/{name}/blob/feedback-attachments/{path}`, because
  `raw.githubusercontent.com` requires a token for a private repository and an
  embedded image would render as a broken icon for every maintainer. A link
  that works is better than an image that does not.

The same visibility fact therefore serves three purposes — what the person is
warned about before submitting (FR-014), which link form is written, and what
the operator's view reports — which is why R11 stores it rather than asking
each time.

**A branch, not the default branch**, so feedback attachments never appear in
the history of the code, never trigger a CI run, and can be pruned by a
maintainer independently of anything else. The branch is created on first use
from the repository's default branch head.

**The residual honesty**: FR-014 says the person is told that what is sent
cannot be recalled by the instance. Committing to a repository is the strongest
possible form of that being true — a commit persists in the branch's history
even after a later delete — and it is the reason FR-014's notice is a
requirement rather than a nicety. The notice says "committed to a repository
and permanent there", not "sent".

**Alternatives rejected**:

- *Link back to the instance's own attachment route.* Every asset route in this
  product is wrapped in `require_authenticated_user` (`src/app/src/main.rs:547-562`),
  so a maintainer on GitHub would get a login page for an account on somebody
  else's self-hosted instance. It also makes the evidence disappear when the
  instance's 30-day retention (R8) expires, which is the opposite of FR-016's
  "independent of what the destination does with its copy".
- *A `data:` URI in the body.* GitHub's markdown renderer does not resolve them.
- *Put the whole log in the body and skip the file.* A chatty session exceeds
  65,536 characters, and the truncated half is frequently the interesting half.
- *Upload through the undocumented web attachment endpoint.* Not available to
  an App, not supported, and a dependency on an interface that can vanish
  without notice.

---

## What no decision was needed for

- **Credential collection and editing.** Spec 040 owns it (spec.md's
  Configuration preamble). This plan consumes the environment form and the
  resolution rule; it does not build a settings screen, and R1's `sources`
  vector is deliberately the shape 040's FR-021 needs.
- **Anonymous submission.** Out of scope by spec.md's Assumptions.
- **Which repository.** One per instance, by assumption. It is a destination
  record, not a per-world choice.
