# Contract: Standing, strikes, and the termination window

The counting already exists and is reused unchanged. What is added is a
consequence, a window, and a way for a person to see where they stand.

## Standing

```graphql
type Strike {
  caseId: UUID!
  entityType: ModerationEntityType!
  entityId: UUID!
  worldId: UUID!
  recordedAt: String!
  "When this strike stops counting, from the existing lookback."
  agesOutAt: String!
}

type Termination {
  openedAt: String!
  deletionDueAt: String!
  "\"none\" | \"open\" | \"upheld\" | \"rejected\"."
  appealState: String!
  "True when this instance requires a person to decide, not a timer."
  requiresHuman: Boolean!
}

type Standing {
  strikes: [Strike!]!
  strikeCount: Int!
  "The rungs in force on this instance, so a person can see how many remain."
  warnAt: Int!
  suspendPublishingAt: Int!
  "The threshold in force on this instance."
  threshold: Int!
  warned: Boolean!
  mayPublish: Boolean!
  disabled: Boolean!
  termination: Termination
}

"What a person was told. The words are rendered from kind + payload, never stored."
type AccountNotice {
  id: UUID!
  kind: String!
  subjectRef: JSON
  payload: JSON
  createdAt: String!
  readAt: String
}

extend type Query {
  "The caller's own standing. Available to a disabled account."
  myStanding: Standing!

  "Anyone's standing. Admin only."
  accountStanding(accountId: UUID!): Standing!

  "What the caller has been told about their own account, newest first."
  myNotices(limit: Int): [AccountNotice!]!
}
```

*Amended 2026-09-10, with US5:* `Strike` names the content (`entityId`, and the
entity type as the moderation enum rather than a string); `Standing` carries
every rung, not only the threshold, because FR-028's "how many remain" is a
question about the next rung, not the last; and `myNotices` is the read side of
`account_notices`, without which a notice is written and never seen.
`Standing.termination` and the `Termination` type arrive with US7, which creates
the table they read — a type with no instance would be a promise in the schema.

`myStanding` is FR-029 and it is not decorative: FR-028 says nobody may reach
the third strike having never been told about the first two, and a page that
answers "where do I stand" at any moment is half of keeping that promise. The
other half is `account_notices`.

**Everything on `Standing` except `termination` is derived on read**, from
`content_moderation_actions` via `moderation::strike_count` — the function
extracted from `repeat_infringer_flags_impl`'s body, which then calls it, so
there is one definition of a strike and FR-027 is true by construction.

## The ladder

| Strikes | Consequence | Setting | Default |
|---|---|---|---|
| 1 | Warned. A notice; publishing unaffected | `MODERATION_STRIKE_WARN_AT` | 1 |
| 2 | Publishing suspended; play, edit and read untouched | `MODERATION_STRIKE_SUSPEND_PUBLISHING_AT` | 2 |
| 3 | Disabled; the window opens | `MODERATION_REPEAT_INFRINGER_THRESHOLD` (**existing**) | 3 |
| — | The window | `MODERATION_TERMINATION_WINDOW_DAYS` | 30 |
| — | A person decides, not a timer | `MODERATION_TERMINATION_REQUIRES_HUMAN` | **true** |

Same parse-or-default shape as the three values that already exist in
`moderation/mod.rs`, including their behaviour on an unparseable value. FR-040.

**`REQUIRES_HUMAN` defaults to true**, and that is the shipped behaviour: the
window runs, the account is disabled, the person is told and has their thirty
days, and at the end the termination lands in an administrator's queue rather
than firing. Automatic deletion is a switch an operator throws. A self-hosted
table of six friends should not have a timer that deletes one of them, and
spec.md lists exactly that edge case.

## What a disabled account may do

An allowlist, not a filter. `resolve_authenticated_user` still resolves the
session — a blanket 401 there would destroy the export and with it the remedy —
and the refusal happens one layer up:

- `authenticated_user(ctx)` **refuses** a disabled account.
- `authenticated_user_even_if_disabled(ctx)` is called by exactly these:

| Surface | Why |
|---|---|
| `exportMyData` and `GET /api/user/data/export` | FR-031, FR-032 — the download |
| `myStanding` | So the person can see the window and the date |
| `myAttestations` | So they can see what they agreed to |
| `fileAppeal` | FR-031 — the appeal |
| sign-out | Nobody should be unable to leave |

Everything else is refused, and a mutation added next year is refused by default
because it will call `authenticated_user` like every other. The allowlist fails
closed, which is the property that makes FR-031's "and can do nothing else"
survive the next feature.

**Content owned by a disabled account stops being served publicly** (FR-038):
share links belonging to it resolve as dead, on the same one-message rule
`UNAVAILABLE` already uses. Other people's worlds are untouched — a world the
account is merely a member of, and a world it created that other people are
still in, are both left alone.

## The appeal

```graphql
extend type Mutation {
  """
  File an appeal against a disablement. One open appeal at a time.
  Callable by a disabled account.
  """
  fileAppeal(statement: String!): Termination!

  "Resolve an appeal. Admin only."
  resolveAppeal(accountId: UUID!, upheld: Boolean!, note: String): Termination!
}
```

1. **An appeal pauses the deletion; it does not extend the window** (FR-034).
   An appeal open at `deletionDueAt` blocks execution until it resolves. A
   rejected appeal resumes from the date that has already passed — so deletion
   is never the outcome of the instance being slow, and filing on day 29 does
   not buy a second window. spec.md's "an appeal filed on day 29 and not
   resolved by day 30" is this sentence.
2. **An upheld appeal restores the account, cancels the deletion, and removes
   the strike it overturned** (FR-033). Removing the strike is a
   `content_restored` row on that case, which is how the moderation programme
   already un-counts one — not a new mechanism and not an edit to a count.
3. **Downloading and appealing are not alternatives** (FR-032). Neither writes
   anything to the other's state, in either order. A test asserts both, both
   ways round.

## The sweep

```rust
// src/server/src/moderation/standing.rs
pub async fn run_due_standing_work(state: &AppState) -> Result<SweepReport, String>;
pub fn spawn_standing_task(state: AppState);   // 300s, from src/app/src/main.rs
```

Idempotent, and called from three places — the periodic task, the admin
moderation query, and the sign-in of an account that has an open termination, so
somebody signing in on day thirty-one sees the truth rather than a stale window.
It:

- opens a termination for an account newly at the threshold, unless doing so
  would disable the last administrator (see below);
- **closes** a termination whose strike count has fallen below the threshold,
  with `closed_reason = 'strikes_aged_out'`, restoring the account without the
  person asking (FR-035);
- executes a deletion that is past due **only** when no appeal is open and the
  termination's snapshotted `requires_human` is false.

`spawn_standing_task` is the fifth instance of a pattern
`lore_sync/schedule.rs:108` already documents as house style — "a `spawn_*_task`
in the library, called from the binary, owning its own schedule and staying off
every hot path. No new infrastructure, deliberately." It is a backstop, not the
mechanism: everything it does is reachable without it, which is what lets a test
move `deletionDueAt` into the past and call the admin surface rather than wait.

## Deletion

`delete_user_data_owned`, with one change FR-038 forces: **a world the account
created that has live members other than the account is not deleted.** It is
retained, its public paths closed, and it is listed for an administrator. The
existing function deletes every world in `worlds.created_by == user_id` along
with its events and tokens, which would destroy other people's tables as a side
effect of one account's disablement.

Attestations are redacted rather than deleted: `subject_username -> NULL`,
everything else kept (FR-010, FR-037). Deletion is real and irreversible, and
the person was told so at the **start** of the window, not only at the end
(FR-036) — the `account_disabled` notice says it in the first sentence.

## The last administrator

If opening a termination would disable the only account with `is_admin`, the row
is written with `requires_human = true` regardless of the setting and **the
account is not disabled** (FR-039). An administrator is told. If that
administrator *is* the only one, the instance says so and does nothing further,
which is the honest outcome: a product that locks itself out in order to enforce
a rule has enforced nothing and can no longer undo it.

## What is deliberately absent

- **No admin mutation that sets a strike count.** Standing is derived. An admin
  changes it by resolving a case, which is the surface that already exists.
- **No "disable this account" button.** Disablement is a consequence of the
  counting. A button would be a second way to reach the same state, with
  different evidence behind it.
- **No re-tuning of the threshold or lookback.** Out of scope by spec.md's
  Assumptions, and unnecessary: the default is already three over 365 days.
- **No detection of somebody creating a new account to escape a disablement.**
  spec.md lists it as an edge case; addressing it means identity verification,
  which is a different feature with a different review. Named here so it is a
  known limit rather than an oversight.
