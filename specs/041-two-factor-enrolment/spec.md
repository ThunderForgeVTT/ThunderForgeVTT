# Feature Specification: Two-Factor Authentication People Can Turn On, Keep, and Recover

**Feature Branch**: `041-two-factor-enrolment`

**Created**: 2026-09-07

**Status**: Draft — **specification only, not being built yet.** Written
alongside 036–040.

**Input**: Project owner: "maybe we spec out 2fa via totp_rs — create a TOTP instance for a user, generate a QR code provisioning URL for the client, verify the user-submitted code — and database backings for it, and all, so we can support 2fa apps?"

## Context

### The engine is already here, and it is good

Checked before writing, because a spec that rebuilds a shipped thing wastes a
week:

- **`totp-rs 6.0.0`** with the `otpauth` feature is already a dependency of
  both `src/server` and `crates/thunderforge-axum-auth-core`.
- **`crates/thunderforge-axum-auth-core/src/totp.rs`** already builds exactly
  the verifier proposed — SHA1, 6 digits, a 30-second step, a skew of one, and
  the issuer `ThunderForge`. It goes further than the sketch in one way that
  matters: every rule takes an **explicit Unix timestamp**, so "does a code
  verify one step late?" is a question a test can answer instead of waiting
  thirty seconds for it.
- **The database backing exists**: `users.two_factor_enabled`,
  `two_factor_secret_encrypted` (encrypted at rest, not merely stored),
  `two_factor_confirmed_at`, `two_factor_admin_required`, a
  `login_two_factor_challenges` table, and an instance-wide
  `two_factor_required_for_all_users`.
- **The provisioning URI is already produced.** `two_factor_setup_start`
  returns `otpauth://totp/ThunderForge:<user>?secret=…&issuer=ThunderForge` —
  the string an authenticator app scans.
- **It is now tested.** `apps/web/e2e/two-factor.spec.ts` proves enrolment,
  the login challenge, wrong, stale and replayed codes, and the instance-wide
  policy, against a real stack.

**So this feature is not "add TOTP".** It is everything around TOTP that
decides whether a person can switch it on, keep working after they do, and get
back in when something goes wrong.

### What is actually missing

**1. Nobody can turn it on.** There is no enrolment interface anywhere in the
application. The only two-factor code in `apps/web/src` is the login challenge
step and the admin policy switch; the setup endpoints are called from nowhere.
A capability MVP.md lists as shipped can be enabled only by hand-rolled API
calls, which means in practice it is enabled by nobody.

**2. Losing a phone loses the account.** Nothing in the codebase mentions
recovery codes, backup codes, or any second way in. Today the only path back
is an operator editing the database, which is not a feature — it is the
absence of one.

**3. A confirmed second factor can be removed with the password alone.**
`two_factor_setup_start` authenticates on the password and immediately clears
`two_factor_enabled` with a fresh secret, so possession of the first factor is
never proven in order to remove the second. And there is no deliberate "turn
it off" path at all, so that side effect is currently the only way off — an
accident doing the job of a decision.

**4. The instance-wide switch is a lockout button.** The requirement is ORed
in globally, verification returns false for anybody with no stored secret, and
the challenge screen offers no way to enrol — so turning the policy on
permanently bars every account that had not already enrolled through the API
by hand. Which, per (1), is all of them.

**5. A code is not bound to the step it matched.** The verifier drops the step
`check_current` returns, and `totp.rs`'s own comment says what that value is
for: refusing to accept the same step twice. The same six digits stay usable
across the skew window against a fresh challenge.

**6. Per-user enforcement has no surface.** `two_factor_admin_required` exists
as a column and an endpoint, and is reachable only by hand-rolled REST.

### The shape of the problem

Every one of those is the same shape: **the cryptography was finished and the
human path was not.** A second factor is not a verifier — it is an enrolment
somebody completes, a recovery they can reach at their worst moment, and a
removal that costs what adding it cost.

## Clarifications

### Session 2026-09-07

- Q: How much of the enrolment interface is in scope? → A: **All of it, and it
  is the priority.** Not an endpoint with a form bolted on: the QR code
  rendered from the provisioning URI, the secret in a form somebody can type
  by hand when they cannot scan, a confirming code before anything takes
  effect, the recovery codes presented once afterwards, and the same flow
  reachable from account settings, from first-run setup and from a login that
  requires enrolment. One flow, three entrances.
- Q: Is a second factor optional for administrators? → A: **No. Administrators
  always have one**, from the moment an instance exists. The person who brings
  an instance up completes enrolment during first-run setup and cannot finish
  setup without it, and anybody later made an administrator enrols before
  they can act as one.
- Q: And for everybody else? → A: **Optional by default, with the
  instance-wide requirement available** to an operator who wants it for the
  whole instance. That switch stays what it is today — a policy an operator
  chooses — while the administrator rule is not a policy and cannot be
  switched off.
- Q: Where does mail fit? → A: Setup configures mail and the administrator's
  second factor in the same pass (spec 040 owns the mail half). Enrolment MUST
  NOT depend on mail working — the first administrator of a fresh instance may
  have no working mail server at the moment they enrol, and that must not be
  what stands between them and their own instance.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The enrolment flow (Priority: P1)

Someone opens their account settings, chooses to add a second factor, scans a
QR code with their authenticator app — or types the secret in by hand if they
cannot scan — enters a code to prove it worked, is shown their recovery codes
once, and is protected from then on.

**This is the build-out, and it is the centre of the feature.** The same flow
is reached from three places and behaves the same in all of them:

1. **From account settings**, by somebody choosing to add one.
2. **During first-run setup**, by the administrator bringing an instance up
   (US3) — where it is not optional and setup does not finish without it.
3. **At a login that requires enrolment**, when an instance-wide policy or an
   administrator's decision means this account must have one (US5, US6).

The steps are the same each time: show the code and the secret → take a
confirming code → issue recovery codes → say plainly that it is now on. What
differs between the three is only where the person came from and where they go
afterwards.

**Why this priority**: without it the feature does not exist for anybody who
is not writing API calls, which is everybody. It is also the prerequisite for
every other story here — an administrator cannot be required to enrol through
a flow that does not exist.

**Independent Test**: enrol from the interface, then sign out and back in, and
be challenged.

**Acceptance Scenarios**:

1. **Given** an account without a second factor, **When** the person opens
   their security settings, **Then** they are offered to add one.
2. **Given** enrolment has begun, **When** the person is shown the setup,
   **Then** they get both a scannable code and the secret in a form they can
   type into an app by hand.
3. **Given** an authenticator has been set up, **When** the person enters a
   current code, **Then** the second factor takes effect and they are told so.
4. **Given** enrolment has begun and is not confirmed, **When** the person
   abandons it, **Then** nothing changes about how they sign in.
5. **Given** a confirmed second factor, **When** the person returns to their
   settings, **Then** they can see it is on and when it was confirmed.
6. **Given** a person who cannot scan — no camera, a desktop authenticator, a
   password manager — **When** they enrol, **Then** the secret is available in
   a form they can copy and type, not only as an image.
7. **Given** any of the three entrances to enrolment, **When** a person
   completes it, **Then** the steps and the wording are the same; only where
   they arrive afterwards differs.
8. **Given** an enrolment in progress, **When** the person mistypes the
   confirming code, **Then** they can try again without starting over or
   re-scanning.

---

### User Story 2 - Getting back in without the phone (Priority: P1)

At enrolment the person is given a set of one-time recovery codes and told to
keep them somewhere else. When their phone is lost, stolen, or wiped, one of
those codes gets them in. Each works once.

**Why this priority**: equal to US1, and the reason US1 is safe to ship. A
second factor with no recovery is a way to lose an account, and the person it
happens to has done nothing wrong.

**Independent Test**: enrol, take the recovery codes, then sign in with one in
place of a code — and confirm the same one cannot be used twice.

**Acceptance Scenarios**:

1. **Given** enrolment is being confirmed, **When** it completes, **Then** the
   person is given recovery codes and told plainly what they are for and that
   they will not be shown again.
2. **Given** a person at the challenge with no authenticator, **When** they use
   a recovery code, **Then** they are signed in.
3. **Given** a recovery code that has been used, **When** it is presented
   again, **Then** it is refused.
4. **Given** recovery codes are running low, **When** the person signs in,
   **Then** they are told how many remain and offered a new set.
5. **Given** a new set is generated, **When** it is issued, **Then** every
   earlier code stops working.

---

### User Story 3 - An administrator always has one (Priority: P1)

The person bringing up a new instance completes first-run setup and, as part
of it, enrols a second factor. They cannot finish setup without it. Anybody
later made an administrator enrols before they can act as one. There is no
switch that turns this off.

**Why this priority**: an administrator holds the instance — its members, its
content, its access policy and its legal contacts. An instance whose
administrator is protected by a password alone is an instance one leaked
password away from belonging to somebody else, and the first administrator is
created at the exact moment nobody has thought about security yet. Doing it at
setup is the only time it is free.

**Independent Test**: bring up a fresh instance and confirm setup cannot be
completed without the administrator enrolling, and that the resulting account
is challenged at its next sign-in.

**Acceptance Scenarios**:

1. **Given** a fresh instance, **When** the first administrator completes
   setup, **Then** enrolment is part of it and setup does not finish until the
   second factor is confirmed.
2. **Given** that enrolment, **When** it completes, **Then** the recovery codes
   are shown and can be saved before setup ends — this is the moment they
   matter most, because a fresh instance may have no working mail and nobody
   else who could help.
3. **Given** a fresh instance with no mail configured, **When** the
   administrator enrols, **Then** it works anyway: enrolment MUST NOT depend on
   the instance being able to send a message.
4. **Given** an existing account, **When** it is made an administrator,
   **Then** it must enrol before it can act as one, and is taken through
   enrolment rather than refused.
5. **Given** an instance upgraded from a version without this rule, **When**
   its existing administrators next sign in, **Then** they are taken through
   enrolment rather than locked out.
6. **Given** an administrator with a second factor, **When** anybody looks for
   a way to switch the administrator requirement off, **Then** there is none —
   it is not a policy.
7. **Given** an administrator who loses their second factor, **When** they use
   a recovery code, **Then** they get in — and where they have lost those too,
   US7's reset path is the answer rather than a database edit.

---

### User Story 4 - Turning it off, on purpose (Priority: P1)

Someone who no longer wants a second factor removes it deliberately, and doing
so costs what adding it cost: they prove they still hold it.

**Why this priority**: it closes the defect where beginning a new enrolment
silently strips a confirmed factor on the password alone. P1 because it is a
security hole, not a convenience.

**Independent Test**: attempt to remove the factor with the password only and
be refused; remove it with password plus a current code and succeed.

**Acceptance Scenarios**:

1. **Given** a confirmed second factor, **When** somebody tries to remove it
   with the password alone, **Then** they are refused.
2. **Given** a confirmed second factor, **When** the person proves they hold it
   and asks to remove it, **Then** it is removed and they are told.
3. **Given** a confirmed second factor, **When** a new enrolment is started,
   **Then** the existing factor MUST stay in force until a replacement is
   confirmed — starting again never disarms anything.
4. **Given** a removal, **When** it happens, **Then** the person is notified
   through a channel other than the session that did it.

---

### User Story 5 - Requiring it of everybody, without locking anybody out (Priority: P2)

An operator turns on the instance-wide requirement. People who have not
enrolled are walked through enrolling at their next sign-in rather than being
refused.

**Why this priority**: the switch exists and is enforced today; what it lacks
is the path that makes it safe to use. P2 because an operator can leave it
alone until then — but leaving it alone is exactly what its current shape
forces.

**Independent Test**: with the requirement on, sign in as an account that never
enrolled and complete enrolment in that same flow.

**Acceptance Scenarios**:

1. **Given** the requirement is on and an account has no second factor,
   **When** they sign in with a correct password, **Then** they are taken
   through enrolment rather than refused.
2. **Given** that enrolment completes, **When** it does, **Then** they
   continue to where they were going.
3. **Given** the requirement is on, **When** an operator looks at their
   instance, **Then** they can see how many accounts have enrolled and how many
   have not.
4. **Given** the requirement is turned off, **When** it is, **Then** everybody
   who enrolled keeps their second factor.

---

### User Story 6 - Requiring it of one person (Priority: P3)

An administrator requires a second factor of a particular account —
another administrator, or a member with wide access — without imposing it on
everybody.

**Why this priority**: the column and the endpoint already exist with no way
to reach them. Small, and last, because the instance-wide switch covers the
common case.

**Independent Test**: require it of one account from the administration
surface and confirm that account and no other is asked to enrol.

**Acceptance Scenarios**:

1. **Given** an administrator viewing an account, **When** they require a
   second factor of it, **Then** that account is taken through enrolment at
   next sign-in and no other account is affected.
2. **Given** such a requirement, **When** an administrator views the account,
   **Then** they can see it is required and by whom.

---

### User Story 7 - When somebody has genuinely lost everything (Priority: P3)

A person has lost the authenticator and the recovery codes. There is a defined
path back that an operator can follow, that is recorded, and that does not
involve editing the database.

**Why this priority**: it will happen, and the current answer is a database
edit. P3 because US2's recovery codes make it rare.

**Independent Test**: an operator resets a locked-out account's second factor
and the reset appears in the record.

**Acceptance Scenarios**:

1. **Given** a locked-out account, **When** an operator resets its second
   factor, **Then** the account can sign in with its password and is taken
   through enrolment again.
2. **Given** such a reset, **When** it happens, **Then** it is recorded — who
   did it, for whom, and when — and the account holder is notified.
3. **Given** an instance with no operator able to do this, **When** somebody is
   locked out, **Then** the product says who can help rather than leaving them
   at a dead end.

---

### Edge Cases

- The person's device clock is wrong by more than the accepted window.
- Somebody enters codes repeatedly, guessing. Attempts are limited without a
  legitimate mistype locking anybody out.
- The same code is presented twice inside the accepted window — once to sign
  in, once to authorise something else.
- Enrolment is confirmed on one device while the account is signed in on
  others.
- A recovery code is used at the same moment as a valid authenticator code.
- The last administrator of an instance loses their second factor.
- An account signs in through an external provider and never sees a password
  or a challenge — what the requirement means for them must be answered rather
  than assumed.
- Recovery codes are generated but the person never saves them and closes the
  page.
- An account with a second factor is deleted, or has its data exported.

## Requirements *(mandatory)*

### Functional Requirements

**Enrolment**

- **FR-001**: A person MUST be able to add a second factor from their own
  account settings, without using an interface intended for developers.
- **FR-002**: Enrolment MUST present both a scannable code and the secret in a
  form that can be typed into an authenticator by hand.
- **FR-003**: A second factor MUST NOT take effect until the person has proved
  it works by entering a current code.
- **FR-004**: An abandoned or unconfirmed enrolment MUST leave the account
  exactly as it was.
- **FR-005**: A person MUST be able to see whether a second factor is in force
  on their account and when it was confirmed.

- **FR-001a**: The enrolment flow MUST be one flow, reachable from account
  settings, from first-run setup, and from a sign-in that requires enrolment —
  identical in its steps and wording, differing only in where the person
  arrives afterwards.
- **FR-001b**: Enrolment MUST NOT require the instance to be able to send a
  message. A fresh instance may have no working mail server at the moment its
  first administrator enrols.
- **FR-001c**: A mistyped confirming code MUST NOT discard the enrolment in
  progress or require re-scanning.

**Recovery codes**

- **FR-006**: Confirming enrolment MUST issue a set of one-time recovery
  codes, shown once, with a plain statement of what they are for.
- **FR-007**: A recovery code MUST be accepted in place of an authenticator
  code at the login challenge.
- **FR-008**: A recovery code MUST work exactly once.
- **FR-009**: Recovery codes MUST be stored so that the instance cannot
  display them again after they are issued.
- **FR-010**: A person MUST be able to generate a fresh set, and doing so MUST
  invalidate every earlier code.
- **FR-011**: A person whose remaining codes are running low MUST be told, and
  offered a new set.

**Removing it**

- **FR-012**: Removing a confirmed second factor MUST require proof that the
  person still holds it, in addition to their password.
- **FR-013**: Starting a new enrolment MUST NOT disable, clear or weaken a
  confirmed second factor. The existing one stays in force until a replacement
  is confirmed.
- **FR-014**: A person MUST have a deliberate way to turn a second factor off;
  it MUST NOT be reachable only as the side effect of another action.
- **FR-015**: Adding or removing a second factor MUST notify the account
  holder by a route other than the session that performed it.

**Verification**

- **FR-016**: A code MUST NOT be accepted twice, including within the window
  where it remains cryptographically valid.
- **FR-017**: Repeated incorrect codes MUST be limited, and the limit MUST NOT
  turn an honest mistype into a lockout.
- **FR-018**: A refusal MUST NOT disclose whether the account has a second
  factor, whether a code was close, or which of the two steps failed for a
  reason the caller can act on maliciously.

**Requiring it**

- **FR-019**: With a requirement in force, an account that has not enrolled
  MUST be taken through enrolment after a correct password, and MUST NOT be
  refused entry for not having done so beforehand.
- **FR-020**: Completing that enrolment MUST return the person to what they
  were doing.
- **FR-021**: An operator MUST be able to see how many accounts have a second
  factor and how many do not.
- **FR-022**: Turning a requirement off MUST leave every existing second
  factor in force.
- **FR-023**: An administrator MUST be able to require a second factor of one
  account from the administration surface, and to see that it is required and
  who required it.

**Administrators always have one**

- **FR-027**: An administrator MUST hold a second factor. This is a property
  of the role, not a policy, and MUST NOT be switchable off.
- **FR-028**: First-run setup MUST NOT complete until the first
  administrator's second factor is confirmed.
- **FR-029**: The first administrator's recovery codes MUST be issued and
  presentable before setup ends — the moment they matter most, because a fresh
  instance has no mail, no second administrator and nobody to ask.
- **FR-030**: An account granted administrator MUST enrol before it can act as
  one, and MUST be taken through enrolment rather than refused.
- **FR-031**: An instance upgraded from a version without this rule MUST take
  its existing administrators through enrolment at their next sign-in, and
  MUST NOT lock them out.
- **FR-032**: An account that loses administrator MUST keep its second factor;
  losing a role never removes protection.
- **FR-033**: For everybody who is not an administrator, a second factor MUST
  be optional by default, and the instance-wide requirement (FR-019 to FR-022)
  MUST remain an operator's choice.

**Locked out**

- **FR-024**: An operator MUST be able to reset an account's second factor
  through a defined path, without editing the database.
- **FR-025**: Such a reset MUST be recorded — who, for whom, when — and MUST
  notify the account holder.
- **FR-026**: A person locked out of an instance MUST be told who can help
  them, rather than being left at a dead end.

### Key Entities

- **Second factor**: an account's confirmed authenticator, with when it was
  confirmed and whether it is in force.
- **Recovery code**: one single-use way past the challenge, belonging to one
  account, unreadable after it is issued.
- **Login challenge**: the outstanding second step of one sign-in attempt.
- **Requirement**: why an account must hold a second factor — because it is an
  administrator (always, and not a policy), because the instance's policy says
  so, or because an administrator decided it for that account.
- **Second-factor event**: an addition, a removal, a reset or a recovery-code
  use, recorded so an account holder and an operator can see what happened.

## Success Criteria *(mandatory)*

- **SC-001**: A person can add a second factor from their account settings in
  under three minutes, using an ordinary authenticator app, without help.
- **SC-002**: 100% of enrolments issue recovery codes, and a person who kept
  them can get back in with no operator involved.
- **SC-003**: A recovery code never works twice.
- **SC-004**: A confirmed second factor cannot be removed or weakened by
  anybody holding only the password — demonstrated by attempting it.
- **SC-005**: The same code cannot be used twice, demonstrated inside the
  window where it is still cryptographically valid.
- **SC-006**: With the instance-wide requirement on, an account that never
  enrolled can still get in — by enrolling — and no account is stranded.
- **SC-007**: An operator can tell, at a glance, how much of their instance has
  a second factor.
- **SC-009**: A fresh instance cannot be brought up with an administrator who
  has no second factor — demonstrated by trying to finish setup without one.
- **SC-010**: The first administrator of a fresh instance can enrol and save
  recovery codes with no mail server configured and no second person involved.
- **SC-011**: Enrolment is the same flow wherever it is entered from,
  demonstrated by completing it from all three entrances.
- **SC-008**: An account that has lost everything is recoverable by a
  documented path that leaves a record, and never by a database edit.

## Assumptions

- **Authenticator apps, not hardware keys or SMS.** TOTP is what is built and
  what the verifier supports; passkeys and hardware tokens are a different
  feature, and SMS is a weaker factor this does not propose adding.
- **The administrator rule is absolute; everything else is a choice.** An
  administrator always holds a second factor; everybody else is optional
  unless an operator turns on the instance-wide requirement or an
  administrator requires it of one account.
- **Setup collects mail and the administrator's second factor in the same
  pass**, and spec 040 owns the mail half. The order does not matter to this
  feature, and enrolment does not depend on the outcome of the mail half.
- **One authenticator per account** for a first pass. Multiple devices is a
  reasonable later addition and nothing here precludes it.
- **Ten recovery codes, single use**, which is the common shape and small
  enough to write down.
- **Recovery codes are stored the way passwords are** — verifiable, not
  readable — which is why FR-009 says the instance cannot show them again.
- **The existing verifier, window and issuer stay as they are.** Skew of one
  step is already reasoned about in `totp.rs`; this feature changes nothing
  about the cryptography.
- **Notification depends on the instance being able to send a message.**
  Spec 040 owns whether it can; where it cannot, the record still exists and
  the person is told in the product.
- **An account that only ever signs in through an external provider** is
  governed by that provider's own second factor, and an instance-wide
  requirement does not apply to it.

## Out of Scope

- Passkeys, WebAuthn, hardware security keys, SMS or email codes.
- Trusted devices, "remember this browser for 30 days", or any way to skip the
  challenge.
- Changing the algorithm, digits, period or skew.
- Multiple authenticators per account.
- Requiring a second factor to authorise individual sensitive actions, as
  distinct from signing in.
- Any change to how sessions themselves work — spec 036 owns that.
