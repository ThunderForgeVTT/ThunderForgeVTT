# Phase 1 Data Model: In-App Feedback

Four durable tables, one resolved-not-stored value, and one thing that is
deliberately in memory and nowhere else. The line between them is the design:
**the submission is durable because FR-018 says the instance must not lose it,
and the log buffer is not durable because FR-012 and R5 say nothing should be
able to find it later.**

Everything below lands in one migration,
`src/server/migrations/2026-09-07-010000-0000_feedback_submissions/`, following
the `YYYY-MM-DD-HHMMSS-NNNN_snake_case` convention the directory already uses.

---

## 1. `feedback_submissions` — what a person sent

The row exists before any delivery is attempted. That single ordering is
FR-018, and it is what makes FR-030 (an unconfigured instance still collects
feedback) possible rather than aspirational.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | `Uuid::now_v7()`, so ordering by id is ordering by time |
| `user_id` | `UUID NOT NULL REFERENCES users(id)` | FR-013's "traceable *inside* the instance". Never sent anywhere |
| `kind` | `TEXT NOT NULL` | `CHECK (kind IN ('feature_request','issue','general'))` — FR-002's exactly three, enforced by the database rather than by a resolver |
| `message` | `TEXT NOT NULL` | What the person typed. Never redacted (spec.md Edge Cases) |
| `summary` | `TEXT` | The one-line title, where the kind asks for one (FR-003) |
| `screen_path` | `TEXT` | The route, e.g. `/world/:id/compendium`. Nullable — a submission from an error state may have none |
| `world_id` | `UUID REFERENCES worlds(id) ON DELETE SET NULL` | Nullable. The login page has no world (Edge Cases). `SET NULL` because a deleted world must not delete a report about it |
| `game_system_id` | `TEXT` | Resolved **server-side** from `world_id`, never accepted from the client (R13) |
| `client_version` | `TEXT NOT NULL` | R12. `"unknown build"` rather than absent |
| `server_version` | `TEXT NOT NULL` | Recorded alongside, because a stale bundle against a new server is a real situation and invisible unless both are captured |
| `browser` | `TEXT NOT NULL` | Coarse, as spec 035's access events are coarse: family and platform, never the full User-Agent string and never an address |
| `delivery_state` | `TEXT NOT NULL DEFAULT 'pending'` | `CHECK (delivery_state IN ('pending','delivered','abandoned'))` |
| `delivery_key` | `UUID NOT NULL UNIQUE` | R4. Written into the issue body; the handle a duplicate would be found by |
| `issue_url` | `TEXT` | Set when delivered. Also set when an ambiguous retry *adopts* an issue it finds |
| `issue_number` | `INTEGER` | What US5's state refresh reads back |
| `issue_state` | `TEXT` | `CHECK (issue_state IS NULL OR issue_state IN ('open','closed'))` — the only two the tracker has (R14) |
| `issue_state_checked_at` | `TIMESTAMP` | Shown with the state, for the same reason `visibility_checked_at` is |
| `attachments_expire_at` | `TIMESTAMP NOT NULL` | Submission time + 30 days. A column, not a computation, so the retention a person was promised is the retention that was in force when they submitted — changing the constant later cannot retroactively shorten it |
| `attachments_purged_at` | `TIMESTAMP` | Set by the sweep. Distinguishes "expired and removed" from "expired and the sweep has not run", which a person asking where their screenshot went deserves |
| `created_by` / `updated_by` | `UUID NOT NULL REFERENCES users(id)` | Principle III / ADR-009. Both are the submitter |
| `created_at` / `updated_at` | `TIMESTAMP NOT NULL DEFAULT NOW()` | |

```sql
CREATE INDEX feedback_submissions_mine ON feedback_submissions (user_id, created_at DESC);
CREATE INDEX feedback_submissions_pending ON feedback_submissions (delivery_state, created_at)
    WHERE delivery_state = 'pending';
CREATE INDEX feedback_submissions_expiring ON feedback_submissions (attachments_expire_at)
    WHERE attachments_purged_at IS NULL;
```

The two partial indexes are the delivery pass and the retention sweep; each is
one query per tick and neither scans delivered history.

### State transitions

```text
                     ┌─── delivery succeeds ──────────> delivered ──┐
                     │                                              │
recorded ──> pending ┼─── attempt fails ──> pending (backoff)       ├─> attachments purged
                     │                                              │   (30 days, either state)
                     └─── operator abandons ─────────> abandoned ───┘
```

- **`recorded` is not a state**, it is the instant the transaction commits.
  There is no window in which a submission exists without being `pending`.
- **`abandoned`** is FR-019's "or is abandoned by an operator" and is the only
  transition a person makes rather than the pass. It is reversible back to
  `pending`, because an operator who fixed the credential should not have to
  ask the submitter to send it again.
- **Purging attachments is orthogonal to delivery.** A submission delivered on
  day one and one still pending on day thirty both lose their bytes on day
  thirty; the pending one's delivery then fails permanently and says so, which
  is honest and is why the operator view (FR-021) exists.

---

## 2. `feedback_attachments` — the evidence, one row per file

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `submission_id` | `UUID NOT NULL REFERENCES feedback_submissions(id) ON DELETE CASCADE` | |
| `kind` | `TEXT NOT NULL` | `CHECK (kind IN ('logs','screenshot'))` |
| `storage_path` | `TEXT NOT NULL` | `feedback/{submission_id}/{id}.{webp\|log}` — computed server-side, never client-supplied, on the same rule `rustfs::object_key` states |
| `content_type` | `TEXT NOT NULL` | `image/webp` or `text/plain`. The latter is this storage layer's first non-image object |
| `byte_size` | `BIGINT NOT NULL` | What FR-011's "what was kept" is reported from |
| `entries_kept` | `INTEGER` | Logs only. FR-011 |
| `entries_dropped` | `INTEGER` | Logs only. A number, because "some were dropped" is not an answer |
| `redaction_count` | `INTEGER NOT NULL DEFAULT 0` | How many markers the client's filter left. Shown in the review, and asserted against in SC-004's test |
| `purged_at` | `TIMESTAMP` | Set when the object is deleted. The row survives its bytes, so the issue can still say what was attached |
| `created_at` | `TIMESTAMP NOT NULL DEFAULT NOW()` | |

**No `content_hash`, and no dedupe lookup.** This is the load-bearing
difference from `canvas_image_assets`, and research.md § R8 is where it is
argued: `storage/dedupe.rs` states that "adding object deletion means adding
reference counting first", and the reason feedback can delete without it is
that **no second row can ever name a feedback object**. Omitting the hash
column is what makes that checkable rather than promised — there is no column
by which a shared path could be introduced.

**Provenance**: no `created_by`/`updated_by`. An attachment is owned entirely
by its submission, which carries both, and `ON DELETE CASCADE` means it has no
independent life. This matches the convention the codebase uses for rows that
are parts of a parent rather than entities.

---

## 3. `feedback_delivery_attempts` — one try, retained

Modelled directly on `lore_sync_runs`, and for the reason that table's own
migration gives: "Retained rather than overwritten, because FR-030's backoff
and FR-029's 'notify once rather than repeatedly' are both statements about a
HISTORY of attempts. A single mutable status column cannot express either."
FR-019 and FR-021 are the same two statements here.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `submission_id` | `UUID NOT NULL REFERENCES feedback_submissions(id) ON DELETE CASCADE` | |
| `attempt` | `INTEGER NOT NULL DEFAULT 1` | Drives the backoff index, as `lore_sync_runs.attempt` does |
| `started_at` | `TIMESTAMP NOT NULL DEFAULT NOW()` | |
| `finished_at` | `TIMESTAMP` | **NULL means in flight.** This is the single-flight mechanism (R4) — `due_now` refuses a submission whose latest attempt has not finished, exactly as `lore_sync`'s selection refuses a connection whose latest run has a null `outcome` |
| `outcome` | `TEXT` | `CHECK (outcome IS NULL OR outcome IN ('created','adopted','failed','not_configured'))` |
| `failure_reason` | `TEXT` | **In terms an operator can act on, never a raw host body.** FR-021 forbids any credential or fragment of one here, and the delivery code maps host errors to a fixed vocabulary rather than passing text through |
| `http_status` | `INTEGER` | Nullable — absent is exactly the ambiguous case R4 is about, and recording its absence is what makes the next attempt search first |

```sql
CREATE INDEX feedback_delivery_attempts_submission
    ON feedback_delivery_attempts (submission_id, started_at DESC);
```

`outcome = 'adopted'` is worth its own value rather than folding into
`'created'`: it records that a retry found an issue a previous ambiguous
attempt had already made, which is the evidence that FR-019 held under the one
condition that could break it.

---

## 4. `feedback_destination` — where issues go, and how visible it is

At most one row. The instance's destination, not a world's.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `installation_ref` | `TEXT NOT NULL` | The App installation. Read only at the host boundary, exactly as `lore_repository_connections.installation_ref` is, and for the same reason |
| `repository_ref` | `TEXT NOT NULL` | `owner/name` |
| `attachment_branch` | `TEXT NOT NULL DEFAULT 'feedback-attachments'` | R15 |
| `is_public` | `BOOLEAN` | **Observed, never assumed** (FR-014, R11). NULL means never checked |
| `visibility_checked_at` | `TIMESTAMP` | Rendered *with* the answer, because visibility changes at the host without telling us — the sentence `lore_repository_connections` already carries |
| `created_by` / `updated_by` / `created_at` / `updated_at` | | Principle III |

**No credential column, deliberately**, matching
`lore_repository_connections`: the installation token is short-lived and
derived per call, so this table is safe to read in full when diagnosing a
destination. That is the property FR-021 depends on — an operator view that
cannot leak a secret because there is no secret in the rows it reads.

---

## 5. Resolved GitHub App credentials — not stored, and the source is the point

```text
ScopedApp {
    app:     GitHubApp,                     // parsed at configuration time (FR-028)
    scope:   AppScope,                      // Global | Sync | Feedback
    sources: Vec<(Field, &'static str)>,    // e.g. (Slug, "GLOBAL_GITHUB_APP_")
}
```

Resolved per call from the environment by
`repo_host::registration_for(scope)`, never persisted. `sources` is the whole
reason this is a struct rather than a tuple: **FR-029 requires an operator to
see which value came from where**, and a half-configured feedback app silently
completed from the global one is the failure that requirement names. Recording
the prefix per field makes "silently" impossible — the operator surface renders
the vector.

The **key is parsed here, not at first use** (FR-028), because that is what
`repo_host.rs` already does and why: "a key that is present but not a key is
the failure a presence check calls configured."

**Diagnostics carry a `RegistrationProblem`, never a value.** The existing enum
already satisfies FR-027 — every variant names a variable and none carries a
key, a fragment or a length — and gains a scope so the message says which
prefix it is complaining about.

---

## 6. The log ring buffer — in memory, and nowhere else

```text
FeedbackLogBuffer (module-scoped, apps/web/src/services/feedbackLogBuffer.ts)
  entries:        LogEntry[]   // capped: 500 entries OR 128 KB, whichever first
  droppedCount:   number
  droppedOldestAt: number | null

LogEntry { at: number, level: 'error'|'warn'|'info', text: string }  // text ≤ 2 KB
```

- **Redacted at push, never after** (FR-012, R6). The array cannot hold a
  secret at any instant, so the review and the submission are the same bytes
  because they are the same array.
- **Never persisted.** Not `localStorage`, not `sessionStorage`, not IndexedDB,
  not OPFS. Closing the tab destroys it; a shared machine inherits nothing.
- **`droppedCount` is not cosmetic** — FR-011 requires the person to be told
  what was kept when something is dropped for size, and that needs a number.
- Per-entry truncation at 2 KB exists so that one enormous line cannot consume
  the whole budget and evict everything that explained it.

**This is deliberately not a table.** A durable log buffer would be a store of
other people's screens, obtainable later, on a machine that may be shared —
the privacy cost of persisting it is larger than the diagnostic value of
surviving a reload, and R13's draft survival covers the case a reload actually
creates.

---

## Entity relationships

```text
User ──1:N──> FeedbackSubmission ──1:N──> FeedbackAttachment ──1:1──> stored object
                     │                          (never shared, therefore deletable)
                     ├──1:N──> FeedbackDeliveryAttempt
                     ├──0:1──> World  (SET NULL; a submission outlives its world)
                     └──N:1──> FeedbackDestination ──> GitHub issue

AppScope ──resolves──> ScopedApp   (per call, never stored, carries its sources)
Browser tab ──holds──> FeedbackLogBuffer   (memory only, dies with the tab)
```

## Validation rules, traced to requirements

| Rule | Requirement |
|---|---|
| A submission carries exactly one of three kinds, enforced by a CHECK | FR-002 |
| A refused submission (rate limit, secret found) leaves the draft intact client-side | FR-006, FR-012 |
| A log line is redacted before it enters the buffer | FR-012 |
| The server refuses an approved payload containing a secret; it never rewrites one | FR-012 |
| The submitter's email appears in no attachment and in no issue body | FR-013, SC-004 |
| Nothing is attached that the person did not see in the review | FR-010, SC-003 |
| Captured evidence is bounded and the person is told the counts | FR-011 |
| The row is committed before any delivery call is made | FR-018 |
| A submission whose latest attempt has `finished_at IS NULL` is not selected | FR-019 |
| An ambiguous retry searches for `delivery_key` before creating | FR-019 |
| Each kind maps to a distinct destination label | FR-020 |
| `failure_reason` is drawn from a fixed vocabulary, never a host body | FR-021, FR-027 |
| `mySubmissions` filters on `user_id` and exposes no other account's rows | FR-022 |
| The specific prefix wins per field; the global fills the rest; `sources` records both | FR-025, FR-029 |
| The private key is parsed when configured, not at first use | FR-028 |
| With no destination row and no credentials, submission still succeeds | FR-030 |
| Attachment objects are deleted at `attachments_expire_at` and only under `feedback/` | FR-016 |
| Destination visibility is read from the host and shown with its observation time | FR-014 |

## Known boundaries, inherited not introduced

- **The rate limiter is per process**, as `share_rate_limit.rs` and the auth
  limiter already are. A multi-process deployment weakens all three together;
  this feature does not make it worse and does not fix it.
- **The delivery pass is per process.** Two servers on one database would both
  select the same pending submission. The single-flight guard is the
  `finished_at IS NULL` state, which narrows the window but does not close it —
  `lore_sync` has the identical exposure and R4's `delivery_key` search is what
  keeps the consequence to a findable duplicate rather than a silent one.
- **Search indexing at the host is not immediate**, so R4's adoption lookup can
  miss an issue created seconds earlier. The first backoff step is thirty
  seconds for that reason, and the residual case is recorded in research.md
  rather than claimed away.
