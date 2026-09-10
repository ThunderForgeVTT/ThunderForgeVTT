# The second factor, end to end

What a ThunderForge second factor is, how somebody gets one, how they get rid
of one, and what happens when they lose it. Written down because the *reasons*
are the durable part — the code moves, and a reader who knows why removal costs
possession will not later "simplify" it into costing a password.

Spec 041. The decisions are ADRs 081, 082, 083 and 094.

---

## One flow, three entrances

Enrolment is one flow. What differs between the three ways in is only how the
caller proved they are allowed to start it, and where they end up afterwards.

| Where | How it is authorised | Where it ends |
|---|---|---|
| `/settings/security` | Username and password, re-typed | Back on the page, factor confirmed |
| First-run setup | The administrator's own new password | The next step of the wizard |
| A sign-in that requires one | The `login_two_factor_challenge` it was just handed | **Signed in**, where they were going |

The third is the one that matters most and the one that did not exist. Before
it, turning the instance-wide requirement on refused every account that had not
already enrolled: the login path handed them a *verification* challenge, they
had no secret, and a verification of nothing can only ever answer "no". That
was a lockout with no way out from the screen that issued it.

`authorise_enrolment` accepts **exactly one** of a username-and-password or a
challenge id. Both together is refused before either is looked at, for the same
reason a challenge carrying two credentials is: one request is one attempt at
one thing. See ADR-082 for why an enrolment-scoped session was rejected.

## What an enrolment does and does not touch

A pending secret lives **beside** the live factor, never on top of it.

This is ADR-081, and it is a fence around a real hole rather than a matter of
taste. `setup/start` used to write the new secret over the live one and clear
`two_factor_enabled` in the same statement, on a password alone. Beginning an
enrolment was therefore a way to *remove* a confirmed factor without ever
proving possession of it — and an enrolment somebody merely abandoned left the
account weaker than it started, silently.

So: `setup/start` writes the pending columns and nothing else. `setup/confirm`
promotes the pending secret once a code has proved it works. A wrong code
leaves the pending secret intact, because a mistyped digit must not send
somebody back to rescanning a QR code.

## Recovery codes are credentials, not links

Ten, issued at confirmation, Argon2id-hashed, single-use.

They are shown **once**, and that is a property of the storage rather than a
promise the interface keeps: there is nothing to show a second time. ADR-083
records why they follow the admin bootstrap code rather than the raw-stored
share codes — the deciding question is not "does a person type it" but "what
does holding it get you", and a recovery code gets you the account.

Two consequences worth knowing before touching that code:

- **Verification has no early exit.** Every unspent hash is checked, so the
  time taken does not vary with which code was offered.
- **Spending is a conditional `UPDATE`.** Two requests presenting one code are
  what a replay looks like, and exactly one may win.

## Proving one, once

A TOTP code is valid for its own thirty-second step and one step either side,
so somebody who reads it over a shoulder has up to ninety seconds to use it.
The consumed challenge does not stop that — their sign-in gets a *new*
challenge — so the refusal has to come from the code.

`matched_step` returns which step matched; `claim_totp_step_sync` claims it
with `WHERE two_factor_last_used_step IS NULL OR two_factor_last_used_step <
$step`. **Strictly less-than, not "different"**: with a skew of one, the
previous step's code is still cryptographically valid and carries a *lower*
number, so `<>` would admit it.

Guessing is bounded per **account**, not per address — five consecutive
failures, then sixty seconds. Per address would be no protection at all against
somebody with more than one address, which is everybody who matters here. The
bound lives inside `throttle::guarded`, which wraps the verification itself, so
it covers every route that checks the credential rather than the one route it
was written for. That distinction is not academic: it guarded one route of four
for a day.

## One refusal

Every refusal on the verification path that is about a *credential* says the
same thing — `"That code was not accepted."`, and a 401. A wrong code, a
correct code whose step is spent, a wrong recovery code, one already used, one
belonging to a different account, an account with no codes, an account with no
factor at all.

Each alternative is a fact about somebody else's account offered to a caller
who has not proved they hold it. "No recovery codes on this account" says which
credential is worth spending the remaining attempts on; "that code was right
but already used" confirms an intercepted code was genuine.

The one deliberate exception is a challenge that is unknown, expired or
consumed. That is a fact about a *request*, and the person has to do something
different about it — sign in again — which no amount of retyping a code will
reach.

## The way off

`POST /authentication/2fa/disable`, costing **password and possession**.

The session is not enough on its own: it proves the password was held at
sign-in, possibly days ago, on a machine that may since have changed hands, and
this is the one action that makes every future sign-in cheaper. Either a
current authenticator code or an unspent recovery code satisfies possession —
the same two proofs regeneration accepts, through the same helper, because two
implementations of "prove you still hold it" is one more than this product
should have.

A refusal here is allowed to be specific, unlike everything else on this
surface: by the time it is reached the caller has proved both factors, so they
are the account holder, and telling them *why* is telling them something they
are entitled to know and can act on.

## Who must hold one

```text
required(user) =
      user.is_admin                                -- the role. Not a policy.
   OR user.two_factor_admin_required               -- one account.
   OR instance.two_factor_required_for_all_users   -- an operator's choice.
```

Computed, never stored — ADR-094. There is no `two_factor_required` column, and
the two that exist are *inputs* to that line rather than caches of its answer.
An account that becomes an administrator is required from that instant, and an
instance upgrading into the rule needs no backfill.

`|| two_factor_enabled` is deliberately **not** a term. Having a factor is not
a reason you must have one; it is a reason you are *asked* for it, which is a
different question with a different answer. Collapsing the two is what made the
instance-wide switch look safe.

Turning any requirement off leaves every confirmed factor in force. The policy
was never one of the factor's inputs.

## When somebody has lost everything

The authenticator is gone and the codes went with it. Without a defined path,
the only way to help them is a database edit: unaudited by construction,
unnotified, and performed from memory at the worst possible moment.

`POST /authentication/admin/users/{id}/2fa/reset` is that path. It clears the
factor, its pending enrolment and its recovery codes in one transaction, writes
a `reset_by_operator` event with the operator's own id against it, and tells
the account holder.

It **cannot enrol and cannot issue codes.** An operator who could hand out a
working second factor could sign in as the account holder, and no amount of
audit trail makes that acceptable. What it leaves behind is an account with no
factor — a state the product already understands — and the person enrols again
from their own screen, with a secret only they ever see.

The route lives in `auth::admin_router()`, which `main.rs` wraps in
`require_admin_user` as a **layer**. A route added to that router is guarded by
having been added to it; `admin_routes_tests` fails if an admin path is
registered anywhere else. The handler also checks for itself — two independent
mechanisms in front of "reset somebody else's second factor" is the shape worth
having.

## Being told

Every change is written to `two_factor_events` **inside the transaction that
made it**. A removal nobody recorded did not happen, and a factor cleared whose
row rolled back would be a security history with a hole in exactly the place
somebody would look.

The row records the act, never the person: no address, no user agent, no code
and no fragment of one. `recovery_code_used` says a code was used; it never
says which, because knowing which would make the log worth stealing.

Two user columns, because "who did it" and "for whom" are different questions
and one column cannot answer both. `actor` is `ON DELETE SET NULL`, so an
operator who later leaves does not take the record of their reset with them.

The **notice** is a separate thing with the opposite failure mode. It runs
after the commit, returns nothing, and swallows what it cannot do — a caller
that could fail on it would refuse a removal because a mail server was down,
leaving the account changed and the request refused. It enqueues through
`mail::outbox`, which succeeds on an instance with no mail configured: the row
lands blocked, naming the settings that are unset.

And because a great many instances have no mail at all — which is the ordinary
state for somebody running this for their own table, not a broken one — the
account holder's own security page shows the same history. On such an instance,
**that page is the notification**.

## Not a dead end

A person who has lost both their authenticator and their codes reaches a screen
where every control asks for something they no longer have. It names who can
reset it, from `support_email`, published anonymously on
`/authentication/setup/status` — a locked-out person is exactly an anonymous
caller. Where the instance has published no address, the screen says so
plainly, because sending somebody looking for a contact that does not exist is
worse than telling them there is not one.

## What an operator sees

Three integers: enrolled, not enrolled, and of those how many are required and
have not. Never a roster. A list of accounts without a second factor is a list
of accounts a stolen password is sufficient for, and it would be handed to
whoever takes over an operator's session.

The figure that matters sits beside the switch it explains: how many people the
switch is about to ask something of, said before it is thrown.
