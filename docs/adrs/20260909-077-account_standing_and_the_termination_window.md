# ADR-077: Account Standing and the Termination Window

**Date:** 2026-09-09
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Related:** spec 039 US5/US7, ADR-076, ADR-094 (an administrator's second
factor is a role property — the same computed-not-stored reasoning)

---

## Problem Statement

The repeat-infringer programme counts strikes and has a threshold, and that is
where it stops. Reaching the threshold means a flag exists; nothing follows
from it. What is missing is the *consequence* — a ladder somebody can be told
about in advance, a window in which they can appeal, and a way back if the
strikes age out.

The design question underneath is which parts of that are **state** and which
are **arithmetic**, because getting it the wrong way round is how a moderation
system ends up with an account that is disabled according to one table and fine
according to another.

---

## Decision

**Standing is derived on every read. The termination window is the only row.**

```text
strike_count(account) = moderation::strike_count(conn, account)
standing(account)     = { strikes,
                          may_publish: strikes < SUSPEND_PUBLISHING_AT
                                       && no open termination,
                          disabled:    an open termination exists,
                          termination: the open row, if any }
```

### Why derived, and what it buys

FR-035 says a strike ageing past the 365-day lookback restores an account. With
standing derived, **that is free**: the count changes because the calendar
moved, and nothing has to notice. With standing stored, it is a scheduled job
that has to run, and an account stays punished for as long as that job is
broken — a failure whose symptom is somebody silently unable to publish and no
row anywhere saying why.

This is the same reasoning as ADR-094's administrator rule, applied to a
different subject: a stored answer to a question that is already computable is
a second source of truth that nothing keeps in step.

### Why the window is a row anyway

Because it is not derivable. `deletion_due_at` is a *promise made at a moment*
— "you have until this date" — and it has to survive the settings that produced
it changing. So:

- **`deletion_due_at` is computed once, at open.** Changing
  `MODERATION_TERMINATION_WINDOW_DAYS` afterwards does not move a window
  already running. Exactly how `restoration_due_at` already behaves when a
  counter-notice is forwarded.
- **`requires_human` is snapshotted at open**, not read at execution. An
  operator flipping the setting must not retroactively change the terms
  somebody was told about at the start of their window (FR-030, FR-036).
- **At most one open termination per account**, as a partial unique index on
  `account_id WHERE closed_at IS NULL`. Two windows for one account is the bug
  that produces two deletion dates, and an index is the only place that
  guarantee cannot be forgotten.

### The sweep is the fifth `spawn_*_task`

Not a cron, not a new scheduler: the shape this server already uses four times
over. It ticks at 300s, is off every hot path, and closes terminations whose
strikes have aged out (`closed_reason = 'strikes_aged_out'`) — so an account
comes back without anybody asking, which is FR-035 arriving on its own.

### `MODERATION_TERMINATION_REQUIRES_HUMAN` defaults to **true**

Deleting an account is the one irreversible act in this feature. The default is
the one where a person has to press the button.

An operator who wants it automatic can have it, and the setting is snapshotted
so their choice is fixed at the moment each window opens. But a default of
`false` means the first unattended instance deletes somebody's worlds because
three notices arrived, and "the software did it" is not something this project
wants to be able to say.

### The ladder is told in advance

| Strikes | Consequence | Setting |
|---|---|---|
| 1 | Warned. Publishing unaffected. | `MODERATION_STRIKE_WARN_AT` = 1 |
| 2 | Publishing suspended. **Play, edit and read untouched** (FR-019). | `MODERATION_STRIKE_SUSPEND_PUBLISHING_AT` = 2 |
| 3 | Account disabled; the window opens. | `MODERATION_REPEAT_INFRINGER_THRESHOLD` = 3, existing |

The middle rung is the one worth defending. Suspending *publishing* rather than
access means somebody with two strikes can still run their game — their table
is not punished for their behaviour, and the sanction lands on the act that
caused it.

All four values are environment variables with parse-or-default fallback,
matching the three already in `moderation/mod.rs` exactly, including that an
unparseable value falls back silently rather than refusing to start.

---

## Alternatives Considered

**Store `is_disabled` and `may_publish` on `users`.** One column read instead
of a count. Rejected: it is two more places for the truth to live, and the
restoration path becomes a job that must run rather than a fact that becomes
true. The symptom of that job failing is invisible.

**No window — disable and delete immediately at the threshold.** Rejected
outright. An appeal that cannot be filed before the data is gone is not an
appeal, and a mistaken third notice would be unrecoverable.

**A window with no appeal, just a delay.** Rejected: a delay is a countdown, an
appeal is a process. The difference is whether a person has anything to do.

**Recompute `deletion_due_at` from the setting on each read.** Rejected — see
above. It means an operator shortening the window moves a date somebody was
already told, which is the one thing a stated deadline may not do.

**`requires_human` read at execution.** The subtler version of the same
mistake, and easier to write by accident. Snapshotted.

---

## Consequences

**Good**

- Restoration by ageing-out costs nothing and cannot fail to happen.
- One place computes standing, so the answer cannot disagree with itself.
- A person in a window knows the date and the terms, and neither moves under
  them.
- Two strikes stops publishing, not playing.

**Costs**

- `strike_count` runs on reads that care about standing. It is a counted query
  over an indexed lookback, on paths that happen at human speed.
- The sweep is a fifth background task to reason about at startup.
- An operator who turns `MODERATION_TERMINATION_REQUIRES_HUMAN` off still finds
  windows already open behaving the old way. Deliberate, and worth explaining
  in the interface rather than smoothing over.
