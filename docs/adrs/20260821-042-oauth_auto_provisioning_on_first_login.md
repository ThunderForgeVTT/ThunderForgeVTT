# ADR-042: OAuth Auto-Provisioning on First Login

**Status:** Accepted

**Decision Date:** 2026-08-21

## Context

ADR-007 (No Auto-Provisioning Policy) required an OAuth identity to match an
existing local account by email before it could be used to sign in; an
unmatched identity returned `no_matching_user` and stopped there. In
practice this meant every OAuth-first user had to separately register a
local account with a matching email before Keycloak/Google/etc. sign-in
would work at all — indistinguishable, from the user's perspective, from a
misconfigured provider (this was reported as a suspected Keycloak scope bug
before being traced to this policy).

With spec 008's onboarding-flow overhaul reducing the sign-up-to-canvas
funnel to a minimum, requiring a redundant manual local registration before
OAuth can be used at all works against that goal, and the concerns in
ADR-007 (duplicate accounts, ambiguous ownership) are already addressed by
requiring password confirmation when linking to an account that already
exists.

## Decision

Outside bootstrap, when an OAuth identity does not match an existing local
account by email, and the provider supplied a verified email address, the
flow now creates a new local account automatically instead of returning
`no_matching_user`:

- The new account's username is derived from the email's local part
  (`derive_bootstrap_username`'s existing logic), with a random suffix
  appended on collision.
- The account is given an unusable random password hash — password login
  never succeeds until the user explicitly sets a password.
- The OAuth identity is linked to the new account immediately; the user is
  signed in in the same request, matching the existing `LinkedUser` flow.
- If the OAuth identity's email instead matches an **existing** local
  account, the prior ADR-007/ADR-006 behavior is unchanged: password
  confirmation is still required before linking (`password_required`),
  since that account may already have a password protecting it.
- If the provider does not return an email at all, the flow still returns
  `no_matching_user` — there is nothing to provision or match against.

## Consequences

### Positive

1. First-time OAuth sign-in works in one step, matching spec 008's funnel
   goals.
2. No change to the safety property that mattered most in ADR-007/ADR-006:
   an OAuth login can never silently take over an existing account without
   password confirmation.

### Negative

1. A user with two different real email addresses (e.g. changes providers)
   can end up with two separate local accounts instead of one — same
   tradeoff ADR-007 was written to avoid, now accepted deliberately.
2. Account creation is no longer always explicit/local-registration-first;
   ownership/export/deletion semantics must treat auto-provisioned accounts
   as first-class from creation, not as a special case.

## Alternatives Considered

1. **Keep ADR-007's require-existing-account rule, improve error copy
   only** — rejected; still leaves a mandatory extra manual step in the
   onboarding funnel spec 008 is explicitly trying to collapse.
2. **Require email verification before provisioning** — deferred; providers
   used today (Keycloak, Google) only assert verified emails in practice,
   and this repo has no local email-verification flow to gate on yet.

## Migration Implications

- No schema change. Existing rows with `no_matching_user`-era behavior are
  unaffected; this only changes what happens on the next unmatched OAuth
  login.

## Security Implications

- Provisioning is only ever keyed off a provider-asserted email — no
  request-supplied data influences which account is created or linked.
- The password-confirmation safeguard for **matching an existing account**
  is untouched; only the "no account exists at all" branch changes.
