# Feature Specification: Instance Access — Signup Gating, Instance Invitations, and Request-Access Intake

**Feature Branch**: `035-instance-access`

**Created**: 2026-09-01

**Status**: Draft

**Input**: User description: "the whole software needs an allow-signups and invite system for the entire app so I can run a demo in prod eventually. And a request-access-to-webhook option allowing us to host it and accept interest kinda deal — but this feature is more far out."

## Context

The operator wants to run a public demo instance in production **without it
being open to the world**. Today an instance has exactly two states: before
first-run setup (the router is gated, nobody in) and after it (registration is
open, and a configured OAuth provider will provision an account for anyone who
can authenticate against it). There is no state in between, which is the state
a demo actually needs: reachable by strangers, joinable only by people the
operator chose.

**This is about the instance, not about a world.** World-level access is done:
a Game Master issues a revocable, expiring, use-capped invite code that admits
an **existing user** to **one world's table**. This feature is one level up. An
instance invitation admits a **person who has no account at all** to **the
application itself**. The two compose — a stranger needs an instance invitation
to become a user, and then a world invite to sit at a table — but they are
different objects, granted by different people (operator vs. Game Master), and
neither replaces the other. World invitations are explicitly out of scope and
must not be redesigned here.

### The decision this feature turns on

Two accepted decisions disagree about what happens when an unknown person
arrives via a configured OAuth provider:

- The earlier policy said an OAuth identity that matches no existing account is
  refused — account creation stays explicit.
- The current policy reversed that: an unmatched OAuth identity with a
  provider-verified email is auto-provisioned a new account and signed in, in
  one step, to collapse the onboarding funnel.

The current policy is what ships. That means an "allow signups" switch that
only governs the local registration form would be **worse than useless**: an
operator would close signups, see the registration form disappear, believe the
instance is closed, and still be admitting every stranger who clicks "Sign in
with Google". That is not a hypothetical — it is the first bug such an operator
would find in production, and probably not before someone had already walked
in. The spec exists primarily to make that impossible.

**Position taken here**: the instance access policy is a gate that sits *above*
both decisions, and it governs *admission*. The auto-provisioning policy
continues to govern *how an admitted person's account gets created* — derived
username, unusable password, immediate identity link, no silent takeover of an
existing account. It no longer governs *whether* a stranger may be admitted;
the instance policy answers that first, for every path without exception. When
the instance is open, behavior is exactly as it is today. When the instance is
not open, an unmatched, uninvited OAuth identity is refused with the same
outcome as an uninvited local registration attempt — and the earlier
no-auto-provisioning policy is, in effect, what a closed instance restores.
Requirements FR-005 through FR-009 state this in testable terms; anything less
specific leaves the hole open.

Two things are deliberately *not* gated by the policy: first-run
administrator bootstrap (a brand-new instance has no users and must still admit
its first administrator — a closed default must not brick a fresh install), and
sign-in by someone who **already has an account**, which is authentication, not
signup.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An operator closes the instance and it is actually closed (Priority: P1)

An operator is about to point a public URL at their instance. They set the
instance to admit nobody new. From that moment, every path that could create a
new account refuses — the local registration form, a direct registration
request, and a first-time sign-in through every configured OAuth provider.
Existing users are entirely unaffected.

**Why this priority**: This is the whole feature's load-bearing guarantee. An
invitation system built on a gate that leaks is a false sense of security, and
the leak is silent — nothing tells the operator it happened until a stranger
appears in their user list. Nothing else in this spec is worth building first.

**Independent Test**: Set the instance closed on a live instance with at least
one OAuth provider configured, then attempt account creation by every available
route — local form, direct request, and a provider sign-in with an email that
matches no existing account — confirming each is refused, that a pre-existing
user can still sign in and use the app throughout, and that each refusal is
recorded for the operator.

**Acceptance Scenarios**:

1. **Given** an instance set to closed, **When** a visitor loads the
   application, **Then** no sign-up affordance is offered and the sign-in path
   remains available.
2. **Given** an instance set to closed, **When** a visitor submits a local
   registration, **Then** no account is created and the response does not
   reveal whether the submitted identifier already belongs to a user.
3. **Given** an instance set to closed and a configured OAuth provider,
   **When** a person authenticates successfully with that provider using a
   verified email matching no existing account, **Then** no account is created,
   no session is issued, and they are shown that this instance is not accepting
   new accounts.
4. **Given** an instance set to closed and a person whose provider email
   matches an existing account, **When** they complete the provider handshake,
   **Then** the existing account-linking rules apply unchanged and the instance
   policy does not block them — linking an identity to an account that already
   exists is not signup.
5. **Given** an instance set to closed, **When** an existing user with a live
   session uses the application, **Then** nothing about their session, worlds,
   or content changes.
6. **Given** an instance with no administrator yet and a closed default,
   **When** the first-run administrator setup is performed, **Then** it
   succeeds — the policy never applies to first-run bootstrap.
7. **Given** an instance set to closed, **When** an administrator reviews the
   access log, **Then** each refused admission attempt appears with its time,
   the path attempted, and the provider where applicable, and without storing
   credentials.

---

### User Story 2 - An operator invites a specific person into the instance (Priority: P1)

The operator knows who they want in the demo. They issue an instance
invitation, hand over the link, and that person creates an account — by
password or by any configured provider — and lands in the app. The operator can
see which invitations are outstanding, which were used, and can revoke one that
leaked before it is redeemed.

**Why this priority**: A closed instance with no way in is a switched-off
instance. This is the story that makes the demo possible, and together with
Story 1 it is the complete minimum product: closed by default, opened one
person at a time.

**Independent Test**: Issue an invitation on a closed instance, redeem it end to
end as a new person via both the local and a provider route, confirm the
account exists and the invitation cannot be redeemed again beyond its limit,
then revoke a second invitation and confirm redemption fails.

**Acceptance Scenarios**:

1. **Given** an administrator on a closed instance, **When** they issue an
   instance invitation, **Then** they receive a link to share, and the
   invitation is listed as outstanding with its expiry and remaining uses.
2. **Given** a valid unredeemed invitation, **When** the recipient opens the
   link and completes local registration, **Then** an account is created
   despite the instance being closed, they are signed in, and the invitation's
   remaining uses decreases by one.
3. **Given** a valid unredeemed invitation, **When** the recipient opens the
   link and signs in with a configured OAuth provider for the first time,
   **Then** an account is created and linked under the existing
   auto-provisioning rules, and the invitation's remaining uses decreases by
   one.
4. **Given** an invitation an administrator has revoked, **When** anyone opens
   the link, **Then** they are told the invitation is no longer valid, no
   account is created, and the reason is not distinguishable from expiry.
5. **Given** an invitation that has reached its use limit or expiry, **When**
   anyone opens the link, **Then** it is refused in the same way.
6. **Given** an invitation issued while the instance was closed, **When** the
   operator subsequently opens the instance to everyone and later closes it
   again, **Then** the invitation's validity is governed only by its own
   revocation, expiry and use count — the policy switch neither invalidates nor
   extends an already-issued invitation.
7. **Given** an invitation link, **When** it is redeemed, **Then** the
   resulting account is a first-class user identical in every respect to one
   created on an open instance, with no residual invited-user status affecting
   ownership, export, or deletion.
8. **Given** an administrator, **When** they view an invitation that has been
   redeemed, **Then** they can see which account redeemed it and when.

---

### User Story 3 - A stranger asks for access and the operator decides (Priority: P2)

A stranger lands on the closed demo instance and sees that they can ask to be
let in. They submit a way to reach them and a short note about why. The
submission reaches the operator through a destination the operator configured.
The operator reads it and, if they like it, issues that person an instance
invitation.

**Why this priority**: This is the "accept interest" half of the request, and
the operator described it as further out. It depends on Story 2 — a request
without an invitation to grant is a dead end — and the instance is demo-ready
without it. It is separated because it is also the only part that stores
personal data submitted by unauthenticated strangers, and that is a distinct
review surface.

**Independent Test**: With a delivery destination configured, submit a request
from an unauthenticated browser, confirm the operator receives it with its
contents intact, confirm the submitter is told only that it was received, and
confirm the operator can act on it by issuing an invitation from the request.

**Acceptance Scenarios**:

1. **Given** an instance that is closed with requests enabled, **When** an
   unauthenticated visitor loads the sign-in page, **Then** they are offered a
   way to request access.
2. **Given** the request form, **When** a visitor submits a contact address and
   a note, **Then** they see a neutral confirmation that does not reveal
   whether that address already belongs to an account.
3. **Given** a submitted request, **When** the operator checks the configured
   destination, **Then** the request has been delivered there with the
   submitted contact address, the note, and the time of submission.
4. **Given** a submitted request, **When** an administrator views the pending
   requests in the application, **Then** the request is listed whether or not
   its delivery has succeeded.
5. **Given** a pending request, **When** an administrator approves it, **Then**
   an instance invitation is issued for that request and the request is marked
   handled, with the administrator and time recorded.
6. **Given** a pending request, **When** an administrator declines it, **Then**
   no invitation is issued, the request is marked handled, and the submitter is
   not automatically contacted.
7. **Given** requests are disabled or the instance is open to everyone,
   **When** a visitor loads the sign-in page, **Then** no request form is
   offered and a direct submission is refused.

---

### User Story 4 - No request is lost, and no destination is leaked (Priority: P2)

The operator's delivery destination is temporarily unreachable. Requests keep
arriving. When the destination comes back, or when the operator looks at the
application, nothing has been dropped. Meanwhile the destination itself — which
is a secret, since possessing it lets anyone post to the operator's inbox — is
never shown back to anyone, including administrators.

**Why this priority**: Same priority as Story 3 because it is the half of
Story 3 that determines whether it can be trusted in production. A request that
vanishes because a destination 500'd is a person the operator never knew wanted
in.

**Independent Test**: Configure a destination that fails, submit several
requests, confirm every one is retrievable in the application and marked
undelivered; restore the destination and confirm outstanding requests are
delivered without duplicating those already delivered; confirm the configured
destination is never returned in readable form to any caller.

**Acceptance Scenarios**:

1. **Given** a configured destination that is unreachable, **When** a visitor
   submits a request, **Then** the submitter still receives a successful
   confirmation and the request is durably recorded.
2. **Given** requests recorded but not delivered, **When** delivery becomes
   possible again, **Then** each is delivered and each is delivered no more
   than once in a way the receiving end can detect as a repeat.
3. **Given** an administrator viewing the delivery configuration, **When** the
   destination has been set, **Then** they can see that one is configured and
   when it last succeeded or failed, but never the destination value itself.
4. **Given** an administrator, **When** they replace or clear the destination,
   **Then** the change takes effect for subsequent deliveries and the previous
   value is not recoverable through the application.
5. **Given** no destination is configured at all, **When** a request is
   submitted, **Then** it is still recorded and visible to administrators, and
   the operator is told in the administrative interface that no destination is
   configured.
6. **Given** a delivered request, **When** the receiving end examines it,
   **Then** it can establish that the delivery came from this instance and was
   not forged or replayed by a third party.

---

### User Story 5 - The instance holds strangers' data responsibly (Priority: P3)

Contact details submitted by people who never became users do not accumulate
forever. Handled requests age out on a stated schedule, an administrator can
delete a request immediately, and the public form cannot be used to flood the
instance or the operator's destination.

**Why this priority**: An obligation rather than a capability, and it becomes
pressing only once Story 3 is live and the form is public. Separated so that it
is reviewed on its own terms rather than assumed to have been handled inside
the feature that creates the exposure.

**Independent Test**: Submit requests, confirm the stated retention window is
enforced without administrator action, confirm an immediate deletion removes
the contact details, and confirm sustained submission from one source is
throttled while a legitimate submission from elsewhere still succeeds.

**Acceptance Scenarios**:

1. **Given** the request form, **When** it is presented, **Then** it states
   what is collected, why, how long it is kept, and how to ask for its removal,
   before anything is submitted.
2. **Given** a request that has been handled, **When** the stated retention
   window elapses, **Then** its contact details are no longer held, without an
   administrator having to act.
3. **Given** any request, **When** an administrator deletes it, **Then** its
   contact details are removed immediately and the fact that a request existed
   and how it was handled may be retained without them.
4. **Given** a single source submitting repeatedly, **When** it exceeds the
   permitted rate, **Then** further submissions are refused for a period while
   submissions from other sources are unaffected.
5. **Given** an abusive or unlawful submission, **When** an administrator
   reviews it, **Then** they can remove it and block further submissions from
   that source without removing the record that action was taken.

---

### Edge Cases

- **A closed instance with no invitations and no requests enabled** is a
  correct configuration: only existing users can get in. It must not be an
  error state.
- **The last administrator on a closed instance loses access.** The instance is
  now unreachable by anyone who could reopen it. The recovery path is
  out-of-band (operator access to the deployment), and this must be stated
  rather than discovered.
- **An OAuth handshake begun while the instance was open and completed after it
  closed.** Admission is evaluated at the moment the account would be created,
  not when the handshake began — a half-finished handshake confers nothing.
- **An invitation issued and then the instance opened to everyone.** The
  invitation is still valid and still consumes a use when redeemed; it is not
  retroactively cancelled, because the operator may close the instance again.
- **A provider that returns no email address.** Nothing changes: there is
  nothing to provision, and the attempt is refused whatever the instance policy
  says.
- **An invitation redeemed by someone whose email already has an account.**
  The invitation is not consumed; they are directed to sign in, and the
  existing account is never taken over.
- **The same person submits a request twice**, or submits a request while
  already holding an unredeemed invitation. Duplicate handling must not confirm
  or deny to the submitter that either is true.
- **A request submitted with a contact address that already belongs to a
  user.** Response is identical to any other submission; the operator sees the
  overlap, the submitter does not.
- **The delivery destination is reconfigured while deliveries are outstanding.**
  Outstanding requests go to the new destination.
- **A destination that accepts the delivery but responds slowly.** The
  submitter's confirmation must not wait on it.
- **The instance is opened to everyone.** Instance invitations are unnecessary
  but existing ones stay valid; the request-access form is not offered.

## Requirements *(mandatory)*

### Functional Requirements

#### Instance access policy

- **FR-001**: The instance MUST have a single access policy with exactly three
  states — **open** (anyone may create an account), **invite-only** (an account
  may be created only by redeeming a valid instance invitation), and **closed**
  (no account may be created by any means) — with exactly one state in effect
  at any moment.
- **FR-002**: An administrator MUST be able to read and change the instance
  access policy, and the change MUST take effect for all subsequent admission
  attempts without restarting or redeploying the instance.
- **FR-003**: The application MUST expose the instance's current policy, and
  whether access requests are being accepted, to unauthenticated visitors so
  that the sign-in surface can present only the routes that will actually work.
  It MUST NOT expose anything else about the instance's users or invitations to
  an unauthenticated caller.
- **FR-004**: Every change of the access policy MUST be recorded with the
  administrator who made it, the previous state, the new state, and the time.

#### Admission — the paths the policy governs

- **FR-005**: The access policy MUST govern **every** path that can result in a
  new user account existing, without exception. At minimum this includes local
  registration and first-time sign-in through each configured OAuth provider.
- **FR-006**: When the policy is **closed**, a successful authentication with a
  configured OAuth provider whose verified email matches no existing account
  MUST NOT create an account and MUST NOT issue a session, regardless of the
  auto-provisioning behavior that applies on an open instance.
- **FR-007**: When the policy is **invite-only**, the outcome in FR-006 MUST be
  the same unless the attempt carries a valid, unexpired, unrevoked instance
  invitation with uses remaining, in which case the account MUST be created and
  the invitation consumed.
- **FR-008**: When the policy is **open**, admission behavior MUST be
  unchanged from the instance's current behavior for both local registration and
  first-time provider sign-in.
- **FR-009**: The access policy MUST NOT affect any of the following:
  authenticating as an existing account by password or by an already-linked
  provider identity; linking a provider identity to an account that already
  exists (including the confirmation that flow requires); or any existing
  session, world membership, or content.
- **FR-010**: The access policy MUST NOT apply to first-run administrator
  bootstrap. An instance with no administrator MUST be able to create its first
  administrator by every route that supports it today, whatever the policy's
  default value is.
- **FR-011**: A refused admission MUST NOT reveal whether a submitted
  identifier already corresponds to an existing account, and refusals MUST be
  indistinguishable across the reasons an invitation can be invalid (revoked,
  expired, exhausted, never existed).
- **FR-012**: Every refused admission MUST be recorded for administrator review
  with the time, the route attempted, the provider where applicable, and the
  policy state at the time — and MUST NOT record submitted credentials.
- **FR-013**: A newly created instance MUST default to a policy that does not
  admit strangers, and this default MUST NOT prevent FR-010.

#### Instance invitations

- **FR-014**: An administrator MUST be able to issue an instance invitation
  that produces a shareable link, and MUST be able to set its expiry and how
  many times it may be redeemed.
- **FR-015**: An administrator MUST be able to revoke an unredeemed instance
  invitation, after which it MUST NOT admit anyone.
- **FR-016**: An instance invitation MUST be redeemable through both local
  registration and first-time provider sign-in, and MUST be consumed exactly
  once per account it admits, including when concurrent redemptions race.
- **FR-017**: An instance invitation MUST NOT confer any world membership,
  world role, or content permission. An account created by redeeming one MUST
  be indistinguishable in ownership, export, deletion, and permission terms
  from any other account.
- **FR-018**: Administrators MUST be able to list instance invitations with,
  for each, its state (outstanding, redeemed, revoked, expired), its expiry,
  its remaining uses, who issued it, and which accounts redeemed it and when.
- **FR-019**: Instance invitation codes MUST be unguessable at the same
  strength as the existing world access links, and MUST NOT be recoverable from
  the application after issuance except to the administrators entitled to list
  them.
- **FR-020**: Instance invitations MUST remain governed solely by their own
  revocation, expiry, and use count; a change of instance access policy MUST
  NOT invalidate or extend an already-issued invitation.

#### Request access

- **FR-021**: An administrator MUST be able to enable or disable inbound access
  requests independently of the access policy.
- **FR-022**: When enabled, an unauthenticated visitor MUST be able to submit a
  contact address and a short free-text note, and MUST receive a confirmation
  that is identical whether or not that address already corresponds to a user
  or to an existing request.
- **FR-023**: Every submitted request MUST be durably recorded before the
  submitter is confirmed, and MUST remain retrievable by administrators
  regardless of whether external delivery has succeeded.
- **FR-024**: Administrators MUST be able to list requests, see each one's
  submission time, contents, and delivery state, and mark each as approved
  (issuing an instance invitation for it in one action) or declined, with the
  deciding administrator and time recorded.
- **FR-025**: Requests MUST be rate limited per source at least as strictly as
  the existing authentication routes, and the limit MUST NOT be disableable in
  a production build.
- **FR-026**: Submitted free-text MUST be treated as untrusted: it MUST be
  displayed to administrators without being interpreted as markup or
  instructions, and administrators MUST be able to delete an abusive submission
  and refuse further submissions from its source, consistent with the
  platform's moderation obligations.
- **FR-027**: When requests are disabled or the policy is open, the request
  surface MUST NOT be offered and a direct submission MUST be refused.

#### Delivery to the operator

- **FR-028**: An administrator MUST be able to configure a single delivery
  destination for access requests, and MUST be able to replace or clear it.
- **FR-029**: The configured destination MUST be treated as a secret: it MUST
  NOT be returned in readable form to any caller, including administrators,
  after it has been set, and MUST NOT appear in logs or diagnostics.
- **FR-030**: Administrators MUST be able to see whether a destination is
  configured and the outcome and time of the most recent delivery attempt,
  without seeing the destination.
- **FR-031**: Delivery failure MUST NOT lose a request and MUST NOT fail the
  submitter's submission. Undelivered requests MUST be retried until delivered
  or until an administrator stops retrying, and delivery MUST NOT block the
  submitter's response.
- **FR-032**: Each request MUST be delivered at most once successfully, and
  every delivery MUST carry an identifier that lets the receiving end recognise
  a repeat of one it has already seen.
- **FR-033**: A delivery MUST carry evidence of its origin sufficient for the
  receiving end to reject a forged or replayed delivery.
- **FR-034**: A delivery MUST contain only what the submitter provided plus the
  submission time and identifier. It MUST NOT contain any credential, session,
  or data about the instance's existing users.

#### Data protection

- **FR-035**: The request form MUST state, before submission, what is
  collected, why, how long it is retained, and how to ask for removal.
- **FR-036**: A handled request's contact details MUST be removed automatically
  once a stated retention period has elapsed, without administrator action.
- **FR-037**: An administrator MUST be able to delete a request's contact
  details immediately on request from its submitter, retaining at most the fact
  that a request existed and how it was handled.
- **FR-038**: A request's contact details MUST NOT be used for anything other
  than deciding on and communicating about that request.

### Key Entities

- **Instance Access Policy**: The single instance-wide admission state — open,
  invite-only, or closed — plus whether access requests are accepted. Exactly
  one exists per instance. Read by every admission path and by the
  unauthenticated sign-in surface.
- **Instance Invitation**: An operator-issued, unguessable, expiring,
  use-capped, revocable grant admitting a person **who has no account** to the
  application. Records who issued it, its limits and state, and which accounts
  redeemed it. Distinct from a world invite, which admits an **existing user**
  to one world's table and confers world membership; an instance invitation
  confers no world membership at all.
- **Access Request**: An unauthenticated stranger's submitted contact address,
  note, and submission time, plus its delivery state and its handling
  disposition (pending, approved with the invitation issued, declined) with the
  deciding administrator and time. Subject to retention limits; the only entity
  in the application holding personal data about people who are not users.
- **Delivery Destination**: The operator-configured, write-only target for
  outbound access-request deliveries. Its value is never readable back; only
  its configured/not-configured state and last-attempt outcome are visible.
- **Access Event**: An append-only record of admission decisions and policy
  changes — refusals with route and provider, redemptions, policy transitions,
  request dispositions — held for administrator review and never containing
  credentials.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the instance closed, **zero** accounts can be created by any
  route. Demonstrated by attempting account creation via local registration and
  via every configured OAuth provider with an unmatched verified email, and
  confirming the instance's user count is unchanged.
- **SC-002**: With the instance closed, 100% of existing users can still sign
  in and reach their worlds — no existing session or membership is disturbed by
  any policy state.
- **SC-003**: An instance created from scratch with the shipped default admits
  its first administrator successfully, with no configuration change required
  beforehand.
- **SC-004**: An operator can take an instance from open to invited-guests-only
  and hand a working link to a named guest in under 3 minutes, without editing
  configuration files or restarting the instance.
- **SC-005**: A revoked instance invitation admits nobody: 0 of any number of
  redemption attempts after revocation results in an account.
- **SC-006**: An invitation with N remaining uses admits at most N accounts,
  including under simultaneous redemption attempts.
- **SC-007**: A stranger can submit an access request in under 60 seconds, and
  the confirmation they receive is byte-identical whether or not their address
  already corresponds to an account.
- **SC-008**: With the delivery destination failing for the entire submission
  window, 100% of submitted requests are still visible to an administrator, and
  once the destination recovers, each is delivered exactly once.
- **SC-009**: The configured destination value cannot be retrieved through any
  application surface, log, or export after being set — verified by inspecting
  every administrator-visible view and the instance's logs.
- **SC-010**: Handled requests hold no contact details past the stated
  retention period, verified without any administrator action being taken.
- **SC-011**: Sustained submission from one source is refused after the
  published per-source limit within a single minute, while a submission from a
  different source in the same minute succeeds.
- **SC-012**: An administrator can go from an inbound request to that person
  holding a usable invitation in a single action.

## Assumptions

- The current auto-provisioning behavior on first OAuth login is the shipping
  behavior and is desirable on an **open** instance; this feature gates it
  rather than reverting it. Reverting it repo-wide is explicitly not proposed.
- World-level invitations are complete and out of scope. This feature adds no
  world membership, role, or content permission of any kind.
- The instance-wide state this hangs on is the same first-run/setup state the
  application already consults on load, so the sign-in surface can learn the
  policy without an extra authenticated call.
- "Administrator" means the existing instance administrator role; no new role
  is introduced. Only administrators manage the policy, invitations, requests,
  and destination.
- The operator's recovery path for an instance whose last administrator is
  locked out is out-of-band deployment access; no in-application recovery is
  specified here.
- Access requests are not email; the instance sends no mail to submitters. The
  operator contacts an approved person themselves using the address supplied.
- The default access policy for a **new** instance is invite-only rather than
  closed, so that first-run bootstrap and subsequent operator-issued
  invitations both work without a first configuration step. **Operator
  confirmation needed** — closed is the more conservative choice if the
  operator prefers to open the gate explicitly.
- A default retention period for handled access requests is assumed at 90 days.
  **Operator confirmation needed** — the number should be whatever the operator
  is willing to state publicly on the form.
- One delivery destination per instance is sufficient; multiple destinations,
  routing rules, and per-request destinations are out of scope.
- No specific delivery protocol, storage shape, or scheduling mechanism is
  chosen here; only the outcomes in FR-028 through FR-034 are required.
