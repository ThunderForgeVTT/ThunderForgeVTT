# Feature Specification: In-App Feedback, Delivered as GitHub Issues

**Feature Branch**: `037-in-app-feedback`

**Created**: 2026-09-07

**Status**: Draft — **specification only, not being built yet.** Written ahead
of implementation so the decisions exist before the code does. No task list,
no plan, nothing shipped. MVP-adjacent: it is how MVP learns what is wrong
with it, not a part of MVP's play loop.

**Input**: User description: "a feedback service that runs in GraphQL that allows users from any screen to see a little submit feedback button… three types: feature request, an issue, and general contact. The back end of these requests are to actually go into GitHub issues… a `FEEDBACK_GITHUB_APP_*` set up for how you actually submit feedback and this manages your issues. Additionally… an optional `GLOBAL_GITHUB_APP_*` and explain to the user that this applies to all the different subsystems that manage things like sync, features, etcetera… the front end component should collect browser logs. It should also offer the user, do they want to take a screenshot of the application, before submitting."

## Context

**There is no way to tell us anything from inside the product.** A person who
hits a bug closes the tab. What comes back instead is a message somewhere else,
days later, without the screen they were on, the world they were in, the
version they were running, or the error the console printed at the time — and
a report without those is a report that costs an afternoon to reproduce, if it
can be reproduced at all.

The playtest is coming, and it is the point of the current sequencing: green
suite, remaining specs, then people playing. **A playtest with no feedback path
is a playtest that produces anecdotes.** This feature is what turns "it broke"
into something a maintainer can act on the same day.

### Why GitHub issues, and not a table

Because the work already happens there. A feedback item that arrives as an
issue can be labelled, assigned, linked to a pull request, closed by a commit
and found again by search — none of which a bespoke admin queue would do
without rebuilding a tracker badly. The instance is not the system of record
for the *work*; it is the system of record for the *submission*, which is a
different thing and matters when GitHub is unreachable (FR-018).

### The precedent this follows

`src/server/src/repo_host.rs` already configures a GitHub App for lore
synchronisation, under `SYNC_GITHUB_APP_*`, itself modelled on spec 007's
`OAUTH_<PROVIDER>_*` shape. It accepts a client ID (not the numeric app id,
because GitHub recommends the client ID and an operator should land where the
value in front of them goes), a slug, and a private key in three declared
forms — file path, base64, or the PEM itself. It parses the key at
configuration time rather than at first use, so a broken key is a diagnostic
and not a mystery an hour later.

**This feature adds a second consumer of that shape, and one level above it.**
`FEEDBACK_GITHUB_APP_*` configures the feedback app; `GLOBAL_GITHUB_APP_*`
configures every subsystem that talks to GitHub at once. Today that is lore
sync and feedback. Tomorrow it is whatever comes next, which is exactly why
the global form is worth having and exactly why it must be explained rather
than merely offered.

### The hard part is privacy, and it is not a plan detail

A screenshot of the play field may contain another player's character, a GM's
private notes, or a name that is not the submitter's to publish. Browser logs
may contain a session identifier, an email address, a world id, or an asset
URL. **A GitHub issue may be readable by anyone on the internet.**

So the requirement is not "handle attachments carefully". It is: the person
sees exactly what is about to leave, secrets never leave at all, and the
destination's visibility is *stated from what it actually is* rather than
assumed. A feedback feature that leaks a session token has done more damage
than every bug it ever reported.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Say something from wherever you are (Priority: P1)

Someone using ThunderForge — mid-session, on the compendium, on a character
sheet — presses a Submit Feedback control that is present on the screen they
are already on. They choose one of three kinds: **a feature request**, **an
issue** (something is broken), or **a general message**. They type, they send,
they are told it arrived, and they are back where they were.

**Why this priority**: without this nothing else in the spec has a source. It
is also the whole feature from the submitter's point of view — everything
below is what happens to what they sent.

**Independent Test**: submit one of each kind from three different screens and
confirm each is accepted, acknowledged, and does not navigate the person away
from what they were doing.

**Acceptance Scenarios**:

1. **Given** any screen in the product, **When** the person looks for a way to
   send feedback, **Then** the control is present without leaving that screen.
2. **Given** the feedback form, **When** the person chooses a kind, **Then**
   the form asks for what that kind needs and does not demand what it does not
   — an issue asks what happened, a feature request asks what they want.
3. **Given** a submitted item, **When** it is accepted, **Then** the person is
   told it arrived and returns to exactly where they were, with anything they
   were doing intact.
4. **Given** a person part-way through writing feedback, **When** they dismiss
   the form and reopen it, **Then** their draft is still there.

---

### User Story 2 - A report that carries its own evidence (Priority: P1)

An issue report arrives with what the maintainer would otherwise have to ask
for: the browser logs from the session, the screen the person was on, the
world and system if any, the app version and browser — and, if the person
chooses, a screenshot of the application as they saw it.

**Why this priority**: it is the difference between a report and a
reproduction. Equal to US1 because feedback without evidence is the thing we
already have.

**Independent Test**: cause a console error, submit an issue, and confirm the
logs and context arrived with it and that the screenshot was included only
because the person chose it.

**Acceptance Scenarios**:

1. **Given** a session in which errors have occurred, **When** the person
   submits an issue, **Then** the logs the application captured are attached
   without them having to find or paste anything.
2. **Given** the feedback form, **When** the person is offered a screenshot,
   **Then** taking one is their choice, they see it before it is sent, and
   declining costs them nothing.
3. **Given** attachments are prepared, **When** the person reviews the
   submission, **Then** they can see **everything** that will be sent — logs
   included — before they send it, and can remove any part of it.
4. **Given** a screenshot the person has taken, **When** they see it contains
   something they would rather not send, **Then** they can drop it and still
   submit.

---

### User Story 3 - It becomes an issue somebody can work (Priority: P1)

An operator configures the feedback GitHub App. Submissions arrive as issues in
the repository they nominated, of the right kind, carrying their evidence, in a
form that can be labelled, assigned and closed like any other issue.

**Why this priority**: the destination is what makes feedback into work.
Without it, US1 and US2 fill a table nobody reads.

**Independent Test**: configure the app against a repository, submit one of
each kind, and confirm three issues arrive with their attachments intact.

**Acceptance Scenarios**:

1. **Given** a configured feedback app, **When** an item is submitted, **Then**
   an issue is created carrying the person's message, the kind, and the
   context and attachments that were approved.
2. **Given** an issue created from feedback, **When** a maintainer looks at it,
   **Then** the logs and any screenshot are part of the issue and can be moved
   around and worked with as ordinary issue content.
3. **Given** the three kinds, **When** issues are created, **Then** each is
   distinguishable as its kind without reading the body.
4. **Given** an operator who has configured nothing, **When** they open the
   configuration, **Then** they are told exactly which values are missing and
   what each one is, naming variables and never printing their values.

---

### User Story 4 - One app for every subsystem, said out loud (Priority: P2)

An operator who does not want to register a GitHub App per subsystem sets one
global app instead. The product uses it for lore sync, for feedback, and for
anything added later — and tells the operator that this is what they are
choosing, before they choose it.

**Why this priority**: it is an operator convenience, not a submitter-facing
capability, and US3 works without it. It is P2 rather than P3 because the
blast-radius explanation is a safety property, not a nicety.

**Independent Test**: set only the global variables and confirm both feedback
and lore sync work through them; then set a feedback-specific value and confirm
it wins for feedback while the global one still serves everything else.

**Acceptance Scenarios**:

1. **Given** only global credentials, **When** any GitHub-using subsystem
   needs an app, **Then** it uses the global one.
2. **Given** both global and feedback-specific credentials, **When** feedback
   needs an app, **Then** the feedback-specific one wins, and the global one
   still serves the subsystems that have no specific credentials.
3. **Given** an operator viewing the configuration, **When** global credentials
   are set or offered, **Then** they are told which subsystems this app will be
   used by and that its permissions apply to all of them.
4. **Given** a partially-specified feature app, **When** resolution happens,
   **Then** the operator is told plainly which values came from where, so a
   half-configured app is never silently completed from the global one without
   it being visible.

---

### User Story 5 - Knowing what happened to what you sent (Priority: P2)

A person who sent feedback can see what became of it — that it was received,
and whether it has since been closed — without an account on the tracker.

**Why this priority**: it is what makes people send a second one. Not P1
because the first submission is valuable even if nothing comes back.

**Independent Test**: submit an item, close the corresponding issue, and
confirm the submitter sees the change.

**Acceptance Scenarios**:

1. **Given** feedback a person has submitted, **When** they look at their own
   submissions, **Then** they see what they sent and its current state.
2. **Given** an issue that is closed by a maintainer, **When** the submitter
   looks again, **Then** they see it closed.
3. **Given** a submitter looking at their own submissions, **When** the list is
   shown, **Then** it contains only their own — feedback is not a public
   channel inside the product.

---

### User Story 6 - Feedback survives a broken destination (Priority: P2)

GitHub is down, the credentials are wrong, or the operator never configured
anything. The person still submits successfully, the submission is kept, and it
reaches the tracker when the destination works again.

**Why this priority**: the failure this prevents is the worst one available —
a person who took the trouble to report something and lost it. P2 only because
US1–US3 must exist for there to be anything to lose.

**Independent Test**: break the destination, submit, confirm the person is told
it was received rather than shown an error, then restore it and confirm the
item arrives.

**Acceptance Scenarios**:

1. **Given** an unreachable or unconfigured destination, **When** a person
   submits, **Then** the submission is accepted and kept, and the person is not
   told it failed.
2. **Given** kept submissions and a destination that starts working, **When**
   delivery is retried, **Then** each item arrives exactly once.
3. **Given** repeated delivery failures, **When** an operator looks, **Then**
   they can see what has not been delivered and why, without the reason
   disclosing any credential.

---

### Edge Cases

- Someone submits from a screen with no world at all — the login page, an
  error boundary, an admin screen. Context is thinner and the submission still
  works.
- A screenshot cannot be taken (the browser refuses, the canvas will not
  render). Offering fails; submitting does not.
- Logs are enormous — a long session with a chatty error. What is sent is
  bounded, and the person can see what was kept and what was dropped.
- A person submits the same thing five times because they are frustrated.
  Rate limiting refuses politely without losing what they wrote.
- The submission contains a credential the person typed themselves, in their
  own words. Redaction applies to what the app collected; what a person
  deliberately types is theirs, and the review step is where they see it.
- A screenshot of the play field contains another player's character or a GM's
  notes. The person sees it before it goes; the product does not decide for
  them, and it does not send it without them.
- The destination repository is private today and public tomorrow, or the app's
  installation is removed entirely.
- A submitter deletes their account after submitting. What is already on the
  tracker cannot be recalled by us, and the person is told this **before**
  they submit, not after.
- Feedback arrives from an instance that is not ours — a self-hosted operator
  pointing at their own repository. That is the normal case, not an exception.

## Requirements *(mandatory)*

### Functional Requirements

**Submitting**

- **FR-001**: A person MUST be able to open a feedback control from any screen
  in the product without navigating away from it.
- **FR-002**: The system MUST offer exactly three kinds — feature request,
  issue, and general message — and each submission MUST carry exactly one.
- **FR-003**: The form MUST ask for what its kind needs and MUST NOT require
  fields that do not apply to that kind.
- **FR-004**: A submission MUST be acknowledged to the person, and MUST return
  them to what they were doing with their work intact.
- **FR-005**: An unsent draft MUST survive the form being dismissed and
  reopened within the same session.
- **FR-006**: Submission MUST be rate limited per account, and a refusal MUST
  NOT discard what the person wrote.

**Evidence**

- **FR-007**: The client MUST capture browser logs during a session so that a
  submitted issue carries them without the person collecting anything.
- **FR-008**: The system MUST offer a screenshot of the application before
  submitting; taking one MUST be the person's choice and declining MUST NOT
  block submission.
- **FR-009**: A submission MUST carry the context the app already knows: the
  screen, the world and game system where applicable, the application version,
  and the browser.
- **FR-010**: The person MUST be shown everything that will be sent — message,
  context, logs and screenshot — before it is sent, and MUST be able to remove
  any part of it and still submit.
- **FR-011**: What is captured MUST be bounded in size, and when anything is
  dropped for size the person MUST be told what was kept.

**Privacy**

- **FR-012**: Session identifiers, authentication tokens, cookies and
  credentials MUST NOT leave the browser in any attachment, whatever the
  person approves — redaction happens **before** the review step, so what the
  person sees is what is sent.
- **FR-013**: The submitter's email address MUST NOT be sent to the
  destination. A submission MUST be traceable back to its submitter *inside
  the instance* without publishing who they are outside it.
- **FR-014**: The person MUST be told, before they submit, how visible the
  destination is — determined from what the destination actually is, never
  assumed — and that what is sent cannot be recalled by the instance
  afterwards.
- **FR-015**: A screenshot MUST be produced from what the person can see, and
  MUST be shown to them at the size they can inspect before it is attached.
- **FR-016**: Attachment retention inside the instance MUST be stated and
  bounded, and MUST be independent of what the destination does with its copy.

**Delivery**

- **FR-017**: An accepted submission MUST become an issue in the configured
  repository, carrying the message, the kind, the approved context and the
  approved attachments.
- **FR-018**: A submission MUST be recorded by the instance **before** any
  delivery is attempted, so that an unreachable or unconfigured destination
  never loses it (US6).
- **FR-019**: Delivery MUST be retried until it succeeds or is abandoned by an
  operator, and MUST NOT produce duplicates when it succeeds after a retry.
- **FR-020**: Each kind MUST be distinguishable at the destination without
  reading the body.
- **FR-021**: Undelivered submissions and the reason for each MUST be visible
  to an operator, and no reason may disclose a credential or any fragment of
  one.
- **FR-022**: A submitter MUST be able to see their own submissions and their
  current state, and MUST NOT be able to see anybody else's.

**Configuration**

> **Spec 040 (Instance Setup and Configuration) now owns credential collection,
> editing and resolution**, including the global-versus-specific rule and the
> blast-radius explanation. The requirements below remain the statement of what
> this feature needs; 040 is where an operator actually sets it. They must
> agree.
>
> The contract is
> `specs/040-instance-setup/contracts/github-applications.md`, decided in that
> feature's `research.md` § R10 and recorded as ADR-090. In the code it is
> `src/server/src/github_apps.rs`; `repo_host::scoped` re-exports it, and
> `settings::registry` declares the nine keys — three scopes by three fields —
> that are the only place these variable names are written down. So
> `FEEDBACK_GITHUB_APP_*` is **configured there, not invented here**: there is
> one credential vocabulary for the product.
>
> Two things that contract decides, which the requirements below do not:
>
> - **Scope is the outer axis and source is the inner one.** FR-024 below and
>   spec 040's FR-010 point opposite ways when a global application is set in
>   the environment and a subsystem application in the administration screens.
>   The subsystem's wins. Somebody who configured a subsystem application
>   meant it.
> - **An application resolves whole.** See the amendment to FR-029.

- **FR-023**: The feedback destination MUST be configurable by environment as
  `FEEDBACK_GITHUB_APP_*`, following the existing `SYNC_GITHUB_APP_*` shape:
  client ID, slug, and the private key in the same three declared forms.
- **FR-024**: A global `GLOBAL_GITHUB_APP_*` MUST be supported, and when set
  MUST apply to every subsystem that needs a GitHub App.
- **FR-025**: Where both are set, the subsystem-specific value MUST win for
  that subsystem, and the global value MUST continue to serve subsystems with
  no specific value.
- **FR-026**: The operator MUST be told, wherever global credentials are set or
  offered, **which subsystems** they will be used by — an explanation of scope,
  not just a field.
- **FR-027**: Configuration diagnostics MUST name the variable that is wrong
  and what it is for, and MUST NEVER print a value, a fragment of one, or its
  length.
- **FR-028**: Credentials MUST be validated when they are configured rather
  than at first use, so a broken key is a diagnostic and not a failure during
  someone's submission.
- **FR-029**: When resolution draws on both global and specific values, the
  operator MUST be able to see which value came from where.
  > **Amended by spec 040 (FR-021, US5 scenario 4, ADR-090):** resolution never
  > draws on both *for one application*. An application resolves whole — a
  > subsystem application with a client ID and no private key does not borrow
  > the global one's key; it is reported incomplete, naming what is missing,
  > and the subsystem falls through to the global application entire. A client
  > ID from one registration signed by a key from another is not an
  > application, it is an authentication failure that reads like a bad key.
  > What the operator is shown is therefore which *application*, from which
  > scope, with each field's source and the variable that fixed it — which is
  > this requirement at the granularity that exists. Spec 040's US5.4 and the
  > field-merging reading of this sentence cannot both hold; the acceptance
  > scenario won.
- **FR-030**: With no destination configured at all, the feedback control MUST
  still work and submissions MUST still be kept (FR-018) — an unconfigured
  instance collects feedback, it just cannot forward it yet.

### Key Entities

- **Feedback submission**: what a person sent — kind, message, context,
  approved attachments, who sent it (inside the instance), when, and its
  delivery state.
- **Attachment**: a log bundle or a screenshot belonging to one submission,
  with a stated retention.
- **Destination**: the repository a submission becomes an issue in, and its
  visibility.
- **GitHub App credentials**: a resolved set — client ID, slug, private key —
  belonging either to one subsystem or to the instance as a whole, with a
  recorded resolution source.
- **Delivery attempt**: one try at creating an issue, its outcome, and the
  reason it failed if it did.

## Success Criteria *(mandatory)*

- **SC-001**: A person can send feedback from any screen in under 30 seconds,
  without losing what they were doing.
- **SC-002**: An issue report submitted from a session in which an error
  occurred arrives with those logs attached, with no action by the submitter
  beyond describing the problem.
- **SC-003**: Every attachment that reaches the destination was visible to the
  person before it was sent, demonstrated by a case where they remove one and
  it does not arrive.
- **SC-004**: No submission, in any configuration, carries a session
  identifier, token, cookie or the submitter's email — demonstrated by
  deliberately putting one in a log and watching it not arrive.
- **SC-005**: With the destination unreachable, 100% of submissions are still
  accepted and kept, and all of them arrive exactly once when it recovers.
- **SC-006**: An operator can configure the feedback destination from
  environment variables alone, and a misconfiguration is reported by name
  before anyone submits.
- **SC-007**: An operator setting one global app sees, in the same place they
  set it, the list of subsystems it will serve.
- **SC-008**: A submitter can see the state of everything they have sent, and
  nothing anybody else has sent.
- **SC-009**: A maintainer can act on a report without asking the submitter a
  follow-up question, in the majority of issue-kind submissions.

## Assumptions

Defaults taken where the description did not settle something. Each is a
candidate for `/speckit-clarify`.

- **Signed-in people submit.** Anonymous submission from public pages is out of
  scope for a first pass: it is an abuse surface with its own design, and the
  people whose feedback matters most for a playtest are signed in.
- **The destination is one repository per instance**, nominated by the
  operator, not one per world or per system pack.
- **The submitter is identified to the destination by an opaque reference**,
  not by email; the instance can map it back, the tracker cannot.
- **The destination may be public.** The product determines the actual
  visibility and says so rather than assuming either way.
- **Screenshots and logs are kept by the instance for a bounded period**
  independent of the tracker's copy.
- **Delivery is retried with backoff and is idempotent**; an operator can
  abandon an item that will never succeed.
- **"Manages your issues" means reading state back and, where a maintainer
  acts, reflecting it** — not editing issues on the submitter's behalf.
- **Existing `SYNC_GITHUB_APP_*` variables keep working unchanged.** The global
  form fills gaps; it does not replace or deprecate what an operator already
  set.

## Out of Scope

- Anonymous or unauthenticated submission.
- Trackers other than GitHub. The shape should not preclude one, and nothing
  here builds one.
- A conversation thread between submitter and maintainer inside the product;
  US5 reports state, it is not a support inbox.
- Automatic triage, deduplication or clustering of submissions.
- Session replay, video capture, or continuous telemetry. This feature sends
  something when a person decides to send it, and at no other time.
- Editing or closing issues from inside ThunderForge.
