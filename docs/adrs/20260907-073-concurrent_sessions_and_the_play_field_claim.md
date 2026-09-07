# An Account May Be Signed In Many Times, and Be at the Table Once

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/036-concurrent-client-sessions/` (FR-001 – FR-015, FR-027 – FR-031)
- **Supersedes**: the revoke-on-login policy in `auth/sessions.rs`, which was
  code rather than a recorded decision
- **Governs**: Constitution Principle III — ownership and authorization at the
  data boundary

## The decision

**Signing in no longer ends the sessions that came before it.** An account may
hold several live sessions at once, bounded, and each is a full client.

**Exactly one of those clients is at the play field.** The rest are companion
surfaces. The claim is not stored; it lives only as long as the client holding
it.

## What was there, and why it was there

`create_session` revoked every un-revoked `user_sessions` row for the account
on each successful login. One line, with its reason written beside it:

> "Revoke existing active sessions on new login to reduce session replay risk."

That was a deliberate, defensible choice, and the table never enforced it —
`user_sessions` is keyed by `id` with a plain `user_id` and no unique
constraint, so several live sessions were always representable. The policy
lived entirely in that statement.

Its cost was that **one account could be signed in in exactly one place**. A
second browser, a second profile or a private window signed the first one out.
That is the shape of the defect that interrupted a live demo, and it is why
every cross-client test in the suite had to be written between two different
*people* — one person with two windows could not be expressed at all.

## Why removing it is safe

Eviction reduced replay by shortening the window in which a stolen cookie is
useful — but only for people who log in often, and at the price of the
capability above. What replaces it is the control the eviction was
approximating, in a form a person can actually operate:

1. Every session is listable by its owner, with when it was created, when it
   was last used, and a coarse origin.
2. Any session is individually revocable, and all of them at once.
3. A password change ends every session but the one that changed it.
4. Session creation is recorded where the owner can see it.

Nothing about session lifetime, expiry, or any existing check changes.
`revoked_at` and `expires_at` keep their meanings exactly.

A **bound** is kept: at most ten live sessions per account. Reaching it ends
the least recently used rather than refusing the new sign-in, because refusing
the newest thing is precisely the failure this decision exists to remove — it
would simply reappear at a higher number.

## Why exactly one play field

This is the narrowing that makes the rest worth having.

Two play fields for one person means two engines, two event appliers and two
peer endpoints for one human being, which is where the session and sync
problems would come from — and nobody asked for it. What people ask for is
their character sheet on the other screen. So the play field is exclusive and
everything else is a companion surface, which is what makes the sheet
possible.

**The claim is a subscription-owned registry entry, not a row.** A claim must
never outlive the client holding it: a crashed or closed window must not lock
somebody out of their own table. `peer_signaling.rs` already solved exactly
this shape and says why in its own module docs — registration begins when the
stream is established and ends when it is dropped, with the guard owned by the
stream, "enforced by construction rather than by a cleanup job that can be
forgotten, misconfigured, or skipped during a crash". A claim in a table needs
a heartbeat, a timeout and a reaper, and its failure mode is a person locked
out by a window that is already gone.

**Takeover resolves in favour of the newest claim**, with the previous holder
demoted to a companion, told plainly, and offered it back. Refusing the new
window instead would reproduce the original defect one level up: "you are
already playing somewhere else" is the same dead end as "you have been signed
out", and the person cannot always reach the other window to release it.

## Consequences

- **Positive.** A person can keep the table on one screen and their character
  sheet on another. Two people at one table can be tested as two people, and
  one person with two windows can finally be tested as one person.
- **Positive.** Sessions become visible and endable, which is a stronger and
  more honest control than a silent revoke somebody only notices by being
  logged out.
- **Cost.** The claim registry is per server process, as presence and the peer
  registry already are. A multi-process deployment needs a shared registry for
  all three together. This decision does not introduce that constraint and
  does not fix it; it is recorded here rather than discovered later.
- **Cost.** Removing the eviction means a stolen session cookie stays useful
  until it expires or is revoked, where previously the owner's next login
  would have ended it by accident. The four controls above are what must be
  built for that trade to hold, and FR-025 and FR-026 forbid lengthening a
  session's life to compensate.
- **Obligation.** A regression guard exists
  (`apps/web/e2e/concurrent-sessions.spec.ts`) whose only job is to fail if
  login ever revokes again. It signs in three times rather than twice, because
  a bug that kept only the newest session would still leave two clients alive.
