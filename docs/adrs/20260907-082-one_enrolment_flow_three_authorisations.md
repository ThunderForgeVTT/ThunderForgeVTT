# ADR-082: One Enrolment Flow, Three Authorisations

**Date:** 2026-09-07
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Related:** spec 041 FR-001a, `contracts/enrolment.md`, ADR-081

---

## Problem Statement

Spec 041 FR-001a asks that enrolment be **one flow**, reachable from three
places, identical in its steps and wording:

1. account settings, by somebody who is already signed in;
2. first-run setup, by the first administrator;
3. a sign-in that a policy has just stopped, by somebody who is *mid*-sign-in.

The three entrances do not have the same thing to prove with. The first two
have a password to hand. The third has a correct password *and no session* —
that is the whole point of FR-019 — and the person is standing at a challenge
screen.

The plan's original task (T021) said to strip `username`/`password` from the
setup request entirely, on the assumption that one flow implies one
authorisation. It does not.

---

## Decision

**One flow, one endpoint pair, and `authorise_enrolment` accepting exactly one
of two proofs**: a username and password, or the `login_two_factor_challenges`
row the login path has just minted.

That row **is** the ticket `contracts/enrolment.md` asks for. It is created
only after a correct password, bound to one account, single-use, and dead in
ten minutes. It needs no new table and no new column — which is load-bearing
rather than convenient: a ticket that required a migration would have been a
password re-post instead, and re-posting a password from a challenge screen is
the thing this exists to avoid.

Both proofs together, or neither, is refused **before either is evaluated** —
the same rule `two_factor_verify` applies to a request carrying a code and a
recovery code. One request is one attempt at one thing.

### The ticket is fenced to the case that needs it

A challenge id now buys something it did not buy before: the ability to put a
second factor on the account it names. So it is refused unless the account
holds **no** confirmed factor. A challenge minted for an already-enrolled
account is a *verification* challenge, and is refused here unspent — this can
never become a way to replace somebody's factor with a stolen challenge id.

Held against the alternative, the ticket is **strictly narrower than the
password it replaces**: a password can enrol on any account at any time; the
ticket can enrol only on an unenrolled account, only for ten minutes, only
once.

---

## Alternatives Considered

**Re-post the password from the challenge screen.** Works, needs nothing new,
and means the password is typed and transmitted a second time in a flow whose
entire purpose is to add a factor *because passwords are not enough*.

**An enrolment-scoped session.** A real session cookie with reduced authority.
Rejected because "a session that is not quite a session" is a second kind of
session for every part of the system that reads one to reason about — the auth
middleware, the session list, revocation, the play-field claim — and the
blast radius of getting that wrong is every authenticated route, not one flow.

**A new `purpose` column on `login_two_factor_challenges`.** Considered and
not needed: "has this account got a confirmed factor?" already distinguishes an
enrolment challenge from a verification one, and it cannot fall out of step
with the account the way a stored enum can.

---

## Consequences

**Good**

- One flow to build, test and word — FR-001a satisfied in fact, not by three
  screens that resemble each other.
- No migration, no new credential type, no second kind of session.
- The ticket's authority is strictly less than the password's.

**Costs**

- `TwoFactorSetupStartRequest` keeps `username`/`password` alongside
  `challenge_id`, which looks like an underspecified input until you read
  `authorise_enrolment`. The refusal of "both, or neither" is what makes it
  precise, and it is tested rather than documented.
- **A residual, stated plainly**: an attacker who has already stolen a password
  (the only way a challenge gets minted) and can read the challenge id out of a
  response can enrol *their own* authenticator on an account that has none.
  That is exactly what they could already do with the password alone at
  `/2fa/setup/start`, so nothing is widened — but nothing is narrowed either.
  Closing it belongs to FR-012 and US3, not here.
