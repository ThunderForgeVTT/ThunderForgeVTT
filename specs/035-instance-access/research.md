# Phase 0 Research: Instance Access

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-09-06

Every finding below was verified by reading the code named in it, not by
recollection. Line numbers are as of `beb8445`.

---

## §1 — The hole is real, and it is two functions wide

FR-005 requires the policy to govern *every* path that can create an account.
Today there are exactly two, and only one of them is gated at all.

**The local path is already gated, by the right seam.**
`auth/sessions.rs:64` opens with:

```rust
if let Err(message) = ensure_registration_allowed(&state).await { ... }
```

`ensure_registration_allowed` (`auth/registration.rs:18`) currently answers one
question — has first-run admin setup happened? — and returns
`Err("Registration is unavailable until the initial admin setup is complete")`
otherwise. It is not an access policy. But it is a single function, called
before any validation or write, that already means "may this request create an
account". That is the seam the policy belongs in.

**The OAuth path is not gated at all.** `grep -rn ensure_registration_allowed`
returns exactly three hits: the definition, the re-export in `auth/mod.rs:97`,
and the one call in `sessions.rs:64`. `resolve_oauth_login`
(`auth/oauth.rs:217`) never calls it. At `oauth.rs:~327`, an authenticated
provider identity whose verified email matches no user is auto-provisioned on
the spot:

```rust
let Some(existing_user_id) = existing_user_id else {
    // ADR-042: ... a first-time OAuth identity with no local account at all
    // is auto-provisioned.
    let username = unique_username_from_email_sync(&mut conn, &provider_email)?;
    ...
```

**Caution — the code cites the wrong ADR for this.** `auth/registration.rs`
(lines 4, 48, 93) and `auth/oauth.rs` attribute auto-provisioning to
"ADR-011". ADR-011 is the **Export-My-Data Contract**. The decision actually
governing this behaviour is **ADR-042** (OAuth Auto-Provisioning on First
Login, 2026-08-21), which supersedes **ADR-007** (No Auto-Provisioning Policy)
— and ADR-007's header says so explicitly. Every reference in this feature's
documents is to ADR-042. The stale citations in the auth source are a separate,
pre-existing defect worth fixing while this feature is in that code.

**Decision**: extend `ensure_registration_allowed` into the single admission
gate — renamed to say what it now decides — and call it from *both* paths,
`resolve_oauth_login` included, before any account is created.

**Rationale**: the spec's central warning is an operator who closes signups,
watches the registration form vanish, and keeps admitting strangers through
Google. One function called from two places is the smallest construction that
makes that impossible, and it is testable by grep as well as by test.

**Alternatives considered**: a middleware layer over both routes — rejected
because OAuth admission is decided mid-callback, after the provider handshake
and after the "does this email match an existing user" lookup, not at request
entry. A check inside `users` insertion — rejected because bootstrap and
invitation redemption legitimately insert users, so the gate would need to be
bypassed by its two most important callers.

---

## §2 — The policy store already has a pattern to copy

`auth_security_settings` (`schema.rs:42`) is a one-row table
(`id -> Int4`, a single boolean, `updated_at`) with three functions around it
in `admin.rs`: `ensure_auth_security_settings` (insert row 1 if absent),
`load_auth_security_settings` (`admin.rs:388`), and `update_two_factor_policy`
(`admin.rs:406`). It is read through `graphql/queries/admin.rs:72`.

**Decision**: `instance_access_settings` follows this shape exactly — one row,
`id = 1`, an `access_policy` column, `ensure_`/`load_`/`update_` in `admin.rs`.

**Rationale**: FR-002 requires a change to take effect without restart, which a
per-request DB read gives for free. Matching the existing singleton means the
admin GraphQL and the admin UI both already have a slot for it.

**Alternatives considered**: an environment variable — rejected outright by
FR-002 and SC-004 ("without editing configuration files or restarting"). A
column on `admin_bootstrap_setup` — rejected because bootstrap state and
ongoing access policy have different lifetimes and FR-011's bootstrap exemption
is easier to reason about when they are separate rows.

---

## §3 — The invitation object already exists one level down

`world_invites` (`schema.rs:826`) carries `invite_code`, `max_uses`,
`used_count`, `expires_at`, `created_by`, `revoked` and `rotated_from` — every
field US2 asks for except the target. Spec 027 built generate / revoke / rotate
/ redeem over it (`graphql/mutations_invites.rs`), and `share_codes.rs`
generates the codes.

**SC-006's concurrency requirement is already solved there.** From
`mutations_invites.rs:240`:

> This replaces a read, an in-memory `is_valid()`, and a write-back of a
> computed count. That sequence lost updates: two joins racing for the last use
> both read `used_count = N`, both computed `N + 1`, and both wrote it —
> admitting two members against one remaining use. Carrying the whole validity
> predicate in the UPDATE's WHERE clause makes the check and the increment one
> indivisible step.

**Decision**: `instance_invitations` mirrors `world_invites` minus `world_id`,
and redemption reuses the conditional-UPDATE pattern verbatim — the validity
predicate (not revoked, not expired, `used_count < max_uses`) lives in the
WHERE clause, and zero rows updated means "unusable" without saying which
reason.

**Rationale**: SC-006 says "at most N accounts, including under simultaneous
redemption attempts", and US2 scenario 4 says a revoked link must be
indistinguishable from an expired one. Both fall out of the shape spec 027
already proved. Re-deriving it would be re-deriving a bug that has already been
fixed once.

**Alternatives considered**: reusing `world_invites` with a nullable
`world_id` — rejected. The spec is emphatic that these are different objects
granted by different people, and a nullable FK would let a world invite and an
instance invitation be confused by any query that forgot the filter. The
`ON DELETE CASCADE` to `worlds` also has no meaning for an instance invitation.

**Open design point carried into data-model**: US2 scenario 8 requires seeing
*which account* redeemed an invitation. With `max_uses > 1` that is a
one-to-many, so a redemption row per account is needed rather than a
`redeemed_by` column on the invitation.

---

## §4 — The unauthenticated status endpoint already exists

FR-003 requires the current policy to be readable by a signed-out visitor, so
the sign-in surface can offer only routes that will work — and requires nothing
else about users or invitations to leak.

`/authentication/setup/status` (`auth/mod.rs:103` → `admin_setup.rs:6`) already
answers unauthenticated, and already returns whether setup is complete plus the
list of enabled, configured OAuth providers. `App.tsx` calls it on every page
load and gates the router on it.

**Decision**: extend that response with the access policy and whether access
requests are accepted. No new public endpoint.

**Rationale**: it is the one surface every client already reads before
rendering anything, it is already the "what shape is this instance" probe, and
adding a field there costs one round trip that is already being made.

**Alternatives considered**: a new `/authentication/access-policy` route —
rejected as a second probe on the same critical path for one enum.

---

## §5 — Where the refusal has to surface

FR-006 requires a closed instance to refuse an unmatched OAuth identity with no
session issued and a message saying the instance is not accepting new accounts.
The OAuth callback is a browser redirect, not a JSON caller, so the refusal has
to arrive as a redirect the front end can render.

**Decision**: refuse inside `resolve_oauth_login` and surface it through the
callback's existing error-redirect path, with a distinct reason code.

**Rationale**: `oauth_callback` (`oauth.rs:95`) already has to convey failures
to a browser; this is one more terminal outcome, not a new mechanism.

**NEEDS CLARIFICATION — resolved by inspection**: whether refusing after a
successful provider handshake leaks anything. It does not: the person proved
control of an identity the instance has no record of, and is told the instance
is closed. That is the same fact `setup/status` already tells anyone.

---

## §6 — Audit trail

FR-004 (policy changes recorded with actor, previous state, new state, time)
and FR-007 (refused admissions visible to an administrator).

The nearest precedent is `content_moderation_actions` (`schema.rs:73`) — an
append-only, admin-visible record of consequential actions, with the submitter
and outcome captured. Spec 015 also proves the public-intake shape US3 will
later need: `submit_takedown_notice_impl`
(`graphql/mutations_moderation.rs:127`) is an unauthenticated mutation reaching
an admin review queue (`ModerationReviewPage.tsx`).

**Decision**: one append-only `instance_access_events` table covering both
policy changes and refused admissions, discriminated by an event type.

**Rationale**: they are read together — an operator asking "is my instance
actually closed" wants the switch history and the refusals on one screen — and
FR-007 explicitly forbids storing credentials, which is easier to guarantee in
one table with one reviewed column set than in two.

**Alternatives considered**: writing refusals to the tracing log only —
rejected by FR-007, which requires an administrator to review them in the
application. A row per refusal is also the only thing that makes SC-001
("zero accounts created … confirming the user count is unchanged") diagnosable
when it fails.

---

## §7 — The admin surface has a slot

`ADMIN_SECTIONS` (`apps/web/src/pages/admin/components/adminSections.ts`) is a
declared list read by both the nav and its test, added to spec 031 FR-032 so
that "adding a section is one entry rather than an edit in three files".
`SecurityPanel.tsx` is the precedent for a settings panel that reads and writes
one policy.

**Decision**: an "Access" section, one `ADMIN_SECTIONS` entry, with a panel
modelled on `SecurityPanel`.

---

## §8 — Scope: the P1 pair only

**Decision**: this plan covers **US1 (the gate) and US2 (instance
invitations)**. US3 (request-access intake), US4 (delivery durability and the
write-only destination) and US5 (retention and rate limiting) are specified but
not planned here.

**Rationale**: the spec itself says US1+US2 "together are the complete minimum
product: closed by default, opened one person at a time", and that is exactly
what a prod demo needs. US1+US2 is largely assembly over machinery that exists
(§2, §3, §4, §7). US3–US5 introduces genuinely new machinery — outbound
delivery with retry and exactly-once semantics, a write-only secret, and a
retention job — none of which the codebase has a precedent for beyond spec
015's intake half, and none of which is needed to hand a demo link to a named
guest.

**What this costs**: the product will briefly have a closed instance with no
self-service way in. That is the intended state for an invite-only demo, and
FR-003 already lets the sign-in surface say so.
