# Feature Specification: Concurrent Client Sessions — One Account Across Tabs, Windows and Browsers, and the E2E Structure That Proves It

**Feature Branch**: `036-concurrent-client-sessions`

**Created**: 2026-09-07

**Status**: Draft

**Input**: User description: "i want to expand the e2e structures to the missing areas, and support multi tab and multi window from the same machine — that's the big thing. If a new window they should be able to sign in and it pick up the same hooks. That's also a key for us for multi browser testing, ensuring the same experience across without needing many accounts."

## Context

**Signing in evicts whoever was already signed in.** One line decides it, and
it says why it is there:

> `src/server/src/auth/sessions.rs:370` — "Revoke existing active sessions on
> new login to reduce session replay risk."

Every successful login updates every un-revoked row in `user_sessions` for
that user before inserting its own. The table itself has no such constraint —
`user_sessions` is keyed by `id` with a plain `user_id` column, so many live
sessions per account are already representable. The policy is in the code, not
the schema, and it was chosen deliberately. **This feature changes that
decision, so it needs to replace the protection rather than delete it.**

### What actually breaks, and what does not

The distinction matters because "multi-tab" and "multi-window" are not one
problem:

- **A second tab or window in the same browser profile already works.** It
  shares the session cookie, so it is the same session, and it needs nothing
  from this feature — unless the person signs in again in it, at which point
  they evict themselves.
- **A second browser, a second profile, an incognito window, or a second
  machine does not.** Each has its own cookie jar, so each must sign in, and
  each sign-in kills the last one. This is the case that bit the demo, and the
  case the e2e harness hits every time it opens a second browser context.

So the product statement is not "support tabs". It is: **an account may hold
several live sessions at once — and exactly one of them is at the table.**

That second half is a deliberate narrowing, taken in clarification: two play
fields for one person is where the session and sync problems come from, and
nobody asked for it. What people do ask for is the sheet on the other screen.
So the play field is exclusive, everything else is a companion surface, and
the feature is worth having because of what the companion can do — show your
character, keep up with it live, and roll the system's own checks into the
table's rolls.

### Why the test harness is in the same spec

`apps/web/e2e/fixtures/helpers.ts` works around the eviction by registering a
*fresh account* for each browser context — `inviteAndJoinAsPlayer` registers a
second user rather than opening a second window as the first. That is the only
thing it could do, and it has a cost: every cross-client behaviour is proven
between two *different people*, and the far more common real situation — one
person, two windows — is proven nowhere. A GM with the map on one screen and
the compendium on another is not two accounts.

Lifting the eviction is what makes a multi-client fixture possible; the
fixture is what makes the lifted eviction provable. Splitting them would ship
one without the other.

### The coverage this unlocks

Two known-empty areas are named here because the multi-client fixture is what
they have been waiting for, and because both are recorded in `TOMORROW.md` as
the next work:

- **Combat.** `graphql/mutations_combat.rs` implements the whole turn
  structure behind 14 Rust tests, and `CombatPanel.tsx` already carries
  `combat-panel`, `start-combat-button`, `combat-round-counter`,
  `advance-turn-button`, `end-combat-button` and `combatant-list`. Nothing in
  `apps/web/e2e/` touches any of them. Turn order is the part of the product
  every system shares, and it is proven only below the UI.
- **OAuth.** Auto-provisioning on first login (ADR-042), password confirmation
  before linking an identity to an existing account (ADR-006), the
  "provider returned no verified email" refusal, and spec 035's deferred T056
  — a closed instance turning away a stranger mid-handshake — have **no**
  end-to-end coverage at all, because the harness has no provider to stub.

### A harness defect found on the way, fixed separately

The e2e harness migrates an empty template database and seeds accounts
afterwards. Spec 035's migration branches on whether any user exists, so every
shard came up `invite_only` and the registration that nearly every spec begins
with was refused suite-wide. The fix is a line in `demo_accounts.sql` and is
not part of this feature; this spec assumes a harness whose instance admits
new accounts.

## Clarifications

### Session 2026-09-07

- Q: Should every client of an account be a full play-field client? → A: No.
  **An account may hold many sessions, but only one of them may hold the play
  field at a time.** The others are companion surfaces. Deliberately narrower
  than "several equal clients": the play field is the surface with a live
  engine, a canvas and an event stream, and admitting two of them per person
  is where the session and sync problems would come from.
- Q: What is a companion window for, if not playing? → A: **The character
  sheet on a second screen, beside the play field on the first.** That is the
  case this makes possible and the reason the restriction is not a loss: one
  window for the table, one for your character.
- Q: Can a player roll a check from the character-sheet window? → A: Yes, and
  **what gets rolled is determined by the game system, not by the sheet.** A
  5e player pressing Strength or a skill gets that system's check, resolved
  the way every other roll in the product is resolved, and it lands in the
  game's rolls where the table can see it. The sheet names the check; it never
  decides the dice or the outcome.
- Q: If the play-field client can continue without the server (ADR-052), can a
  companion window do the same? → A: **No, and this is a hard line.** Peer-to-
  peer continuation belongs to the play field and nowhere else. A companion
  window that has lost the server — even while the player can still reach the
  GM — MUST refuse the roll and say to retry it from the play field, rather
  than route it over a peer connection that was never scoped to carry it.
- Q: When a second window opens the play field, what happens to the first? →
  A: The newest claim wins and the previous play-field client is demoted to a
  companion surface with a plain explanation and a way to take the play field
  back. Refusing the new window instead would reproduce the demo defect one
  level up — "you are already playing somewhere" is the same dead end as
  "you have been signed out".

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A second window signs in without evicting the first (Priority: P1)

A Game Master running a session on one screen opens ThunderForge in a second
browser — a different browser, a second profile, or a private window — and
signs in with the same account. Both windows stay signed in. Both show the
world. One of them holds the play field; the other is a companion surface, and
neither is signed out to make room for the other.

**Why this priority**: This is the reported defect and the whole point of the
feature. Nothing else in this spec is reachable while a sign-in destroys the
session that came before it.

**Independent Test**: Sign in as one account in two independent browser
contexts, act in each, and confirm both remain authenticated and both observe
each other's changes. Delivers the capability on its own with no test-harness
or coverage work attached.

**Acceptance Scenarios**:

1. **Given** an account signed in and viewing a world, **When** the same
   account signs in from a second browser context, **Then** the first context
   remains signed in and continues to receive live updates without a reload.
2. **Given** two contexts signed in as one account, one holding the play
   field, **When** a token moves, **Then** the companion context reflects the
   world change it is showing without a reload.
3. **Given** two contexts signed in as one account, **When** one performs an
   action its role permits, **Then** the other is not required to re-
   authenticate, and no action is refused because a sibling client exists.
4. **Given** two contexts signed in as one account, **When** the second signs
   in while the first has unsaved per-client state (a selection, an open
   panel), **Then** the first keeps that state.

---

### User Story 2 - One account, several clients, in the test suite (Priority: P1)

A test author opens two or more clients as the *same* account and asserts what
each sees, without inventing a second account to stand in for a second window.

**Why this priority**: Equal to US1 because it is how US1 stops being a claim.
Every cross-client assertion in the suite today is between two different
people, so "one person, two windows" — the situation this feature exists for —
has no test that can express it.

**Independent Test**: A single spec opens two contexts as one account and
asserts an action in one is visible in the other. Once that fixture exists,
any later spec can use it.

**Acceptance Scenarios**:

1. **Given** the shared fixtures, **When** a test asks for a second client for
   an existing account, **Then** it gets a signed-in client without a second
   registration and without the first client being disturbed.
2. **Given** a test using two clients of one account, **When** it runs beside
   the rest of the suite under the sharded harness, **Then** it passes without
   requiring the suite to be serialised.
3. **Given** an existing spec that registers a second account only to obtain a
   second window, **When** it is rewritten on the fixture, **Then** it asserts
   the same behaviour with one account.

---

### User Story 3 - Shared truth, private view (Priority: P2)

A person with two windows open expects some things to be identical in both and
others to be theirs alone. Membership, permissions, the character they have
claimed, which scene the world is on, and world content are the same in every
window. Which token is selected, where the status panel sits, and the camera
are each window's own.

**Why this priority**: Without this stated, "the same experience across
clients" is untestable — and the wrong reading (mirroring selection and camera
into every window) would make two windows useless for the reason people open
two windows.

**Independent Test**: With two clients of one account, change a shared thing
in one and a private thing in the other, and assert exactly one of them
crosses.

**Acceptance Scenarios**:

1. **Given** two clients of one account, **When** the GM changes which scene
   the world is on in one, **Then** the other follows, because that is world
   state and not a per-client choice.
2. **Given** two clients of one account, **When** one selects a token or moves
   its camera, **Then** the other's selection and camera are unchanged.
3. **Given** two clients of one account, **When** the account's role or an
   object permission changes, **Then** both clients enforce the new
   permission without a reload.
4. **Given** two clients of one account in one world, **When** presence is
   displayed, **Then** the person appears once, not once per client.

---

### User Story 3a - The sheet on the second screen (Priority: P1)

A player keeps the play field on one screen and their character sheet on
another. The sheet window is signed in as the same account, shows the same
character, and updates as the character changes — but it does not draw the
canvas, does not compete for the play field, and does not need to.

**Why this priority**: This is what the play-field restriction buys, and the
restriction is not worth having without it. It is also the concrete thing the
person asked for.

**Independent Test**: Open the play field in one window and the character
sheet in another as one account, change a stat, and assert both agree while
only one holds the canvas.

**Acceptance Scenarios**:

1. **Given** a play-field client open, **When** the same account opens a
   character sheet in a second window, **Then** the sheet loads without the
   play field being disturbed.
2. **Given** a sheet window and a play-field window, **When** a value on the
   character changes from either side, **Then** both show the new value
   without a reload.
3. **Given** a sheet window open, **When** the person asks it for the play
   field, **Then** it takes the play field and the previous play-field client
   is told plainly that it has become a companion.
4. **Given** a companion window, **When** it is open, **Then** it does not
   run the canvas or hold a play-field claim.

---

### User Story 3b - A check rolled from the sheet is the system's check (Priority: P2)

A 5e player presses Strength on their character sheet and gets that system's
check — the dice the system declares, against the character's own values —
resolved the way every roll in the product is resolved and shown in the
game's rolls where the table can see it. A player of a system with no ability
scores at all sees whatever that system declares instead, and nothing about
5e leaks into it.

**Why this priority**: It is the reason a sheet window is worth opening rather
than a static reference. P2 because US1 and US3a must exist first for there to
be a sheet window at all.

**Independent Test**: Roll a named check from the sheet window in two
different systems and assert each produced its own system's roll, visible to
the table, with the sheet deciding neither the dice nor the outcome.

**Acceptance Scenarios**:

1. **Given** a character sheet in a system that declares ability checks,
   **When** the player rolls one from the sheet, **Then** the roll appears in
   the game's rolls, attributed to that character, visible to the table on the
   same terms as a roll made from the play field.
2. **Given** two worlds on two different systems, **When** the same gesture is
   made on each sheet, **Then** each produces its own system's check, and the
   sheet contains no rule belonging to a particular system.
3. **Given** a system that declares no such check, **When** the sheet is
   opened, **Then** it offers none, rather than offering one that cannot mean
   anything.

---

### User Story 3c - Offline, the sheet sends you back to the table (Priority: P2)

A player loses the server but can still reach their GM. The play field keeps
going, as it already may. The sheet window cannot, and says so: it refuses the
roll and tells the player to make it from the play field instead.

**Why this priority**: This is the hard line stated as behaviour. Left
unstated, the natural next step for an implementer is to route the sheet's
roll over the peer connection that is already there — which would extend
peer adjudication to a surface it was never scoped for.

**Independent Test**: Disconnect a client from the server while its peer
connection survives, roll from the sheet, and assert the refusal names the
play field as the place to retry.

**Acceptance Scenarios**:

1. **Given** a sheet window that has lost the server, **When** the player
   attempts a roll, **Then** it is refused with an explanation naming the play
   field as where to retry, and no roll is recorded anywhere.
2. **Given** a sheet window that has lost the server, **When** it is offline,
   **Then** it opens no peer connection of its own and sends nothing over one
   that exists.
3. **Given** the same disconnection, **When** the play field continues under
   ADR-052's existing scope, **Then** that continuation is unchanged by this
   feature.

---

### User Story 4 - Ending a session, deliberately (Priority: P2)

Someone who signs out of one window expects the others to keep working, and
someone who has lost a device expects a way to end every session at once.
Because logging in no longer clears old sessions, ending them has to become
something a person can actually do.

**Why this priority**: This is the security half of removing the eviction. It
is P2 rather than P1 only because US1 is what unblocks the demo — but this
must ship with the feature, not after it.

**Independent Test**: With several clients signed in, sign out of one and
assert the others survive; then end all sessions and assert every client is
signed out on its next request.

**Acceptance Scenarios**:

1. **Given** an account with three live sessions, **When** it signs out of
   one, **Then** the other two remain usable and the signed-out one is
   rejected on its next request.
2. **Given** an account with several live sessions, **When** the person ends
   all sessions, **Then** every client including the one that asked is signed
   out on its next request.
3. **Given** an account with several live sessions, **When** its password
   changes, **Then** every session other than the one that made the change is
   ended.
4. **Given** a person looking at their own account, **When** they ask what is
   signed in, **Then** they are shown each live session with enough to
   recognise it and to end it individually.

---

### User Story 5 - Combat, driven through the UI (Priority: P3)

A GM starts combat, adds combatants, advances the turn and ends it, while a
player watches from their own client and sees the round and the active
combatant change.

**Why this priority**: The behaviour already exists and is unit-tested; this
is coverage, not capability. It is placed here because it is the first thing
the multi-client fixture makes cheap to write, and because turn order is
shared by every system.

**Independent Test**: One spec driving `CombatPanel.tsx`'s existing test ids
from a GM client while asserting a second client follows.

**Acceptance Scenarios**:

1. **Given** a world with tokens and a GM client, **When** the GM starts
   combat, **Then** the panel shows a round and a combatant list.
2. **Given** combat in progress and a second client watching, **When** the GM
   advances the turn, **Then** the watching client shows the new active
   combatant and round without a reload.
3. **Given** combat in progress, **When** the GM ends it, **Then** both
   clients return to the non-combat view.

---

### User Story 6 - The OAuth surface, observed (Priority: P3)

A person signs in with an external provider for the first time and gets an
account; a person whose email already has an account is asked to confirm their
password before the identity is linked; a provider that returns no verified
email is refused; and a closed instance turns away a stranger mid-handshake.

**Why this priority**: Three ADRs describe behaviour nothing exercises in a
browser, and one deferred task (spec 035 T056) is waiting on exactly this. It
is last because it needs a stubbed provider that does not exist yet, which is
the largest piece of work in the spec that is not the feature itself.

**Independent Test**: A test-only provider the harness can point at, seeded as
a provider row, driven through the real redirect flow.

**Acceptance Scenarios**:

1. **Given** an instance that admits new accounts and an identity unknown to
   it, **When** the person completes an external sign-in, **Then** an account
   is provisioned and they arrive signed in.
2. **Given** an existing account holding that verified email, **When** the
   same identity signs in externally, **Then** the person is asked to confirm
   their password before the identity is linked.
3. **Given** a provider that returns no verified email, **When** the handshake
   completes, **Then** the person is refused and no account is created.
4. **Given** a closed instance and an identity unknown to it, **When** the
   handshake completes, **Then** the person is refused, no account is created
   and no session is issued.

---

### Edge Cases

- One client's session expires while a sibling stays active — the expired one
  must be told to sign in again and must not take its sibling down with it.
- A person is removed from a world, or their role is reduced, while two of
  their clients have it open. Both must lose access, not just the one that
  happened to act.
- A claimed character: two clients of the owner both act on the same actor at
  once. The rules that already adjudicate two people acting apply unchanged —
  one account is not a licence to skip adjudication.
- Two clients of one account each redeem the same single-use invitation
  simultaneously. Exactly one may succeed, as it already does for two people.
- Two-factor authentication: a second sign-in must still challenge, and
  passing it must not end the sibling session.
- Presence and heartbeats from several clients of one person must not make
  them appear repeatedly, flap between online and offline, or hold a world
  open after their last client is gone.
- A client left asleep for hours reconnects and must reconcile rather than
  apply a stale view over current state.
- An unbounded number of sessions per account is a resource question as well
  as a security one; the number is bounded and the bound is stated.
- Two windows race to claim the play field at the same instant. Exactly one
  holds it afterwards, and the other knows it does not.
- The play-field client is closed, crashes, or its machine sleeps without
  releasing the claim. The claim must not be held by a client that is gone —
  a companion window must be able to take the table back without waiting out
  a timeout nobody can see.
- A companion window is open on a world the account has since been removed
  from, or on a character it no longer owns. It loses access on the same terms
  as the play field.
- A roll is made from the sheet at the same moment the play field makes one.
  Both are the table's rolls; neither is dropped or merged into the other.
- The play field goes offline under ADR-052 while a companion window is still
  online. The companion is not a second route into the table for anything the
  play field is holding in its outbox.

## Requirements *(mandatory)*

### Functional Requirements

**Concurrent sessions**

- **FR-001**: An account MUST be able to hold more than one live session at
  the same time; a successful sign-in MUST NOT end any session that already
  exists.
- **FR-002**: Each live session MUST be independently valid — usable for every
  action the account's roles and permissions allow, with no notion of a
  primary or secondary client.
- **FR-003**: Live sessions MUST be individually revocable, and revoking one
  MUST leave the others untouched.
- **FR-004**: The number of concurrent live sessions per account MUST be
  bounded, and reaching the bound MUST end the least recently used session
  rather than refuse the new sign-in.
- **FR-005**: Each session MUST be recognisable by its owner — at minimum when
  it was created, when it was last used, and where from — so that "end the one
  I don't recognise" is possible.

**Ending sessions**

- **FR-006**: Signing out MUST end only the session that asked.
- **FR-007**: A person MUST be able to end all of their sessions at once,
  including the one making the request.
- **FR-008**: A password change MUST end every session for that account except
  the one that made the change.
- **FR-009**: An ended session MUST be refused on its next request, and its
  client MUST be told to sign in again rather than left showing state it can
  no longer refresh.
- **FR-010**: Ending a session MUST close the live update streams that session
  holds, so a revoked client stops receiving world events.

**What is shared and what is not**

- **FR-011**: World state MUST be identical across every client of every
  account that can see it — including which scene the world is on, world
  content, membership, roles and per-object permissions.
- **FR-012**: A change of role or permission MUST take effect in every live
  client of the affected account without requiring a reload.
- **FR-013**: Per-client view state — selection, camera, and viewer-chosen
  panel placement — MUST remain private to the client that set it.
- **FR-014**: A person with several clients open MUST appear once in presence,
  and MUST be shown as present while at least one client is live.
- **FR-015**: Every live client of an account MUST receive world events for
  the world it has open, on the same terms as any other client.

**The test structure**

- **FR-016**: The e2e fixtures MUST provide a way to obtain an additional
  signed-in client for an account that is already signed in, without
  registering another account.
- **FR-017**: That fixture MUST work for separate browser contexts and for
  additional tabs or windows within one context, and MUST make clear which of
  the two a test is asking for.
- **FR-018**: Specs using several clients of one account MUST run under the
  existing sharded harness without requiring the suite to be serialised.
- **FR-019**: Existing specs that register a second account solely to obtain a
  second window MUST be moved onto the fixture, and the ones that genuinely
  need two different people MUST stay as they are.
- **FR-020**: The suite MUST include a case that fails if the eviction
  behaviour returns.

**Coverage the structure unlocks**

- **FR-021**: The combat panel MUST be driven end to end through its existing
  interface — starting combat, adding and removing combatants, advancing the
  turn, and ending combat — with a second client asserting what it observes.
- **FR-022**: The external sign-in surface MUST be exercised end to end
  against a test-only provider: first-time provisioning, linking to an
  existing account behind a password confirmation, refusal when no verified
  email is returned, and refusal by a closed instance.
- **FR-023**: The test-only provider MUST NOT require any branch, flag or
  special case in the product's own sign-in path; a test that exercises a
  test-only branch proves the branch and not the flow.
- **FR-024**: Once FR-022 holds, spec 035's deferred manual pass (T056) MUST
  be closed and struck from the deferred-manual-pass register.

**Not weakening what the eviction protected**

- **FR-025**: Removing login-time eviction MUST NOT extend how long a stolen
  session remains usable: session lifetime, expiry and the conditions that end
  a session other than a new login MUST be unchanged or tightened.
- **FR-026**: The creation of a session MUST be recorded, and a person MUST be
  able to see that record, so that an unrecognised sign-in is discoverable by
  its owner rather than silently replacing the evidence.

**One play field per account**

- **FR-027**: An account MUST hold at most one play-field client at a time;
  every other live client of that account is a companion surface.
- **FR-028**: A client MUST be able to claim the play field from another
  client of the same account, and the claim MUST be decided so that exactly
  one client holds it however many ask at once.
- **FR-029**: A client that loses the play field MUST be told plainly that it
  has become a companion, and MUST offer to take it back.
- **FR-030**: A play-field claim MUST NOT survive the client that made it: a
  closed, crashed or unreachable client MUST NOT keep another window of the
  same account out of the table.
- **FR-031**: A companion surface MUST NOT run the canvas or hold a play-field
  claim, and MUST NOT be required to in order to show a character.

**Companion surfaces**

- **FR-032**: A person MUST be able to open a character sheet as a companion
  surface in its own window, signed in as the same account, without disturbing
  the play field.
- **FR-033**: A change to a character MUST be reflected in every surface
  showing it — sheet and play field alike — without a reload.
- **FR-034**: A companion surface MUST enforce the same permissions as the
  play field, and MUST lose access at the same moment when membership, role
  or object permission changes.

**Rolling from a companion surface**

- **FR-035**: A character sheet MUST be able to initiate a check the world's
  game system declares, and MUST NOT define what that check is: the dice, the
  values used and the outcome are the system's and the server's, never the
  sheet's.
- **FR-036**: A check initiated from a sheet MUST be indistinguishable, once
  resolved, from the same check initiated at the play field — same
  adjudication, same visibility to the table, same record.
- **FR-037**: A sheet MUST offer no check for a system that declares none, and
  MUST contain no rule belonging to any particular system.

**The peer boundary**

- **FR-038**: Peer-to-peer continuation MUST remain a play-field capability
  only. No companion surface may open a peer connection or send anything over
  one.
- **FR-039**: A companion surface that has lost the server MUST refuse actions
  requiring adjudication, MUST say that it has lost the server, and MUST name
  the play field as where to retry.
- **FR-040**: A refusal under FR-039 MUST leave no record of a roll anywhere —
  not locally, not queued for replay, and not at the table.
- **FR-041**: This feature MUST NOT widen what a disconnected play field may
  decide; ADR-052's existing scope is unchanged.

### Key Entities

- **Account**: the person's identity on the instance. Holds many sessions.
- **Session**: one signed-in client's authority to act, with its own lifetime,
  its own end, and enough description for its owner to recognise it. Already
  represented per-row today; the change is that several may be live at once.
- **Client**: one tab, window or browser. Holds at most one session, plus view
  state that belongs to it alone.
- **Live subscription**: the stream of world events a client holds while it
  has a world open. Bound to a session, and ends when that session does.
- **Play-field claim**: the one client of an account currently at the table.
  Exactly one per account, held by a live client and released when that client
  is gone.
- **Companion surface**: a signed-in client that is not the play field — the
  character sheet is the first of them. Sees and acts through the server, and
  never through a peer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: One account can be signed in on at least four clients at once,
  and every one of them stays usable for a full session's length without
  re-authenticating.
- **SC-002**: An action taken in one client is visible in every other client
  of the same account within the same time it takes to reach a different
  account's client.
- **SC-003**: Signing out of one client leaves every other client of that
  account working; ending all sessions signs out every client on its next
  request.
- **SC-004**: A revoked or expired session is refused on its next request and
  receives no further world events.
- **SC-005**: A test author can obtain a second client for an existing account
  in a single fixture call, and a spec that does so passes under the sharded
  harness with no change to how the suite is run.
- **SC-006**: The number of accounts the suite registers falls, and no
  behaviour that used to be covered is lost — every spec rewritten onto the
  fixture asserts what it asserted before.
- **SC-007**: The combat turn structure and the four external sign-in
  outcomes each have at least one end-to-end case that fails when the
  behaviour is broken, demonstrated by breaking it once on purpose.
- **SC-008**: A demonstration can be run from two windows of one account, on
  one machine, with no account juggling and no client dropping out.
- **SC-009**: However many windows of one account ask for the play field at
  once, exactly one holds it, and the others say so rather than appearing to
  work.
- **SC-010**: A person can play with the table on one screen and their
  character sheet on another for a whole session without either window
  needing a reload.
- **SC-011**: A check rolled from the sheet reaches the table's rolls, and is
  indistinguishable from the same check rolled at the play field.
- **SC-012**: With the server unreachable and a peer reachable, a roll
  attempted from the sheet is refused, names the play field, and leaves no
  record — demonstrated by breaking the connection on purpose.

## Assumptions

Recorded because the description did not settle them and a reasonable default
exists. Each is a candidate for `/speckit-clarify` before planning.

- **Signing out ends one session, not all.** The alternative — sign-out
  meaning "everywhere" — is more surprising than the eviction being removed,
  and "end all sessions" is provided explicitly instead (FR-007).
- **A password change ends other sessions.** This is the standard expectation
  after a credential change and it partly replaces what login-time eviction
  was doing.
- **A bound exists on concurrent sessions, and reaching it evicts the oldest
  rather than refusing the newest.** Refusing the new sign-in would reproduce
  the reported defect at a higher number.
- **Presence is per person, not per client.** Two windows are one player at
  the table, and the play-field claim does not change that.
- **The character sheet is the first companion surface, not the only possible
  one.** Nothing here forbids another later; everything here applies to it
  when it arrives.
- **"Multi-browser" means several browsers on one machine, all Chromium-based.**
  Cross-engine support (Firefox, Safari) is unchanged by this feature and
  remains untested by construction — the suite runs Chromium alone.
- **The test-only external provider is a test fixture, not a product feature.**
  It ships in the harness and seed data, never in a release.
- **The harness seeds an instance that admits new accounts.** The
  `invite_only` template defect is fixed separately.

## Out of Scope

- Cross-machine and cross-network sessions beyond what already works: this
  feature is about one account's several clients, not about federation or
  device handoff.
- Firefox or Safari support.
- Mirroring per-client view state between clients (a "follow my other window"
  mode). FR-013 deliberately keeps them private.
- Two play fields for one account, on one machine or several. FR-027 forbids
  it, and this feature does not treat it as a later phase.
- A companion surface that draws the canvas. If it draws the canvas it is a
  play field, and FR-027 applies to it.
- Widening what a disconnected client may decide. ADR-052's scope — token
  position, rotation and scale, replayed on reconnection — is untouched here,
  and FR-038 through FR-041 exist to keep it that way.
- Real external identity providers in the suite. FR-022 is satisfied by a
  stub; pointing the suite at a live provider is a different kind of test.
