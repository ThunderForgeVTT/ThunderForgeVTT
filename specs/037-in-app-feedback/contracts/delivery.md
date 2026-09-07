# Contract: Delivery — the pass, the issue, and exactly once

The instance holds the submission; GitHub is where a copy of it goes. This
contract covers the pass that carries it, what it produces, and what "exactly
once" can honestly mean against a host that offers no idempotency key.

---

## 1. The pass

`src/server/src/feedback/schedule.rs`, spawned from `src/app/src/main.rs`
beside `spawn_lore_sync_task`, and modelled on it deliberately rather than
incidentally.

```rust
pub const TICK_SECONDS: u64 = 30;
const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600];

pub fn next_attempt_after(last_attempt: NaiveDateTime, attempt: i32) -> NaiveDateTime;

pub struct Due { pub submission_id: Uuid }
pub fn due_now(conn: &mut PgConnection, now: NaiveDateTime) -> Result<Vec<Due>, String>;

pub fn spawn_feedback_delivery_task(state: crate::AppState);
```

**`due_now` is a separate, pure, testable function** for the reason
`lore_sync/schedule.rs` gives about its own: a rule enforced by an `if` inside
a loop inside a spawned task is a rule nothing can test. Every exclusion below
is one assertion against a database.

A submission is due when **all** of:

| Condition | Requirement |
|---|---|
| `delivery_state = 'pending'` | FR-018 |
| its latest attempt has `finished_at IS NOT NULL` (or there is none) | FR-019 — an in-flight attempt is not due, exactly as `lore_sync` refuses a connection whose latest run has a null `outcome` |
| `now >= next_attempt_after(latest.started_at, latest.attempt)` | FR-019's backoff |
| `attachments_purged_at IS NULL` | Delivering a submission whose evidence has expired would produce a report without the thing it was for |

Ordered by `created_at ASC`, so a busy instance starves nobody — without an
order the database is free to return the same few rows every tick.

**Spawned unconditionally**, even with no destination configured, for the
reason `main.rs` already gives about the lore sync task: gating the spawn on
configuration would mean an operator who configures the feature has to restart
to use it, which is a worse trade than a query that finds no rows.

**The same tick does two other jobs**, so there is one schedule and not three:
the retention sweep (`attachments.md` § 5) and US5's state refresh
(`research.md` § R14), each bounded per tick.

### The backoff ends

`BACKOFF_SECONDS` terminates at an hour rather than growing forever, copying
`lore_sync`'s reasoning verbatim: a submission that has failed nine times is
failing in a way retrying will not fix, and hourly is the honest floor — often
enough to recover on its own when a host comes back, rare enough not to be
noise. An operator has been able to see it since the first failure (FR-021).

---

## 2. One attempt

`src/server/src/feedback/deliver.rs`. An attempt writes a
`feedback_delivery_attempts` row with `finished_at` NULL, does the work, and
finishes the row. There is no path that does work without a row.

```text
1. Resolve credentials  → registration_for(AppScope::Feedback)
     no destination row, or unresolvable credentials
       → finish: outcome = 'not_configured', failure_reason names the variables
         (never a value), submission stays pending. FR-030.

2. If the previous attempt was AMBIGUOUS (transport error, or 5xx, or no status):
     search  GET /search/issues?q=<delivery_key>+repo:<owner>/<name>
       found → finish: outcome = 'adopted', record issue_url/number,
                submission → delivered.   FR-019.
     not found → continue to 3.

3. Attachments (R15), for each approved attachment:
     PUT /repos/{owner}/{name}/contents/attachments/{submission_id}/{name}
       on branch `feedback-attachments`, created from the default head on
       first use.

4. Create   POST /repos/{owner}/{name}/issues
     { title, body, labels }
       2xx  → finish: outcome = 'created', submission → delivered
       4xx  → finish: outcome = 'failed', http_status recorded.
              NOT ambiguous — the host declined and created nothing, so the
              next attempt does NOT search.
       5xx or transport error
            → finish: outcome = 'failed', http_status recorded or NULL.
              AMBIGUOUS — the next attempt searches first.
```

### `open_issue` must learn to read a status code

Today `repo_host::open_issue` never checks `response.status()`. It parses the
body and infers failure from a missing `html_url`. For lore sync's occasional
disassociation notice that is survivable; for a retry loop it is not — a 403
secondary rate limit and a 422 validation error are permanently different
situations, and a loop that cannot tell them apart either hammers the host or
abandons a submission that would have succeeded. FR-019 and FR-021 both need
the code, so it is added.

---

## 3. What arrives at the destination

**Title**

```text
[Issue] Tokens vanish when the scene changes
[Feature] A way to reorder the initiative list
[Feedback] Thanks for the compendium search
```

**Labels** (FR-020 — "distinguishable as its kind without reading the body"):
`feedback` on every issue, plus exactly one of `feedback:issue`,
`feedback:feature-request`, `feedback:general`. Labels are set on creation, in
the same POST, so a failure between creating and labelling cannot exist. A
label GitHub does not have is created by that POST implicitly; if the host
rejects an unknown label the attempt falls back to `feedback` alone and records
that it did, because a delivered report with one label beats a lost report with
three.

**Body**

```markdown
<the person's message, verbatim>

---

### Context

| | |
|---|---|
| Screen | `/world/…/compendium` |
| World | Ashfall (dnd5e) |
| App | web 1.4.2 (a1b2c3d) · server 1.4.2 (a1b2c3d) |
| Browser | Chrome on Linux |
| Submitted | 2026-09-07T14:02:11Z |
| Submitter | `018f3c…` — an instance reference, not an identity |

### Screenshot

![screenshot](https://raw.githubusercontent.com/…/feedback-attachments/…)
<!-- or, for a private repository, a plain link to the blob page -->

<details><summary>Browser logs — 214 entries kept, 61 dropped for size, 3 redactions</summary>

```
2026-09-07T14:01:58Z ERROR GraphQL request failed (rollDice): …
…
```

</details>

Full log: [attachments/018f3c…/logs.log](…)

<!-- thunderforge-feedback: 018f3c9a-… -->
```

Rules for the body:

1. **The trailing `delivery_key` comment is load-bearing** (R4). It is what a
   retry searches for and what makes a duplicate, if one ever occurs, findable
   rather than silent. It is the same technique `lore_sync/binding.rs` uses to
   recognise its own issue by reading it back.
2. **The submitter is a reference, never an address** (FR-013). The submission
   id, and nothing else about the person. An operator can map it back; the
   tracker cannot, and neither can anyone who reads the repository.
3. **The inline log block is capped at 48 KB**, against a 65,536-character body
   limit, and the summary line states what was withheld. The complete bundle is
   the committed file, so nothing is lost by the truncation.
4. **The image is embedded for a public repository and linked for a private
   one**, decided from the visibility this instance actually observed (R11,
   R15). `raw.githubusercontent.com` needs a token for a private repository, and
   a link that works beats an image that renders broken for every maintainer.

---

## 4. Exactly once, stated honestly

FR-019 forbids duplicates after a retry. GitHub's issue endpoint accepts no
idempotency key, and the request that produces a duplicate is the one that
**succeeded at the host and whose response never arrived** — indistinguishable,
from here, from one that never arrived at all.

Three mechanisms, in order:

1. **A `delivery_key` visible at the destination**, generated at record time,
   never reused.
2. **Search before create, but only after an ambiguous failure.** A 4xx never
   triggers the search, because a 4xx means the host declined and created
   nothing.
3. **Single-flight by state**, in the database: `finished_at IS NULL` means in
   flight and not due.

**The residual case, recorded rather than claimed away**: search indexing at
GitHub is not immediate, so an attempt retried seconds after an ambiguous
failure can miss its own issue. This is why the first backoff step is thirty
seconds rather than immediate, and why the key is one we control — a duplicate,
if one occurs, carries the same `delivery_key` as its twin and is therefore
findable and closable. That is what "exactly once" can honestly mean here, and
saying so is better than a guarantee that quietly is not one.

---

## 5. The operator's view

```graphql
type UndeliveredFeedback {
  id: UUID!
  kind: FeedbackKind!
  createdAt: String!
  attemptCount: Int!
  lastAttemptAt: String
  "From a fixed vocabulary. Never a host body, never a credential or a fragment."
  lastFailureReason: FeedbackFailureReason
  nextAttemptAfter: String
  "The instance's copies expire here, delivered or not."
  attachmentsExpireAt: String!
}

enum FeedbackFailureReason {
  NOT_CONFIGURED           # no destination, or credentials do not resolve
  CREDENTIALS_REJECTED     # the host refused the assertion or the installation
  DESTINATION_NOT_FOUND    # the repository is gone, renamed, or the install removed
  PERMISSION_REFUSED       # the installation lacks issues:write or contents:write
  HOST_UNAVAILABLE         # 5xx or transport failure
  HOST_RATE_LIMITED        # 403/429 with a rate-limit signal
  REJECTED_BY_HOST         # 4xx the instance cannot act on
  ATTACHMENTS_EXPIRED      # the evidence was purged before delivery succeeded
}

extend type Query {
  "Administrators only. Everything pending or abandoned, oldest first."
  undeliveredFeedback: [UndeliveredFeedback!]!
}

extend type Mutation {
  "Stop retrying an item that will never succeed. Reversible."
  abandonFeedbackDelivery(submissionId: UUID!): Boolean!
  "Return an abandoned item to the queue, after fixing what was wrong."
  resumeFeedbackDelivery(submissionId: UUID!): Boolean!
}
```

**`FeedbackFailureReason` is an enum and not a string, and that is the point.**
FR-021 says no reason may disclose a credential or any fragment of one. A
free-text field carrying a host's response body is one 401 payload away from
breaking that, and the failure would be invisible in review because the string
looked fine on the day it was written. A closed vocabulary cannot leak, and the
mapping from host error to variant is a function with tests.

`resumeFeedbackDelivery` exists because an operator who has just fixed a
credential should not have to ask the submitter to send it again — and because
an irreversible "abandon" is a button nobody presses.

---

## What is deliberately absent

- **No webhook receiver.** It requires the instance to be reachable from the
  internet, which a self-hosted table behind a home router is not, and it adds
  a secret, an endpoint and a signature check — a second delivery mechanism to
  keep correct so that a status label updates sooner (R14).
- **No editing or closing issues from inside ThunderForge.** spec.md's Out of
  Scope.
- **No comment sync.** US5 reports state; a support inbox is a different
  product.
- **No per-world destination.** One repository per instance, by spec.md's
  Assumptions.
