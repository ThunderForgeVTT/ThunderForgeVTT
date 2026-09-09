# ADR-094: An Administrator's Second Factor Is a Property of the Role

**Date:** 2026-09-07
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Supersedes:** nothing; **related to** spec 040 (first-run setup), spec 041
FR-027/FR-028/FR-029

> Numbered 094 rather than the 097 the task text guessed: 084 through 093 were
> taken by specs 037 and 040 while this one was being planned. The decision is
> what matters, not the digits.

---

## Problem Statement

Spec 041 FR-027 says an administrator **must** hold a second factor. There are
two ways to express a rule like that, and they behave very differently over
time:

1. **Stored**: a flag set when somebody becomes an administrator.
2. **Computed**: derived from the role, every time it is asked.

A stored flag drifts. Somebody is promoted and the flag is not set; somebody is
demoted and it is not cleared; a migration adds an administrator directly. Each
of those leaves a row whose stored answer disagrees with the account's actual
authority, and the disagreement is silent.

---

## Decision

**Computed, never stored.** `required(user)` is one function —
`two_factor::requirement::second_factor_required(is_admin, instance_required,
admin_required)` — returning whether this account must hold a factor, and
`is_admin` is its first term.

There is no `two_factor_required` column, and the ones that exist —
`two_factor_admin_required` (this account, by an administrator's decision) and
`auth_security_settings.two_factor_required_for_all_users` (the instance
policy) — are *inputs* to that function, not caches of its answer. An account
that becomes an administrator is required to hold a factor from that instant,
because nothing had to be updated for it to be true.

Three consequences follow:

- **Giving up the role gives up the requirement.** An administrator who cannot
  keep a factor is told to stop being an administrator, which is a thing they
  can actually do, rather than being told "no" by a policy with no exit.
  `DisableRefusal::AdminRole` is checked *before* the instance and per-account
  policies for exactly this reason: an administrator told "the instance
  requires it" would go hunting for a setting to change, when what they must do
  is give up the role.
- **First-run setup cannot complete without one** (FR-028). The first
  administrator is an administrator, so the rule applies to them at the moment
  the account exists — and setup completion is where it is enforced, because an
  instance whose only administrator has no second factor is the state this
  requirement exists to prevent.
- **Both directions or neither.** The `is_admin` term reached `disable.rs`
  first and this expression second, and in the gap FR-027 was half true:
  `refusal_for` refused an administrator's *removal* while the login path let
  an administrator who had never enrolled sign in untouched. A requirement
  enforced only on the way out is not a requirement, and it is the same shape
  as the lockout `requirement.rs` was written to remove, pointing the other
  way. `an_administrator_is_required_to_hold_one_by_the_role_alone` pins it.
- **Turning the instance policy off leaves existing factors alone** (FR-022).
  The policy is an input to `required`, not a switch that disarms anything;
  ADR-081 already establishes that a confirmed factor is replaced, never
  disarmed.

---

## Alternatives Considered

**A stored `two_factor_required` column.** One query instead of a small
computation, and it is wrong the first time anybody changes a role by a path
that forgets to update it — including a migration, a fixture, or a support
script. The failure is silent and the symptom appears far from the cause.

**Enforce only at sign-in.** Cheap, and it means an account can *be* an
administrator without a factor for as long as it does not sign in — which is
precisely the window an attacker who has just promoted an account would use.

**Let administrators opt out.** Rejected: the role is the reason for the
requirement. An administrator who wants out of the requirement wants out of
the role, and the product should say so plainly rather than offer a switch
that quietly makes FR-027 untrue.

---

## Consequences

**Good**

- The rule cannot drift, because there is nothing to keep in step.
- One function to read, one place to change, and the boundary with spec 040 is
  clean: 040 owns *when setup is complete*, 041 owns *what completeness
  requires*.
- The refusal an administrator meets names the thing they can act on.

**Costs**

- A small computation on every check rather than a column read. Immaterial:
  it is a boolean over values already loaded to authorise the request.
- "Why can I not turn this off?" needs a good answer in the interface, because
  the honest one — give up the role — is a bigger step than the person was
  expecting. `DisableRefusal::AdminRole` carries that wording.
