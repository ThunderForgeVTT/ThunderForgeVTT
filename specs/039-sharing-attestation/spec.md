# Feature Specification: The Sharing Attestation — Recorded, Enforced, and on Every Path That Publishes

**Feature Branch**: `039-sharing-attestation`

**Created**: 2026-09-07

**Status**: Draft — **specification only, not being built yet.**

**Not legal advice.** `legal/README.md` marks the instance's legal prose as
needing review before launch, and nothing here changes that. This spec
describes what the *product* must do; whether the words are the right words is
a lawyer's judgement, not this document's.

**Input**: Project owner, 2026-09-07: "the accountable owner should be the one who uploads and shares the media. We act as a middleman when it comes to DMCA… if a user uploads copyrighted work to their world and adds it to a collection and decides to share that collection out… they open themselves up to be flagged and get taken down and have that access totally revoked… we want to limit our liability exposure saying that this is user generated, user added, or user manipulated content, and anything that violates copyright material via share links when flagged that goes onto the user. And that should really go into the user agreement and the sharing agreement. When you go to share a collection, we should have an agreement that pops up every single time."

## Context

### The position

**The person who uploads and shares content is its accountable owner.** The
instance is an intermediary. Someone who uploads a publisher's rulebook, puts
it in a collection and shares that collection for other people to adopt has
opened themselves to a notice, a takedown, and the loss of their ability to
share. That is handled through the notice-and-takedown program that already
exists and runs.

### Most of the prose already exists. That is not the problem.

Checked before writing this, because a spec that restates a shipped feature
wastes an afternoon:

- `legal/terms-of-service.md` already says **"You are responsible for what you
  upload. Specifically: that you have the right"**.
- `legal/collection-sharing-terms.md` already says **"You are stating that you
  have the right to share everything in it"**, and is shown at the share step
  by `WorldCollectionsPage.tsx` under spec 026 FR-026. Its own comment says it
  is "a confirmation, not a policy page: a wall of text at a button is a wall
  of text nobody reads."
- `src/server/src/moderation/` implements intake, disable, counter-notice with
  a waiting period and lazy auto-restoration, and repeat-infringer tracking
  with a lookback and a threshold.

**So this feature writes no policy.** It makes the policy real, and there are
exactly three things missing.

### The three gaps

**1. It is shown on one path out of four.** Collections show the terms. The
singleton share links — actor, item, ability — show nothing at all. And
ADR-071 made all three readable *without an account*, so they publish exactly
as much as a collection does. A person who shares a character sheet full of
transcribed rules text has been asked to agree to nothing.

**2. Nothing is recorded.** The terms are displayed and the moment passes.
There is no record of who agreed, when, or to which text. **An agreement
nobody recorded is not evidence** — and evidence is the whole point of the
position above. If a notice arrives in eighteen months, "they clicked
something" is not an answer; "this person, at this time, agreed to this exact
text" is.

**3. The server does not require it.** `create_collection_share_link_impl`
never asks whether the person agreed. The confirmation lives entirely in the
web page, so a direct call to the API produces a share link with no agreement
at all. **A policy enforced only in a UI is a policy with a hole in it**, and
this project already holds the line that authorization belongs at the data
boundary rather than in a component.

### Why this is its own feature rather than a note in another spec

Because it applies to every content type there is and every one there will be.
Audio (spec 038) inherits it. Whatever comes after audio inherits it. Writing
it into each feature separately guarantees that the fourth one forgets.

### One chain, three links

The feature grew a third link in clarification, and they are the same idea at
three scales:

1. **Whoever uploads** content is responsible for having the right to it —
   already the published position in `legal/terms-of-service.md`.
2. **Whoever publishes** it attests to that, at the moment of publishing, and
   the attestation is recorded (US1–US4).
3. **Whoever runs the instance** carries the legal obligations of everything
   inside it (US8).

The third link is the one the project cannot enforce and most needs stated. A
self-hosted instance is beyond our reach by construction — no access, no
takedown, no standing — and the honest response to that is to say so plainly
to the person who takes it on, at the moment they take it on, rather than to
pretend a mechanism exists.

## Clarifications

### Session 2026-09-07

- Q: How does an operator's contact information actually get into the
  instance? → A: **First-run setup collects it, and it lives in the database.**
  Setup already takes the first administrator's account; it should take the
  instance's operator identity and the contact for copyright notices in the
  same pass, and the legal pages should render those values instead of
  carrying `[OPERATOR]` placeholders somebody has to edit in a markdown file
  they may never see. An operator running a container should not have to edit
  the source to become contactable.
- Q: Who is accountable for a self-hosted instance? → A: **The person who
  chose to run it, entirely.** Somebody who takes this software, hosts it and
  fills it with other people's work is not a problem the project can fix: we
  have no access to their instance, no ability to take anything down, and no
  standing to try. So the acknowledgement has to happen where someone actually
  *becomes* an operator — at first-run setup — and it has to be recorded, for
  the same reason a share attestation is.
- Q: What happens when an account accumulates copyright strikes? → A: **Three
  strikes and the account is disabled and subject to deletion.** The person
  then has **thirty days**, in which they may download their data, appeal, or
  both. If they do nothing, the account is deleted at the end of it. A strike
  is an upheld takedown that was not restored — the counting the moderation
  program already does, whose default threshold is already three — so an
  accusation is not a strike and a successful counter-notice removes one.
- Q: What happens to copies already adopted into other worlds when the source
  is taken down? → A: **Disabled with notice, never deleted.** An adopter acted
  in good faith and their world is theirs; silently destroying content inside
  it is a worse act than the one being remedied. The copy stops being usable
  and stops being served, the adopter is told what happened and why, and if
  the notice is later withdrawn or a counter-notice succeeds the copy comes
  back through the same process the source does — the moderation program
  already restores lazily after the counter-notice waiting period, and an
  adopted copy is not a special case.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Asked every time, on every path that publishes (Priority: P1)

Anyone about to publish content — a collection, a character, an item, an
ability, or any type added later — is shown what they are agreeing to and must
agree before it happens. Every time, not once, because each share is a
separate act about different content.

**Why this priority**: three of the four existing paths ask nothing today.
This is the gap that makes the position unenforceable.

**Independent Test**: attempt to publish by each path and confirm each one
asks, and that a second share asks again.

**Acceptance Scenarios**:

1. **Given** a person about to share a collection, **When** they proceed,
   **Then** they must agree before a link exists.
2. **Given** a person about to share an actor, an item or an ability, **When**
   they proceed, **Then** they must agree — the same requirement as a
   collection, because these publish just as widely.
3. **Given** a person who has shared before, **When** they share again,
   **Then** they are asked again.
4. **Given** a person who declines, **When** they decline, **Then** nothing is
   published and they lose nothing they had assembled.
5. **Given** a new shareable content type added later, **When** it can be
   published, **Then** it is covered by this requirement without a separate
   decision.

---

### User Story 2 - The agreement is a record, not a moment (Priority: P1)

When somebody agrees, the instance keeps who agreed, when, to which version of
the text, and for which share — so that a question asked years later has an
answer.

**Why this priority**: equal to US1. Asking without recording produces the
friction of a legal control and none of its value.

**Independent Test**: share something, then look up the attestation and
confirm it names the person, the time, the exact text version, and the share.

**Acceptance Scenarios**:

1. **Given** a completed share, **When** the attestation is looked up, **Then**
   it identifies the person, the moment, the version of the text and the share
   it belongs to.
2. **Given** a share that was later revoked or deleted, **When** the
   attestation is looked up, **Then** it still exists — the record outlives the
   thing it was about.
3. **Given** a notice filed against shared content, **When** it is handled,
   **Then** the attestation for that share is available to whoever handles it.
4. **Given** an attestation, **When** anybody reads it, **Then** it contains
   what was agreed rather than a pointer to whatever the terms happen to say
   now.

---

### User Story 3 - The server requires it (Priority: P1)

Publishing without an attestation is refused wherever the request comes from —
the product's own screens, a script, a direct call.

**Why this priority**: the difference between a control and a decoration. Also
the smallest of the three to state and the easiest to leave out.

**Independent Test**: call the publishing operation directly with no
attestation and confirm it is refused and no link exists.

**Acceptance Scenarios**:

1. **Given** a publish request carrying no attestation, **When** it is made,
   **Then** it is refused and nothing is published.
2. **Given** a publish request carrying an attestation to a text version the
   instance does not recognise, **When** it is made, **Then** it is refused.
3. **Given** a refusal, **When** the person sees it, **Then** they are told
   what is missing without being told anything that would help forge it.

---

### User Story 4 - The words can change, and old agreements still mean what they meant (Priority: P2)

The sharing terms are revised. New shares are made against the new text; every
attestation already recorded still says exactly what that person agreed to.

**Why this priority**: without it, US2's record decays into "they agreed to
whatever it says today", which is the failure it exists to prevent. P2 because
the first version of the text is the one in force on day one.

**Independent Test**: record an attestation, revise the terms, and confirm the
old attestation still resolves to the old words and new shares use the new
ones.

**Acceptance Scenarios**:

1. **Given** revised terms, **When** somebody shares, **Then** they agree to
   the new text and the version recorded says so.
2. **Given** an attestation from before a revision, **When** it is read,
   **Then** it resolves to the text as it was.
3. **Given** the terms are revised, **When** existing shares continue to
   exist, **Then** they are not retroactively treated as agreed to the new
   text.

---

### User Story 5 - Repeat infringement costs the ability to publish (Priority: P2)

Somebody whose shared content is repeatedly taken down loses the ability to
share, on the terms the moderation program already defines.

**Why this priority**: the consequence half of the position. P2 because the
tracking already exists; what is missing is the consequence being connected to
publishing.

**Independent Test**: drive an account past the existing repeat-infringer
threshold and confirm publishing is refused while existing content remains
theirs.

**Acceptance Scenarios**:

1. **Given** an account past the repeat-infringer threshold, **When** they try
   to publish, **Then** they are refused and told why in terms they can act on.
2. **Given** such an account, **When** they use their worlds privately,
   **Then** they can — losing the ability to publish is not losing their
   content.
3. **Given** such an account, **When** their standing changes under the
   existing process, **Then** publishing becomes available again without
   anybody editing a database by hand.

---

### User Story 6 - A takedown reaches everywhere the work went (Priority: P2)

Content taken down stops being served everywhere it travelled by a path the
instance controls — the share link, the collection it was in, and anywhere its
adopters can still reach it through us.

**Why this priority**: a takedown that leaves live copies has not happened.
P2 because the notice-and-takedown mechanism exists; this is about its reach.

**Independent Test**: publish something, have it adopted, take it down, and
confirm every path the instance controls stops serving it.

**Acceptance Scenarios**:

1. **Given** shared content that is taken down, **When** anyone follows the
   share link, **Then** they are refused as they would be for a dead link.
2. **Given** content that was adopted into other worlds before the takedown,
   **When** the takedown is applied, **Then** those copies are disabled rather
   than deleted, their adopters are told why, and the rest of each adopter's
   world is untouched.
2a. **Given** disabled adopted copies, **When** the notice is withdrawn or a
   counter-notice succeeds, **Then** they are restored by the same process
   that restores the source, without the adopter asking.
3. **Given** a takedown, **When** it is applied, **Then** the person who
   shared is told, and the record of their attestation is unaffected.

---

### User Story 7 - Three strikes (Priority: P2)

An account accumulates upheld, unrestored takedowns. It is warned as it goes,
not surprised at the end. On the third, the account is disabled and scheduled
for deletion in thirty days. During those thirty days the person can download
everything they have, appeal, or both. If they do nothing, it is deleted.

**Why this priority**: this is the consequence the whole position rests on —
"they open themselves up to be taken down and have access revoked" is not true
until something revokes it. P2 rather than P1 because the counting already
exists and today merely flags for review: `repeatInfringerFlags` surfaces
accounts at the threshold and nothing acts on it.

**Independent Test**: drive an account to three upheld, unrestored takedowns
and confirm it is warned twice, disabled on the third, offered its data and an
appeal, and deleted at day thirty if neither is used.

**Acceptance Scenarios**:

1. **Given** an account with one upheld strike, **When** it is recorded,
   **Then** the person is told: what it was, that it counts, how many are
   left, and when it stops counting.
2. **Given** an account reaching the third strike, **When** it is recorded,
   **Then** the account is disabled and the thirty-day window begins, and the
   person is told plainly what happens at the end of it.
3. **Given** a disabled account inside the window, **When** the person signs
   in, **Then** they can download their data and file an appeal, and can do
   nothing else.
4. **Given** a disabled account inside the window, **When** the person
   downloads their data, **Then** they get what the export already provides,
   and downloading does not shorten, waive or end the appeal window.
5. **Given** an appeal that succeeds, **When** it is resolved, **Then** the
   account is restored, the deletion is cancelled, and the strike that was
   overturned no longer counts.
6. **Given** the window ending with no successful appeal, **When** it expires,
   **Then** the account is deleted.
7. **Given** a strike that ages past the lookback window, **When** it does,
   **Then** it stops counting toward the three without anybody asking.

---

### User Story 8 - Running it makes it yours (Priority: P2)

Somebody deploys their own instance. During first-run setup — the point at
which they become an operator — they are told, in plain language, that the
legal obligations of everything inside this instance are theirs: the content
people upload to it, the notices filed against it, the law that applies where
it runs. They acknowledge it, and the acknowledgement is recorded with the
instance.

**Why this priority**: it is the only moment this can be said to the person it
applies to. An operator never signs up for anything — they clone a repository
and run a binary — so first-run setup is the one place the statement can
land. P2 because the instances that exist today are ours and the people
running them already know; it becomes urgent the moment somebody else deploys
one.

**Independent Test**: bring up a fresh instance, walk first-run setup, and
confirm the operator statement is shown, must be acknowledged, and is recorded
with a version.

**Acceptance Scenarios**:

1. **Given** a fresh instance, **When** the first administrator completes
   setup, **Then** they are shown the operator responsibilities and must
   acknowledge them before setup finishes.
2. **Given** that acknowledgement, **When** it is made, **Then** it is recorded
   with who, when and which version — the same record a share attestation gets.
3. **Given** an operator, **When** they look for what they agreed to, **Then**
   they can read it again without hunting through a repository.
4. **Given** a person using somebody else's instance, **When** they look at
   who is responsible for it, **Then** they are told it is that instance's
   operator and how to reach them — not the project.
5. **Given** an upgrade that changes the operator statement, **When** the
   operator next administers the instance, **Then** they are shown what
   changed and acknowledge the new version.
6. **Given** first-run setup, **When** the operator fills in who they are and
   where copyright notices should be sent, **Then** those values are stored by
   the instance and appear on its published legal pages without anybody
   editing a file.
7. **Given** an instance whose notice contact has never been filled in,
   **When** somebody tries to publish content beyond a world, **Then** they
   are refused and the administrator is told what is missing — an instance
   with nobody to notify may be played on, but may not publish.
8. **Given** an operator who needs to change the notice contact, **When** they
   change it, **Then** it takes effect on the published pages and the change
   is recorded.

---

### Edge Cases

- Someone shares content they did not upload — a collaborator with editing
  rights over a world they do not own. Who attests, and what they are
  attesting to, must be answerable.
- A person adopts a shared collection into their own world and then shares it
  onward. They are now a publisher and attest in their own right.
- The terms change between the dialog opening and the person confirming.
- A share link is revoked and re-issued for the same content.
- An account is deleted after sharing. The attestation must survive as a record
  even when the person is gone, in whatever minimal form is lawful.
- Somebody shares a hundred things in a session. The requirement is per share,
  and the interface must not make honest bulk work unbearable.
- Content is taken down while a recipient is mid-adoption.
- An instance operator who is also the only user — a self-hosted table of six
  friends. The requirement still applies and should not feel absurd.
- A third strike lands against the instance's own administrator, or the only
  administrator. Disabling them must not lock the instance out of itself.
- An account is disabled mid-session, with players at its table depending on
  a world it owns.
- An appeal is filed on day 29 and is not resolved by day 30.
- A strike ages out of the lookback window while the account is disabled.
- The person downloads their data and then successfully appeals.
- Somebody creates a new account to escape a disablement.
- The instance is self-hosted and the operator does not want automatic
  deletion at all.
- An operator ignores every notice sent to their instance. That is their
  exposure, and the product cannot and does not reach into it.
- A notice is sent to the project about content on somebody else's instance.
  There must be a truthful answer available about who to contact instead.
- An instance is deployed and never has its `[OPERATOR]` markers filled in, so
  it publishes terms naming nobody.

## Requirements *(mandatory)*

### Functional Requirements

**Asked, every time, everywhere**

- **FR-001**: Every operation that publishes content beyond its world MUST
  require an attestation before it takes effect.
- **FR-002**: This MUST include collection shares and the singleton share links
  for an actor, an item and an ability, and MUST extend to any shareable
  content type added later without a separate decision.
- **FR-003**: The attestation MUST be required on each publish, not once per
  person, per world or per session.
- **FR-004**: The person MUST be shown what they are agreeing to at the moment
  they agree, in a form short enough to be read.
- **FR-005**: Declining MUST publish nothing and MUST NOT discard work the
  person has assembled.

**Recorded as evidence**

- **FR-006**: An attestation MUST record who agreed, when, which version of the
  text, and which share it authorised.
- **FR-007**: An attestation MUST survive the share it authorised being revoked
  or deleted.
- **FR-008**: An attestation MUST resolve to the text as it was when agreed,
  not to the current text.
- **FR-009**: Whoever handles a notice MUST be able to retrieve the attestation
  for the share in question.
- **FR-010**: Attestation records MUST be retained on a stated schedule, and
  MUST survive the deletion of the account that made them in whatever minimal
  form is lawful.

**Enforced at the boundary**

- **FR-011**: A publish request with no attestation MUST be refused, whatever
  the request's origin.
- **FR-012**: A publish request naming a text version the instance does not
  recognise MUST be refused.
- **FR-013**: A refusal MUST say what is missing without disclosing anything
  that would help fabricate an attestation.
- **FR-014**: The requirement MUST NOT be satisfiable by the client alone; the
  instance MUST be the party that records it.

**Versioned text**

- **FR-015**: The sharing terms MUST carry a version identity that changes when
  the words change.
- **FR-016**: Every version ever attested to MUST remain retrievable.
- **FR-017**: Revising the terms MUST NOT retroactively alter what any earlier
  attestation says was agreed.

**Consequences**

- **FR-018**: Publishing MUST be restricted as strikes accumulate, on a stated
  ladder, rather than only at the final threshold — a person on their way to
  disablement must meet a consequence before the last one.
- **FR-019**: Losing the ability to publish MUST NOT remove the person's
  access to their own content for their own use.
- **FR-020**: Restoration MUST follow the existing process rather than a manual
  database edit.
- **FR-021**: A person refused publishing MUST be told, in terms they can act
  on, and pointed at the process.

**Reach of a takedown**

- **FR-022**: Content taken down MUST stop being served by every path the
  instance controls — the share link, the collection containing it, and any
  onward access the instance mediates.
- **FR-023**: Copies already adopted into other worlds MUST be **disabled, not
  deleted**, when the source is taken down. The rule is fixed and applied
  consistently, never decided per case.
- **FR-023a**: A disabled copy MUST stop being usable and stop being served,
  and MUST NOT be silently removed from the adopter's world.
- **FR-023b**: The adopter MUST be told that a copy has been disabled and why,
  in terms that make clear they are not accused of anything.
- **FR-023c**: Work the adopter did around a disabled copy — their own edits,
  their own additions, the rest of the world it sits in — MUST be unaffected.
- **FR-023d**: Where a notice is withdrawn or a counter-notice succeeds,
  disabled copies MUST be restored by the same process that restores the
  source, without an adopter having to ask.
- **FR-024**: A takedown MUST notify the person who shared, and MUST NOT alter
  their attestation record.

**Strikes, disablement and deletion**

- **FR-027**: A strike MUST be an upheld takedown that was not restored,
  counted within the existing lookback window. An accusation MUST NOT be a
  strike, and a withdrawn notice or successful counter-notice MUST remove one.
- **FR-028**: The person MUST be told each time a strike is recorded: what it
  was for, that it counts, how many remain, and when it stops counting. Nobody
  may reach the third strike having never been told about the first two.
- **FR-029**: A person MUST be able to see their own standing at any time —
  their current strikes, what each was, and when each ages out.
- **FR-030**: On the third strike the account MUST be disabled and scheduled
  for deletion after thirty days, and the person MUST be told what happens at
  the end of that period before it happens.
- **FR-031**: A disabled account MUST retain, for the whole window, the ability
  to download its data and to file an appeal — and MUST be able to do nothing
  else.
- **FR-032**: Downloading MUST provide what the existing export provides, and
  MUST NOT shorten, waive or end the appeal window. Exercising one remedy MUST
  NOT forfeit the other.
- **FR-033**: An appeal that succeeds MUST restore the account, cancel the
  deletion, and remove the strike it overturned.
- **FR-034**: An appeal still unresolved when the window expires MUST pause the
  deletion until it is resolved. Deletion MUST NOT be the outcome of the
  instance being slow.
- **FR-035**: A strike that ages past the lookback while an account is disabled
  MUST be recounted, and an account that falls below the threshold MUST be
  restored without the person asking.
- **FR-036**: Deletion at the end of the window MUST be real deletion, and the
  person MUST have been told it is irreversible at the start of the window,
  not only at the end.
- **FR-037**: What survives deletion MUST be limited to what is lawfully
  required — the attestation records of FR-010 among them — and MUST be stated
  rather than left to the implementation.
- **FR-038**: Content owned by a disabled account MUST stop being served
  publicly, and other people's worlds MUST NOT be destroyed as a side effect
  of one account's disablement.
- **FR-039**: Disablement MUST NOT be able to lock an instance out of its own
  administration; where the account is the last administrator, the action MUST
  require a human decision rather than happening automatically.
- **FR-040**: The threshold, the lookback and the thirty-day window MUST be
  configurable by the operator, as the existing moderation values already are,
  and an operator MUST be able to require a human decision instead of automatic
  deletion.

**The operator of a self-hosted instance**

> **Collection of these values belongs to spec 040 (Instance Setup and
> Configuration), added after this spec was written.** The requirements below
> state *what must be true* — the instance knows who operates it, has somebody
> to notify, and refuses to publish without one. *How they are collected and
> edited* is 040's FR-001 to FR-009. Where the two overlap they must agree;
> if they drift, 040 owns the setup screen and this spec owns the rule.

- **FR-041**: First-run setup MUST present the operator responsibilities and
  MUST require an acknowledgement before setup completes.
- **FR-042**: That statement MUST say plainly that the legal obligations of
  everything in the instance — the content in it, notices filed against it,
  and the law where it runs — belong to whoever operates it.
- **FR-043**: The acknowledgement MUST be recorded with who, when and which
  version, on the same terms as a sharing attestation (FR-006).
- **FR-044**: The operator statement MUST be versioned, and a change to it MUST
  be surfaced to the operator for acknowledgement rather than applied silently.
- **FR-045**: The statement MUST remain readable inside the running instance
  afterwards, not only at setup.
- **FR-046**: The product MUST make clear to a person using an instance that
  the instance's operator is the responsible party, and MUST give them a way to
  reach them.
- **FR-047**: An instance whose operator identity has never been filled in MUST
  say so to its administrator rather than publishing terms that name nobody.
- **FR-048**: Nothing in the product MUST imply that the project can act on
  content in an instance it does not run. Where a person is looking for a
  takedown on somebody else's instance, they MUST be pointed at that instance's
  operator.
- **FR-049**: The operator statement MUST live in the reviewable prose in
  `legal/`, like every other piece of legal text (FR-026).

**Setting the instance up as a place with somebody in it**

- **FR-050**: First-run setup MUST collect, alongside the first
  administrator's account, the instance's **operator identity** (who is
  offering this service) and a **contact for copyright notices**.
- **FR-051**: Those values MUST be stored by the instance, not compiled in and
  not left in a file, and MUST be editable afterwards by an administrator.
- **FR-052**: The published legal pages MUST render those stored values in
  place of the `[OPERATOR]` placeholders they carry today, so that becoming
  contactable requires no source edit.
- **FR-053**: An instance with no notice contact MUST refuse every operation
  that publishes content beyond its world, and MUST tell its administrator
  exactly what is missing. Playing privately MUST remain possible — an
  instance with nobody to notify is a private instance, not a broken one.
- **FR-054**: A change to the operator identity or notice contact MUST be
  recorded — who changed it, when, and what it was before — because a notice
  sent to a stale address is a failure with consequences.
- **FR-055**: Setup MUST state that registering a designated agent, where the
  operator's jurisdiction requires one, is the operator's own obligation and
  is not performed by this software.
- **FR-056**: The notice contact MUST be discoverable by anyone who needs to
  file a notice, without an account.
- **FR-057**: Setup MUST NOT be completable by filling these fields with
  nothing; a blank or obviously placeholder value MUST be refused rather than
  published.

> **FR-050 to FR-057 are built, and they are built in spec 040.**
>
> They were written here first because attestation is what needs them, but
> instance setup is where they belong, and building them twice would have
> produced two operator identities that could disagree. What exists today:
>
> | Here | There |
> |---|---|
> | FR-050, FR-051 | the `operator.*` and `notice.*` declarations in `src/server/src/settings/registry.rs`, stored as rows |
> | FR-052 | `publishedOperatorValues` on `/api/graphql/public`, read by the legal pages |
> | FR-053 | `src/server/src/graphql/publishing_gate.rs`, covered end to end by `apps/web/e2e/publishing-gate.spec.ts` |
> | FR-054 | `instance_setting_changes`, surfaced per key in the instance settings panel |
> | FR-055 | the designated-agent notice in the setup wizard |
> | FR-056 | the same anonymous query as FR-052 |
> | FR-057 | `Validator::NotAShippedPlaceholder` and `NonEmptyAfterTrim` |
>
> So 039 does not implement these; it **depends on** them, and the dependency
> is satisfied. What 039 still owns is the attestation record itself. Spec 040
> T050 is this note.

**Saying what the instance is**

- **FR-025**: The published terms MUST state that content is uploaded, added
  and shared by users, that the person sharing is responsible for having the
  right to, and that notices are handled through the published process.
- **FR-026**: Where the product asks somebody to agree, the words MUST be the
  reviewable prose in `legal/`, not text embedded in a component — the property
  that makes legal review actionable.

### Key Entities

- **Attestation**: one person's agreement, at one moment, to one version of the
  sharing terms, authorising one publish. Outlives what it authorised.
- **Terms version**: an identified, retrievable revision of the sharing text.
- **Publishable act**: any operation that makes content reachable beyond its
  world — today a collection share or a singleton share link.
- **Standing**: whether an account may publish and whether it is disabled,
  derived from its strikes rather than set by hand.
- **Strike**: one upheld, unrestored takedown against an account, counting for
  the length of the lookback window and removable by a successful appeal or a
  withdrawn notice.
- **Termination window**: the thirty days between a third strike and deletion,
  during which download and appeal are both available.
- **Operator acknowledgement**: the instance's own attestation — who brought it
  up, when, and to which version of the operator statement.
- **Instance operator record**: who operates this instance and where notices
  are sent, stored by the instance, editable, and rendered into everything it
  publishes.

## Success Criteria *(mandatory)*

- **SC-001**: 100% of operations that publish content require and record an
  attestation — demonstrated by attempting every publishing path.
- **SC-002**: A publish attempted directly against the interface, bypassing the
  product's screens, is refused without an attestation.
- **SC-003**: For any share, a person handling a notice can retrieve who
  agreed, when, and the exact words, in under a minute and without a developer.
- **SC-004**: After the terms are revised, every previously recorded
  attestation still resolves to the words that were shown at the time.
- **SC-005**: An account past the repeat-infringer threshold cannot publish and
  can still play, demonstrated in one session.
- **SC-006**: Taken-down content is unreachable by every path the instance
  controls within one session of the takedown being applied.
- **SC-008**: A takedown never destroys anything in an adopter's world: after
  one is applied, the adopter still has their world, their own edits and an
  explanation, and the disabled copy returns intact if the notice is
  withdrawn.
- **SC-009**: No account reaches disablement without having been told about
  each earlier strike, demonstrated by walking one through all three.
- **SC-010**: A disabled account can download everything it owns and file an
  appeal within the window, and doing one does not prevent the other.
- **SC-011**: A successful appeal restores an account and cancels its deletion
  with nothing left behind that still treats it as disabled.
- **SC-012**: An appeal unresolved at day thirty never results in a deletion.
- **SC-013**: A fresh instance cannot complete first-run setup without its
  operator acknowledging the responsibilities, and the acknowledgement is
  retrievable afterwards.
- **SC-014**: Anyone using an instance can find out who operates it and how to
  reach them, in one step from the legal pages, with no account.
- **SC-015**: A fresh instance goes from first boot to publishing terms that
  name a real operator and a real notice contact without anybody editing a
  file.
- **SC-016**: An instance with no notice contact cannot publish anything beyond
  a world, and says why.
- **SC-007**: Sharing five things in a row remains a task a person will
  actually complete — the requirement is per share and is not experienced as
  five walls of text.

## Assumptions

- **The person performing the publish attests**, whether or not they own the
  world or uploaded the content. They are the one causing it to be published.
- **Adopting shared content is not publishing.** An adopter attests when they
  publish onward, not when they take a copy.
- **Attestations are retained for at least as long as the notice window they
  might be needed for**, and survive account deletion in a minimal form.
- **The existing repeat-infringer threshold and lookback are reused unchanged.**
  This feature connects a consequence to them; it does not re-tune them. Both
  are already operator-configurable and the default threshold is already three,
  so "three strikes" is the behaviour the counting was built for.
- **The ladder below the third strike is: warned at one, publishing suspended
  at two, disabled at three.** The owner specified the third rung; the first
  two are chosen so that nobody is surprised, and are configurable with the
  rest.
- **A person may download and appeal.** They are not alternatives, whatever
  order they are done in.
- **Deletion is of the account and what only it owns.** Worlds other people
  depend on, and content already adopted elsewhere, follow the rules those
  already have rather than being destroyed by association.
- **Adopted copies are disabled rather than deleted** (decided 2026-09-07,
  see Clarifications and FR-023 through FR-023d). No longer an assumption.
- **The words stay short.** `legal/collection-sharing-terms.md` already argues
  this and it applies to every path added here.
- **Self-hosted instances get the same behaviour.** An operator may not need it
  for six friends; the product does not know which instance is which.
- **The operator record is collected once at setup and edited rarely.** It is
  configuration about the instance, not about any world in it.
- **An instance may run without a notice contact**, and simply cannot publish
  beyond a world until it has one. That is the softest enforcement that is not
  merely advisory.
- **The operator statement is a separate document from the terms of service.**
  The ToS speaks to a person using an instance; this speaks to the person
  running one, and they are read by different people at different moments.
- **The AGPL already disclaims warranty and liability** for the software. The
  operator statement does a different job — it says who carries the obligations
  of *operating a service*, which a software licence does not address — and one
  is not a substitute for the other.

## Out of Scope

- Writing new legal policy. The prose exists; this makes it operative.
- Proactive content inspection, fingerprinting or licence detection.
- Any change to the notice-and-takedown process itself, its forms, its waiting
  periods or its thresholds.
- Attestation for anything that does not publish beyond its world — private
  worlds, private uploads, invites to a table.
- Transient streams that produce no copy, such as spec 038's shared tab
  audio. There is nothing published to attest to.
- Jurisdictional variation in the words shown. One text, versioned.
