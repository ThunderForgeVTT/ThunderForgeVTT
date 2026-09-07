# The Instance Decides Admission Before ADR-042 Decides Provisioning

- **Date**: 2026-09-06
- **Status**: Accepted
- **Spec**: `specs/035-instance-access/` (FR-001 – FR-013a)
- **Follows**: ADR-007 (no auto-provisioning, superseded), ADR-042 (OAuth
  auto-provisioning on first login), ADR-008 (bootstrap admin exception)
- **Governs**: Constitution Principle III — ownership and authorization at the
  data boundary

## The decision

An instance has an **access policy** — `open`, `invite_only` or `closed` — and
it is consulted by **every** path that can bring a new user account into
existence, before that account is created.

ADR-042 continues to govern **how** an admitted person's account is made: the
derived username, the unusable password hash, the immediate identity link, and
the untouched password-confirmation rule for linking to an account that already
exists. It stops governing **whether** a stranger may be admitted at all. That
question is answered first, by the policy, for every route without exception.

## Why this needs an ADR

Principle IV requires one when a change moves an established ownership or access
boundary. ADR-042 is an accepted decision that reversed ADR-007, and this
narrows the conditions under which it applies. That is a change to the meaning
of a shipped decision, not a new feature behaving in a new way.

## What was actually wrong

ADR-042's decision is sound and is not being reversed. What it did not
anticipate is that the instance had no way to say "no new accounts".

There are exactly two paths that create a user account outside bootstrap:
`auth/sessions.rs`'s local registration, and `auth/oauth.rs`'s
`resolve_oauth_login`. Only the first was gated at all, by
`ensure_registration_allowed`, and that function answered a different question —
"has first-run setup happened?".

So an "allow signups" switch that governed only the local form would have been
**worse than useless**. An operator would close signups, watch the registration
form disappear, conclude the instance was closed, and keep admitting every
stranger who clicked "Sign in with Google". The failure is silent: nothing
reports it until an unknown account appears in the user list.

## Why a gate above, rather than reverting ADR-042

ADR-042's reasoning holds wherever the instance is open, and it was adopted for
a real problem — a redundant manual registration step that users could not tell
apart from a broken provider. Reverting it would reintroduce that.

Layering keeps both facts true at once. **A closed instance restores ADR-007's
effective outcome** for an unmatched OAuth identity — no account is created —
without restoring ADR-007 as policy on an instance that is open. The two ADRs
stop competing because they answer different questions.

## Consequences

- **Every account-creating path must call the gate.** There are two today. A
  third added later inherits the requirement, and a path that creates an
  account without calling it is the defect this ADR exists to prevent — not an
  oversight to be fixed afterwards.
- **The gate sits after the "does this email match an existing user" lookup**
  in `resolve_oauth_login`. That placement is load-bearing: it makes
  "authenticating as an existing account is not signup" true by construction
  rather than by a second check that could drift.
- **Bootstrap is never gated** (ADR-008). An instance with no administrator
  must still be able to make its first one, whatever the policy says, or a
  conservative default would brick a fresh install.
- **A closed instance refuses even a valid invitation.** `closed` means no
  account by any means. Reopening to `invite_only` makes issued invitations
  work again; the policy switch never alters an invitation's own validity.
- **A refusal must not become an oracle.** The gate runs before the
  username/email uniqueness probes, so a closed instance answers identically
  whether or not the submitted address belongs to a user.
- **The default differs for a new instance and an upgraded one.** A fresh
  install starts `invite_only`; an instance that already has users starts
  `open`, preserving the behaviour it had before the upgrade. Silently closing
  a running community on upgrade would be a worse failure than the one this
  ADR prevents.

## A correction this ADR also makes

`src/server/src/auth/registration.rs` attributed auto-provisioning to
"ADR-011" in three comments. ADR-011 is the Export-My-Data Contract; the
governing decision is ADR-042. The citations are corrected in the same change
set, because an ADR trail that points at the wrong document is worse than none.
