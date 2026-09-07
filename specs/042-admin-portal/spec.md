# Feature Specification: The Admin Portal — Seeing and Steering the Whole Instance

**Feature Branch**: `042-admin-portal`

**Created**: 2026-09-07

**Status**: Draft — **specification only, and deliberately last.** The project
owner's own framing: it spans the whole codebase, and it is where data
management and data privacy land. It should be tackled after 036–041, several
of which it depends on.

**Input**: Project owner: "the next spec is really from an admin panel perspective. We have the roots of what an admin panel kinda feels like, but what I think we really wanna hammer home is the administration of the entire instance — being able to see the user accounts, the world counts, which user has how many worlds or how much data… being able to see the queues for DMCA takedowns, or use the GitHub app to pull the issues from GitHub… being able to see users' collections and how many shares they have and where they are, and be able to find all these little tendrils in the application."

## Context

### What exists is a set of panels, not a portal

The roots are real and this spec builds on them rather than replacing them.
Eleven `/admin/*` routes already exist — welcome, settings, configuration,
analytics, storage, oauth, system, access, security, moderation — behind a
sidebar and a section shell, with a disk-usage chart, a metrics card, a
manifest editor, an OAuth provider form, an access panel, a security panel and
a moderation review page.

**What is missing is the instance.** Each panel answers a question about a
subsystem. Nothing answers the questions an operator actually opens the portal
with: *who is on my instance, what do they have, where is my storage going,
and what is waiting for me to decide?*

### Four absences, stated precisely

**1. There is no way to see the people.** `all_worlds` exists in the admin
query surface; `all_users` does not. An operator cannot list the accounts on
their own instance — not their roles, not when they last signed in, not
whether they hold a second factor, not their standing under the strike
process. It is the first thing an administrator opens and it is not there.

**2. There is no way to follow a person.** Which worlds they own, what
collections they have assembled, what share links they have issued and where
those point, what they have uploaded. Every one of those exists in the
database and none of it is reachable from the portal.

**3. Storage is measured per directory, not per person.**
`recalculate_disk_usage` walks the filesystem and reports bytes under
`worlds`, `assets`, `client`, `databases` and `modules`. That is a true
picture of a disk and it cannot be attributed to anybody.

**4. There is no queue for anything.** `/admin/moderation` reaches a case only
through `repeatInfringerFlags` — so a case leaves the list the moment its
owner disputes it, which is exactly when a human has 10 to 14 days to decide
it. There is no case search and no "waiting for a decision" list, and the same
is true of every other thing that waits on a person.

### Storage attribution is genuinely hard, and the portal must be honest about it

This is the part most likely to be built wrong, so it is stated in the
requirements rather than left to a query.

`storage/dedupe.rs` records a measurement from 2026-09-03: **4,387 canvas
image assets holding 61 distinct images. 2,695 MB stored for 116 MB of
content.** 3,815 of those rows share their bytes with a row in a *different
world*. Each asset keeps its own row, its own owner and its own permission
check; only the stored object is shared.

So "how much data is behind this user" has **two different honest answers**:

- **What they reference** — add up the assets their worlds point at. Large,
  and counts the same object once per referrer.
- **What deleting them would free** — the objects nothing else refers to.
  Often close to nothing, because the duplication is almost entirely across
  worlds.

A single number labelled "storage" means neither, and an operator making a
decision — who is using my disk, what happens if this account goes — needs to
know which one they are looking at. **Nothing in this product deletes stored
objects today**, which is also why the second number is currently theoretical
and must be labelled as such rather than implied to be actionable.

### The portal is itself a privacy risk

An interface that can see everything is a new way for everything to be seen.
This is why the owner puts it last and why it is the spec where data privacy
lands.

The line this spec draws: **administration is about accounts, ownership,
volume and standing — not about content.** How many worlds somebody has, how
much they store, whether they hold a second factor, whether they are subject
to a strike: administration. What is written in their lore, on their character
sheets, or in their table's chat: not. An administrator who needs to see
content needs a reason that is recorded — a takedown notice names the
material — not a general power.

### What this depends on

- **Spec 040** owns collecting and editing instance configuration. This is the
  surface that configuration lives on; 040 decides what the values are and how
  they resolve.
- **Spec 037** owns feedback and its GitHub destination. This portal shows the
  issues that came back.
- **Spec 039** owns strikes and standing. This portal is where an operator sees
  them.
- **Spec 041** owns second factors. This portal reports who has one.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Who is on this instance (Priority: P1)

An operator opens the portal and sees the accounts: who they are, when they
joined, when they were last seen, what role they hold, whether they have a
second factor, and where they stand. They can search it, sort it, and page
through it without it becoming slow.

**Why this priority**: it is the first thing an administrator opens, and it
does not exist. Every other story here starts from a person.

**Independent Test**: with several accounts on an instance, open the portal and
find one by name, then confirm the columns are true against those accounts.

**Acceptance Scenarios**:

1. **Given** an instance with accounts, **When** an operator opens the people
   view, **Then** they see every account with its role, join date, last-seen,
   second-factor state and standing.
2. **Given** many accounts, **When** the operator searches or filters, **Then**
   they find one without paging by hand, and the view stays responsive.
3. **Given** an account that has never signed in, **When** it is listed,
   **Then** that is visible rather than shown as a blank.
4. **Given** the list, **When** it is displayed, **Then** it contains nothing
   from inside a person's worlds — this is an account list, not a content
   list.

---

### User Story 2 - Following one person's tendrils (Priority: P1)

An operator opens one account and sees what it has: the worlds it owns, the
collections it has assembled, the share links it has issued and what each one
points at, and what it has uploaded. Each of those leads to the thing itself.

**Why this priority**: it is the question behind every real administrative
task — a support request, an abuse report, a storage problem, a departure.
Today it takes a database session.

**Independent Test**: create an account with a world, a collection and a share
link, then find all three from that account's page in the portal.

**Acceptance Scenarios**:

1. **Given** an account that owns worlds, **When** an operator opens it,
   **Then** they see the worlds, how many, and can open each.
2. **Given** an account with collections and share links, **When** it is
   opened, **Then** each is listed with what it contains, whether it is live,
   and when it was issued.
3. **Given** a share link, **When** an operator looks at it, **Then** they can
   see what it points at and revoke it, without having to sign in as the
   person who made it.
4. **Given** an account, **When** its page is displayed, **Then** it shows
   what the account *has*, not what is written inside it.

---

### User Story 3 - Where the storage actually goes (Priority: P1)

An operator sees storage attributed to accounts and worlds, with the portal
saying plainly which of the two possible meanings each figure carries — what
is referenced, or what would actually be freed.

**Why this priority**: "which user has how much data" was asked for directly,
and it is the number most likely to be produced wrongly. A single unlabelled
figure would be worse than no figure, because it would be acted on.

**Independent Test**: with one image imported into several worlds owned by
different people, confirm the portal's figures for each account are consistent
with their stated meaning.

**Acceptance Scenarios**:

1. **Given** storage figures, **When** they are shown against an account or a
   world, **Then** the portal says which meaning is displayed — referenced, or
   reclaimable — and never presents one as the other.
2. **Given** content shared between worlds, **When** referenced totals are
   summed across accounts, **Then** the sum exceeding the disk is explained
   rather than presented as a contradiction.
3. **Given** a reclaimable figure, **When** it is shown, **Then** it is
   labelled as theoretical while nothing in the product deletes stored
   objects.
4. **Given** the existing per-directory view, **When** the new attribution is
   added, **Then** the two agree about the total and each says what it
   measures.

---

### User Story 4 - One place to configure the instance (Priority: P2)

An operator changes anything about the instance from one portal: the values
first-run setup collected, per-feature settings for whatever subsystems exist,
and instance-wide globals — with the source of every live value visible.

**Why this priority**: the panels exist; what is missing is that they are one
place with one rule. P2 because spec 040 owns the values and can land first.

**Independent Test**: change a value in each configurable area from the portal
and confirm it takes effect and reports its source.

**Acceptance Scenarios**:

1. **Given** an operator in the portal, **When** they look for a setting,
   **Then** it is in the portal — not in a file, not only in the environment.
2. **Given** a value fixed by the environment, **When** it is displayed,
   **Then** the portal says so and does not offer an edit that will not take.
3. **Given** a subsystem with its own settings, **When** it is configured,
   **Then** it appears alongside the others rather than in its own dialect.
4. **Given** a change to a load-bearing value, **When** it is made, **Then**
   it is recorded with who made it and what it was before.

---

### User Story 5 - What is waiting for a human (Priority: P2)

An operator sees a work queue: moderation cases awaiting a decision — with the
ones on a clock shown as on a clock — alongside undelivered messages, failed
feedback deliveries and requests for access.

**Why this priority**: the moderation half is a live defect, not a wish. A
counter-noticed case disappears from the only list that reaches it, at the
exact moment somebody must decide it within a statutory window.

**Independent Test**: produce a counter-noticed case and confirm it appears in
the queue with its deadline, and can be opened and decided.

**Acceptance Scenarios**:

1. **Given** a case awaiting a decision, **When** an operator opens the
   queue, **Then** it is there regardless of the account's strike count, with
   the date by which it must be decided.
2. **Given** a case, **When** an operator knows its reference, **Then** they
   can find it directly.
3. **Given** other things waiting — undelivered mail, failed deliveries,
   access requests — **When** an operator opens the portal, **Then** they can
   see that something is waiting without opening each subsystem.
4. **Given** an empty queue, **When** it is shown, **Then** it says so rather
   than looking broken.

---

### User Story 6 - The issues our people filed (Priority: P2)

An operator binds a repository, sets what a filed report looks like, and sees
the issues that came back — open, closed, and what happened to them — without
leaving the portal.

**Why this priority**: feedback that nobody watches is feedback nobody acts on.
P2 because it needs spec 037's destination and spec 040's credentials to exist
first.

**Independent Test**: with a bound repository, file a report and see it appear
in the portal, then close it externally and see the state follow.

**Acceptance Scenarios**:

1. **Given** configured credentials, **When** an operator binds a repository,
   **Then** the binding is validated before it is stored and the portal says
   what it will be used for.
2. **Given** a bound repository, **When** reports are filed, **Then** the
   operator can see them and their current state in the portal.
3. **Given** an issue closed outside the product, **When** the portal is
   opened, **Then** it reflects that.
4. **Given** the templates a report is filed with, **When** an operator edits
   them, **Then** subsequent reports use the new ones and existing issues are
   untouched.
5. **Given** no binding or a broken one, **When** an operator looks, **Then**
   they are told what is missing, and no credential is ever displayed.

---

### User Story 7 - Acting for somebody, on the record (Priority: P2)

An operator handles a person's request — export my data, delete my account —
from the portal, and every such act is recorded: who did it, for whom, when
and why.

**Why this priority**: these obligations exist whether or not there is an
interface, and today they are met with a database session. P2 rather than P3
because a legal request has a clock on it.

**Independent Test**: perform an export and a deletion on behalf of an account
and find both in the record afterwards.

**Acceptance Scenarios**:

1. **Given** a request from an account holder, **When** an operator exports
   that account's data, **Then** they get what the person would have got, and
   the act is recorded.
2. **Given** a deletion request, **When** an operator carries it out, **Then**
   what is deleted matches what the person's own deletion would delete, and
   the act is recorded.
3. **Given** any action taken on somebody's behalf, **When** it is recorded,
   **Then** the record names the operator, the subject, the time and the
   reason.
4. **Given** the record, **When** it is read, **Then** it cannot be edited or
   removed from within the portal.

---

### User Story 8 - The portal cannot read the table (Priority: P1)

An administrator opening the portal can see counts, ownership, storage and
standing — and cannot read a person's lore, character sheets or chat through
it. Where content must be seen, there is a reason attached to seeing it, and
that is recorded.

**Why this priority**: P1 despite being a restriction, because it constrains
every other story here. Built without it, US1 through US7 quietly become a
window into everybody's game.

**Independent Test**: attempt to reach a private world's content from every
portal surface and be refused; then reach it through a moderation case and
find the access recorded.

**Acceptance Scenarios**:

1. **Given** any portal surface, **When** an administrator uses it, **Then**
   they can see that a world exists, who owns it and how large it is, and
   cannot read what is in it.
2. **Given** a moderation case naming specific material, **When** an
   administrator opens that material, **Then** they can see it, and the access
   is recorded against the case.
3. **Given** any content access by an administrator, **When** it happens,
   **Then** it is recorded — content access is an event, never a side effect
   of holding a role.
4. **Given** an account holder, **When** they ask what an administrator has
   looked at, **Then** the record can answer.

---

### Edge Cases

- An instance with tens of thousands of accounts, or one with three.
- An account that owns nothing, and an account that owns a thousand worlds.
- Storage figures computed while an import is running.
- The last administrator tries to remove their own administrator role, or
  delete their own account.
- A share link pointing at content that has since been taken down.
- Two administrators acting on the same account at the same moment.
- A bound repository that is renamed, made private, or has its installation
  removed.
- The instance has no mail configured, so nothing waiting in a queue can be
  announced to anyone.
- An operator opens the portal on a phone during a live incident.
- Deleting an account that is the sole owner of a world other people play in.

## Requirements *(mandatory)*

### Functional Requirements

**Seeing the people**

- **FR-001**: The portal MUST list every account on the instance with its
  role, when it joined, when it was last seen, whether it holds a second
  factor, and its standing.
- **FR-002**: The list MUST be searchable and filterable, and MUST stay
  responsive on an instance far larger than a single table.
- **FR-003**: An account that has never signed in MUST be shown as such rather
  than as a blank.
- **FR-004**: The list MUST contain nothing from inside a person's worlds.

**Following a person**

- **FR-005**: An account's page MUST show the worlds it owns, the collections
  it has assembled, the share links it has issued and what it has uploaded.
- **FR-006**: Each of those MUST lead to the thing itself, within what
  FR-020 to FR-023 permit an administrator to see.
- **FR-007**: An operator MUST be able to revoke a share link from the portal
  without signing in as its owner.
- **FR-008**: An account's page MUST show what the account has, never what is
  written inside it.

**Storage**

- **FR-009**: Storage MUST be attributable to accounts and to worlds, not only
  to directories.
- **FR-010**: Every storage figure MUST state which meaning it carries —
  **referenced** (what this account's content points at) or **reclaimable**
  (what removing it would actually free) — and MUST NOT present one as the
  other.
- **FR-011**: Referenced totals summing to more than the disk MUST be
  explained where they are shown, because shared objects are counted once per
  referrer.
- **FR-012**: A reclaimable figure MUST be labelled theoretical while nothing
  in the product deletes stored objects.
- **FR-013**: The existing per-directory view MUST remain, MUST agree with the
  new attribution on the total, and each MUST say what it measures.

**Configuration**

- **FR-014**: Everything configurable about the instance MUST be reachable
  from the portal, including per-feature settings and instance-wide values.
- **FR-015**: A value fixed outside the portal MUST be shown as fixed, with
  its source, and MUST NOT be offered as an editable field.
- **FR-016**: A change to a load-bearing value MUST be recorded with who made
  it, when, and what it was before.
- **FR-017**: A new configurable subsystem MUST appear in the portal on the
  same terms as the existing ones rather than inventing its own.

**Queues**

- **FR-018**: The portal MUST present the moderation cases awaiting a human
  decision, independently of any account's strike count, each with the date by
  which it must be decided.
- **FR-019**: An operator MUST be able to find a case directly by its
  reference.
- **FR-019a**: Other work waiting on a person — undelivered messages, failed
  deliveries, requests for access — MUST be visible without opening each
  subsystem one at a time.

**What an administrator may see**

- **FR-020**: The portal MUST expose accounts, ownership, volume and standing,
  and MUST NOT expose the contents of a person's worlds — their lore, their
  character sheets, their messages.
- **FR-021**: Where content must be seen, it MUST be reached through a reason
  that names it, such as a moderation case, rather than through a general
  power.
- **FR-022**: Any administrator access to content MUST be recorded as an
  event, with who, what, when and under what reason.
- **FR-023**: That record MUST be able to answer an account holder asking what
  was looked at, and MUST NOT be editable or removable from within the portal.

**Acting for somebody**

- **FR-024**: An operator MUST be able to export an account's data on that
  account's behalf, producing what the person's own export would produce.
- **FR-025**: An operator MUST be able to delete an account on request,
  deleting what the person's own deletion would delete.
- **FR-026**: Every act performed on somebody's behalf MUST be recorded with
  the operator, the subject, the time and the reason.
- **FR-027**: The portal MUST NOT allow an instance to be left with no
  administrator.

**The issues that came back**

- **FR-028**: An operator MUST be able to bind a repository for filed reports,
  with the binding validated before it is stored.
- **FR-029**: The portal MUST show the reports filed from this instance and
  their current state, including changes made outside the product.
- **FR-030**: An operator MUST be able to edit the templates reports are filed
  with; changes MUST apply to subsequent reports and leave existing ones
  alone.
- **FR-031**: Where a binding is missing or broken, the operator MUST be told
  what is missing, and no credential, fragment or length MUST ever be
  displayed.

### Key Entities

- **Account summary**: one person as administration sees them — role, dates,
  second factor, standing. Deliberately not their content.
- **Holdings**: what an account owns — worlds, collections, share links,
  uploads — and where each leads.
- **Storage attribution**: bytes against an account or a world, carrying which
  of the two meanings it has.
- **Work item**: something waiting on a person, with what it is, what it
  concerns, and any deadline.
- **Repository binding**: where filed reports go, what they look like, and
  what came back.
- **Administrative act**: something an administrator did to or on behalf of an
  account, recorded and unalterable from inside the portal.

## Success Criteria *(mandatory)*

- **SC-001**: An operator can find any account on their instance by name in
  under 15 seconds, and see its role, last-seen, second factor and standing
  without leaving the page.
- **SC-002**: From one account, an operator can reach every world, collection
  and share link it owns without using a database.
- **SC-003**: Every storage figure in the portal states which meaning it
  carries; no figure appears without one.
- **SC-004**: A moderation case awaiting a decision is reachable from the
  portal within one session of being created, whatever the account's strike
  count, and shows its deadline.
- **SC-005**: An operator can tell, on opening the portal, whether anything is
  waiting on them.
- **SC-006**: No portal surface exposes the contents of a private world;
  demonstrated by attempting it from every surface.
- **SC-007**: Every administrator access to content, and every act on
  somebody's behalf, appears in a record that names who, what, when and why.
- **SC-008**: An operator can bind a repository and see the state of filed
  reports without leaving the portal, and no credential is ever displayed.
- **SC-009**: An instance cannot be left with no administrator through any
  portal action.

## Assumptions

- **One role: administrator.** A finer-grained staff model — a moderator who
  sees cases but not configuration — is a reasonable later addition and is not
  proposed here.
- **The portal reports; the subsystems own their behaviour.** Moderation
  decisions still follow spec 015's process, strikes follow 039's, and
  configuration values follow 040's. This feature is where an operator reaches
  them.
- **Storage attribution is computed rather than tracked** for a first pass —
  a figure produced on demand from what is stored, not a running total
  maintained on every write.
- **"Standing" means what spec 039 defines**, and the portal displays it
  rather than deciding it.
- **The existing eleven `/admin` routes are kept and grown into**, not
  replaced. A rebuild would throw away working panels to change their
  arrangement.
- **The portal is for one instance.** Managing several deployments from one
  place is a different product.

## Out of Scope

- A finer-grained staff permission model.
- Editing a person's content from the portal. An administrator may take
  content down through the moderation process; they may not rewrite it.
- Impersonating a user, or signing in as them.
- Cross-instance or fleet administration.
- Billing, quotas, or enforcing storage limits. This feature shows where
  storage goes; deciding what to do about it is not proposed here.
- Deleting stored objects. Reference counting is the prerequisite and does not
  exist; FR-012 exists because of that.
- Analytics beyond what administration needs — no engagement metrics, no
  behavioural reporting on the people using the instance.
