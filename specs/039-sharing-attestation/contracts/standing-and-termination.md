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
| `myNotices` | So they can read what they were told — the page that shows the window shows these |
| `myAttestations` | So they can see what they agreed to |
| `fileAppeal` | FR-031 — the appeal |
| `submitCounterNotice` | The statutory route back — *added 2026-09-10*. Still the content's own GM only |

Everything else is refused, and a mutation added next year is refused by default
because it will call `authenticated_user` like every other. The allowlist fails
closed, which is the property that makes FR-031's "and can do nothing else"
survive the next feature. `graphql/disabled_surface_tests.rs` walks the crate
for call sites and fails on a sixth.

*Amended 2026-09-10 by the owner's decision:* **`submitCounterNotice` is on the
list.** FR-031 read literally — "download and appeal, nothing else" — would have
refused a DMCA counter-notice to the account the notices disabled, which is the
one account that most needs the statutory process FR-020 says restoration must
follow. A case under counter-notice review does not count (FR-027), so filing
one on the third strike drops the account below the threshold, and the resolver
sweeps that account at once: it is restored when it files, and its window closes
as `counter_notice`. If staff uphold the takedown anyway, the case counts again
and a new window opens. A disabled account files it from the standing page,
since its content pages are out of reach.

*Amended 2026-09-10, with US7:* **`myNotices` is on the list** — the standing
page is where a disabled person reads that the window opened and when it ends,
and a notice they cannot read is a notice they were not given. **The REST layer
fails closed too**: `require_authenticated_user` refuses a disabled account, and
exactly two routers use `require_authenticated_user_even_if_disabled` — GraphQL,
which runs the allowlist above, and the download. **The authentication routes
are not refused** — sign-in, sign-out, session refresh, and password and
second-factor changes. A disabled person has to be able to sign in to reach
their remedies, and securing the credential that reaches them is part of
reaching them; none of those routes touches content.

**Content owned by a disabled account stops being served publicly** (FR-038):
share links belonging to it resolve as dead, on the same one-message rule
`UNAVAILABLE` already uses — including when its standing cannot be read, which
fails closed.

*Amended 2026-09-10 by the owner's decision of 2026-09-08:* **a world the
account created is deleted with it**, including one other people play in — the
same rule as a person deleting their own account. What survives is each
player's character: before the world goes, every actor owned by somebody else is
moved into a world that player owns, inside a collection named for the world it
came from, and the player is told. FR-038's "other people's worlds MUST NOT be
destroyed" is replaced by "other people's **characters** MUST NOT be
destroyed".

## The appeal

```graphql
extend type Mutation {
  """
  File an appeal against a disablement. One per window.
  Callable by a disabled account.
  """
  fileAppeal(statement: String!): Termination!

  """
  Resolve an appeal. Admin only. overturnedCaseId names the strike the appeal
  overturned; omitted, it is the most recent — the one that crossed the
  threshold.
  """
  resolveAppeal(accountId: UUID!, upheld: Boolean!, note: String, overturnedCaseId: UUID): Termination!

  """
  Carry out a window that waits for a person (REQUIRES_HUMAN, the default).
  Admin only. Refused before the date, while an appeal is open, and for the
  instance's last administrator.
  """
  executeTermination(accountId: UUID!): Boolean!
}
```

*Amended 2026-09-10, with US7:* **one appeal per window, not one at a time.** A
rejected appeal resumes the window from the date already passed (rule 1); if the
person could then file again, each filing would pause deletion anew and the
window would never end. **`executeTermination` exists** because the shipped
default sends every due window to a person, and the person needs a control —
this is the "administrator's queue" the ladder section describes, not a
"disable this account" button: it acts only on a window the counting opened and
the calendar closed.

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

`delete_user_data_owned` — the same code a person deleting their own account
runs, on the connection the sweep holds, inside the transaction that closes the
window.

*Amended 2026-09-10 by the owner's decision of 2026-09-08*, replacing "a world
with live members is retained": **the account's worlds are deleted, and each
player's character is moved out first.** For every actor in those worlds owned
by somebody else, the actor is copied — through the collections copy path, so
its sheet, images and abilities come with it — into a world its player owns on
**the same game system** (a new "<name>'s characters" world if they have none),
inside a collection named for the world it came from, and the player gets an
`actor_rescued` notice saying where. The same step runs when a person deletes
their own account. The collection is deliberate: when content can live on a
profile outside any world, organised by collections, a rescued character is
already where that move expects it to be.

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
