# Feature Specification: Moderation Across the Instance

**Feature Branch**: `053-moderation-across-the-instance`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Project owner, reading `/admin/moderation` on 2026-09-15: "It's just
GUIDs. It's not really like anything that I can actually track." And, on what
the screen should cover: content *and* conduct; every row naming the world, the
account and the thing in words; a history per account; and conduct reports only
within a shared table — "a player can report the Game Master of a world they're
in, and a Game Master can report a player at their table, both ways, nothing
instance-wide."

## The problem

### The moderation screen cannot answer "what is under moderation"

`/admin/moderation` (`apps/web/src/pages/admin/ModerationReviewPage.tsx`) has
three parts, and none of them is a list of what is happening.

- **Open a case** takes a case reference typed into a box
  (`ModerationReviewPage.tsx:150-190`). It answers only for somebody who
  already has the reference in hand, which spec 039's SC-003 is about: a person
  working a notice they received by e-mail.
- **Repeat-infringer flags** lists account identifiers — raw UUIDs in a `<code>`
  element, each with a "View history" button (`:192-218`). An account appears
  here only after it has crossed the strike threshold.
- **Case history** shows, per case, the case UUID and the line
  `{entityType} · {entityId}` (`:240-247`).

So there is no name anywhere on the screen — not the world's, not the account's,
not the thing's — and there is no way to see a case at all unless its account
has already been flagged three times or somebody typed its reference in. That is
not a presentation defect. The server has no query for it: the whole moderation
surface is `moderationStatus`, `moderationCase(caseId)`,
`moderationHistoryForAccount(accountId)` and `repeatInfringerFlags`
(`src/server/src/graphql/queries/moderation.rs:140-180`). Nothing lists open
cases, and nothing returns a name.

The names are not missing from the database. `content_moderation_actions`
(`src/server/src/schema.rs:193-215`) carries `world_id`, `entity_type`,
`entity_id`, `account_id`, and the reporter — `claimant_name` and
`claimant_contact`, which no screen renders. And the product already knows how
to turn an entity into a name: `moderation::reach::name_of`
(`src/server/src/moderation/reach.rs:140-179`) maps an actor to its label, an
item and an ability to their names, a lore entry to its title and a scene to its
name, so that a takedown can tell somebody which of their copies was disabled.
The moderation screen has simply never asked.

### Two content types cannot be moderated, or cannot be shown

- The server understands five entity types — actor, item, lore entry, ability,
  scene (`src/server/src/graphql/types_moderation.rs:14-51`). The web client
  understands four: its union omits `WORLD_ABILITY`
  (`apps/web/src/types/moderation.ts:1-7`). An ability takedown exists on the
  server and cannot be displayed or filed from the app.
- **Collections are not a moderation entity type at all**, and collections are
  shared: `createCollectionShareLink` calls the publishing gate
  (`src/server/src/graphql/mutations_collection_shares.rs:180`) and
  `sharedCollection` answers a caller with no account (ADR-070,
  `src/server/src/graphql/anonymous.rs:1-12`). A collection is the most
  publishable thing in the product and the one thing a notice cannot name.

### Nothing takes in a report about a person

Spec 042 names an abuse report as a kind of task an operator handles
(`specs/042-admin-portal/spec.md:141`), and nothing in the product takes one in.
Spec 051 recorded the same gap in its Assumptions and built its pause-request
mechanism to accept a trigger from an intake that does not exist
(spec 051 FR-037). This spec is that intake.

The one thing that looks like it might serve is in-app feedback (spec 037), and
it must not: a feedback submission is delivered to the project's **public**
repository and keeps the resulting `issue_url`
(`src/server/src/schema.rs:263-277`). A person reporting harassment must never
be one mis-click from filing it in public.

## What this spec is, and is not

**It is** one place where an operator can see what is under moderation across
the instance, in words, and what an account's history is — and a way for
somebody at a table to report conduct at that table.

**It is not** a rewrite of the takedown flow. Spec 015's statutory path —
validation, disable, counter-notice, restoration, retention — and spec 039's
standing ladder and termination window stay exactly as they are. This spec
reads them, lists them and names them. Where it adds a case type (conduct), that
type does not enter the strike ladder; see FR-044.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An operator sees what is under moderation, in words (Priority: P1)

An operator opens the moderation screen and sees every case the instance is
carrying: what kind it is, which world it is in, whose account it belongs to,
which thing it concerns, who reported it, what state it is in and when it last
moved — all named, none of it an identifier.

**Why this priority**: It is the owner's defect, and it is the difference
between a screen that records moderation and a screen that lets somebody do it.

**Independent Test**: File a takedown against a named actor in a named world,
then open `/admin/moderation`. The case appears in the list without anybody
typing a reference, naming the actor, the world, the account that owns it and
the claimant who filed it.

**Acceptance Scenarios**:

1. **Given** cases exist, **When** an operator opens the moderation screen,
   **Then** they are listed without a reference being typed, most recently
   moved first.
2. **Given** a listed case, **Then** it names in words: the kind of case, the
   world, the account the content belongs to, the thing itself, who reported
   it, the case's state, and when it last changed.
3. **Given** a case whose content, world or account has since been deleted,
   **Then** the row still names what it concerned, as it was when the case was
   opened, and says the thing is gone.
4. **Given** many cases, **When** an operator looks for a subset, **Then** they
   can narrow the list by state, by kind and by world, and find a case by name
   or by reference.
5. **Given** a listed case, **When** the operator opens it, **Then** they see
   its whole history in order, with every step named the same way.

---

### User Story 2 - An account's record reads as a history (Priority: P1)

An operator looking at an account sees everything the instance holds about its
standing: the cases against it, in words; how many count as strikes and why;
where it sits on the ladder; and any termination window that is open.

**Why this priority**: "Repeat offender history per account" is the owner's
second requirement, and today it is reachable only for an account that has
already crossed the threshold — which is exactly the wrong time to start
looking.

**Independent Test**: An account with one upheld takedown, one restored by
counter-notice and one conduct report shows all three, with the restored one
marked as not counting and the conduct report marked as outside the strike
ladder.

**Acceptance Scenarios**:

1. **Given** any account, **When** an operator opens its record, **Then** they
   see every moderation case involving it, whether or not it has ever been
   flagged.
2. **Given** a case resolved in the account's favour, **Then** the record says
   so and says it does not count toward the strike ladder.
3. **Given** an account at or over the threshold, **Then** the record says
   which cases put it there and what happens next, consistent with what the
   account itself is told (spec 039's standing surfaces).
4. **Given** an account with an open termination window, **Then** the record
   shows when it opened and when deletion falls due.
5. **Given** an account whose worlds include one with a pause in force
   (spec 051), **Then** the record links to that pause without restating its
   grounds.

---

### User Story 3 - A player reports the conduct of somebody at their table (Priority: P1)

Somebody at a table is being harassed. From the world they are in, they report
it: they say what kind of conduct it was, who it was, what happened, and when.
It goes to the instance's operators and to nobody else. They are told it
arrived.

**Why this priority**: It is the half of moderation the product does not have at
all, and it is the half that involves a person being hurt rather than a rights
holder being annoyed. Equal to US1 because a queue nobody can file into is as
useless as a queue nobody can read.

**Independent Test**: A player in a world reports its Game Master for
harassment. The report reaches operators with both people and the world named;
the Game Master is told nothing; the player can see that their report was
received.

**Acceptance Scenarios**:

1. **Given** a person with a role in a world, **When** they report somebody who
   also has a role in that world, **Then** the report is accepted and appears
   in the operator queue.
2. **Given** the same person, **When** they try to report somebody they share
   no world with, **Then** it is refused, and the refusal explains that reports
   are for people at a shared table.
3. **Given** a Game Master, **When** they report a player at their table,
   **Then** it is accepted on the same terms — the direction of the report does
   not matter.
4. **Given** a submitted report, **Then** the person reported is not told of it
   by the product, and nothing in any surface they can see changes.
5. **Given** a submitted report, **Then** the reporter can see that it was
   received and its state, and nothing more about what operators are doing with
   it.
6. **Given** a report, **Then** it is never delivered outside the instance, and
   in particular never to the path in-app feedback uses.

---

### User Story 4 - An operator acts on a conduct report (Priority: P2)

An operator reads a report with the world, both people and the account history
of each in front of them, and does something: nothing, a warning, a pause of
that world's play, or an action on an account's standing. What they did is
recorded.

**Why this priority**: A report that arrives and can only be read is a promise
the product does not keep. It is second because the intake must exist before
there is anything to act on.

**Independent Test**: Open a report, see both accounts' histories, take an
action, and find the action recorded against the report and against the account.

**Acceptance Scenarios**:

1. **Given** a conduct report, **When** an operator opens it, **Then** they see
   the world, the reporter, the person reported, the category, the account's
   account of what happened, and each account's moderation history.
2. **Given** a report, **When** an operator decides, **Then** they record the
   outcome with a reason, and the report moves to a resolved state that says
   which outcome it was.
3. **Given** a report warranting it, **When** an operator pauses the world,
   **Then** the pause is spec 051's, raised from this report as its trigger,
   and the table sees spec 051's notice and nothing of the report.
4. **Given** a resolved report, **Then** the reporter is told it was resolved
   and is not told what was decided about the other person.
5. **Given** several reports about the same person, **Then** an operator sees
   them together with that account's record.

---

### User Story 5 - Every content type can be moderated and named (Priority: P2)

A notice can name anything a world can publish — actors, items, abilities, lore
entries, scenes and collections — and every one of them reads as a name.

**Why this priority**: It closes two holes rather than presenting existing data
better, which is why it sits after the listing stories. Collections are the
sharpest: the most publishable thing in the product cannot be taken down.

**Independent Test**: File a takedown against a shared collection and against an
ability. Both are accepted, both appear in the list by name, and both disable
the right thing without touching a sibling.

**Acceptance Scenarios**:

1. **Given** a shared collection, **When** a notice names it, **Then** it can
   be filed, disabled and restored like any other content type.
2. **Given** an ability, **When** a notice names it, **Then** it can be filed
   and displayed by the app, not only by the server.
3. **Given** any moderated content type, **Then** the moderation list shows its
   name, and a person whose content it is sees the same name in what they are
   told.
4. **Given** a takedown against a collection, **Then** unrelated content in the
   same world is unaffected (spec 015 FR-010).

---

### Edge Cases

- **A report about somebody who then leaves the world.** The report stands: it
  is judged on the roles that existed when it was made, and the record keeps
  them. Leaving a table does not delete what happened at it.
- **A report about somebody who has left the instance.** Accepted and recorded.
  The account may be gone; the report is what the operator has.
- **Both people report each other.** Two reports, cross-linked, neither
  treated as an answer to the other. An operator sees both.
- **A report used as a weapon.** Reports are not anonymous to operators, and a
  pattern of reports from one account is visible on that account's record. The
  product does not judge motive; it makes the pattern legible.
- **A person with no world.** They can report nobody through this path. The
  instance's other contact routes (support, legal enquiry) are unchanged.
- **The reporter is an operator.** They file as a member of the world, and the
  report enters the queue like any other. An operator deciding on their own
  report is a matter of practice, not of the product; the record names who
  decided.
- **The world is deleted while a report is open.** The report survives, naming
  the world as it was, as spec 051's pause records do (ADR-100 decision 7).
- **A takedown against content in a world with an open conduct report.** They
  are separate cases about separate things and never merge, even when they
  concern the same account.
- **Names that have changed.** A row names the thing as it was when the case
  was opened, and as it is now when the two differ, so that an operator reading
  an old case is not looking for something under a name nobody uses.
- **A hidden creature's name in a case row.** The moderation screen is an
  operator surface and is not bound by a world's in-play visibility rules; the
  name shown to a player is not the name shown here.

## Requirements *(mandatory)*

### Functional Requirements

**Seeing what is under moderation**

- **FR-001**: An operator MUST be able to list every moderation case on the
  instance without knowing any identifier in advance.
- **FR-002**: Every row MUST name, **in words**: the kind of case, the world,
  the account whose content or conduct it concerns, the thing it concerns, who
  reported it, the case's current state, and when it last changed. An
  identifier MAY be shown in addition; it MUST NOT be the only rendering of any
  of those facts.
- **FR-003**: A case whose world, account or content has been deleted MUST
  still name what it concerned, from what was recorded when the case was
  opened, and MUST say that the thing no longer exists.
- **FR-004**: An operator MUST be able to narrow the list by state, by kind and
  by world, and to find a case by a name it carries or by its reference.
- **FR-005**: Opening a case MUST show its whole history in order, each step
  named the way FR-002 requires.
- **FR-006**: The list MUST cover every kind of case the instance carries —
  content takedowns and conduct reports — and MUST say which kind each is.

**An account's record**

- **FR-010**: An operator MUST be able to open any account's moderation record,
  whether or not that account has ever been flagged.
- **FR-011**: The record MUST show every case involving the account, which of
  them count as strikes, which do not and why, and where the account sits on
  the standing ladder (spec 039).
- **FR-012**: The record MUST show an open termination window, when it opened
  and when deletion falls due.
- **FR-013**: The record MUST agree with what the account is told about itself.
  Two surfaces stating a person's standing differently is a defect, not a
  view.
- **FR-014**: The record MUST link to a pause in force on a world the account
  is in (spec 051) without restating that pause's grounds on any surface a
  member of the world can see.

**Reporting conduct**

- **FR-020**: A person with a role in a world MUST be able to report the
  conduct of another person with a role in **that same world**, in either
  direction: a player may report a Game Master or Owner, and a Game Master or
  Owner may report a player.
- **FR-021**: A report MUST be refused when the two people share no world. There
  MUST be no instance-wide reporting path for one member against another.
- **FR-022**: A report MUST carry: the world, the reporter, the person
  reported, a category, the reporter's own account of what happened, and when
  it happened. The category list MUST include harassment of a sexual nature and
  the comparable conduct a table needs words for (see Key Entities), and MUST
  include a category for conduct that fits none of them.
- **FR-023**: A report MUST be delivered only to the instance's operators. It
  MUST NOT be sent to any destination outside the instance, and MUST NOT travel
  the path in-app feedback uses.
- **FR-024**: The person reported MUST NOT be told of a report by the product,
  and nothing on any surface they can see may change because a report exists.
- **FR-025**: The reporter MUST be able to see that their report was received
  and whether it is open or resolved, and MUST NOT be told what was decided
  about the other person.
- **FR-026**: A report MUST stand after either person leaves the world or the
  instance, and MUST record the roles both people held when it was made.
- **FR-027**: Reports MUST be rate-limited per account, so that the path cannot
  become a way to flood an operator or to harass somebody by filing.

**Acting on a report**

- **FR-030**: An operator MUST see, on one screen, the report and the
  moderation record of both accounts.
- **FR-031**: An operator MUST be able to resolve a report with a recorded
  outcome and reason, and the outcome MUST say what was done, including when
  nothing was.
- **FR-032**: An operator MUST be able to raise a pause of the world from a
  report, which is spec 051's pause with this report as its trigger. This spec
  MUST NOT add a second way to stop a table.
- **FR-033**: Resolving a report MUST tell the reporter that it was resolved,
  and nothing about the other person.

**Who may read what, and for how long**

- **FR-040**: A conduct report's content — the account of what happened, the
  category, and who reported whom — MUST be readable only by the instance's
  operators and by its own reporter (their own report only). No world role,
  the world's Owner included, may read a report about their world.
- **FR-041**: A takedown case's claimant contact details MUST be readable only
  by operators and by whoever the statutory process requires them to be
  forwarded to (spec 015 FR-007). Listing a case MUST NOT publish the
  claimant's contact details to anybody the statute does not.
- **FR-042**: Every read of a conduct report by an operator MUST be attributable
  to that operator.
- **FR-043**: A retention period MUST be stated for conduct reports, and
  enforced: a resolved report is kept for that period and then removed, except
  where it is part of an account's standing record or subject to a legal hold.
  Takedown records keep spec 015 FR-013's retention, which this spec does not
  change.
- **FR-044**: A conduct report MUST NOT create a copyright strike and MUST NOT
  be counted by the repeat-infringer ladder. Whether repeated conduct findings
  carry a consequence of their own is Q1 below.

**Every content type**

- **FR-050**: A collection MUST be a content type a takedown can name, disable
  and restore, on the same terms as an actor, item, ability, lore entry or
  scene.
- **FR-051**: Every content type the server can moderate MUST be expressible and
  displayable in the app. A type one half understands and the other does not is
  a defect.
- **FR-052**: Every content type MUST resolve to a name for display, and the
  name MUST be recorded with the case so that FR-003 holds after deletion.

**Proof**

- **FR-060**: An end-to-end test MUST prove that a takedown filed against a
  named actor appears in the moderation list, named, without any identifier
  being typed.
- **FR-061**: An end-to-end test MUST prove that a player can report the Game
  Master of their world, that a Game Master can report a player, and that a
  report of somebody outside the world is refused.
- **FR-062**: An end-to-end test MUST prove that the person reported sees no
  change on any surface available to them.
- **FR-063**: An end-to-end test MUST prove that a takedown against a
  collection and against an ability can be filed, listed by name and resolved.
- **FR-064**: A test MUST prove that a conduct report adds no strike and moves
  no account along the standing ladder.

### Key Entities

- **Moderation case**: one thing being moderated — a content takedown or a
  conduct report — with a kind, a world, an account, a subject, a reporter, a
  state and a history.
- **Content case**: spec 015's takedown, unchanged in flow. Its subject is a
  piece of content; its reporter is the claimant.
- **Conduct report**: a report about a person's behaviour at a shared table.
  Its subject is an account; its reporter is another member of the same world.
- **Category**: what kind of conduct is being reported. Harassment of a sexual
  nature; other harassment or bullying; threats or intimidation; hate or
  slurs directed at a person or group; sharing another person's private
  information; content harmful to a minor; and *something else*, which carries
  the reporter's own words.
- **Account record**: everything the instance holds about one account's
  moderation history — its cases, its strikes, its position on the ladder, any
  termination window.
- **Name snapshot**: what a world, an account or a thing was called when a case
  was opened, kept so a case survives what it describes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator can see every open case on the instance in **one
  step** from the admin portal, with **zero** identifiers required to get
  there.
- **SC-002**: **100%** of listed cases name the world, the account and the
  subject in words; **0** rows render any of those three as an identifier
  alone.
- **SC-003**: An operator can open any account's moderation record in **two
  steps** from a case, for an account that has never been flagged.
- **SC-004**: A player at a table can file a conduct report in **three steps**
  from the world they are in.
- **SC-005**: **100%** of attempts to report somebody with whom the reporter
  shares no world are refused.
- **SC-006**: **0** surfaces available to the person reported change as a result
  of a report, verified by inspecting every surface that person can reach.
- **SC-007**: **0** conduct reports leave the instance by any delivery path.
- **SC-008**: **100%** of conduct reports add **0** copyright strikes.
- **SC-009**: A takedown can be filed and listed by name for **every** content
  type the product can share: actor, item, ability, lore entry, scene,
  collection.
- **SC-010**: A case whose world and content have been deleted still names both
  in **100%** of cases.

## Assumptions

- **"Operator" means the instance's site-level administrators**, as spec 042
  and spec 051 use the word, not any role inside a world.
- **A shared table is the unit of standing to report.** The owner's decision.
  Two people who have never been in the same world have no relationship the
  instance is responsible for, and a reporting path between them is a weapon
  rather than a remedy.
- **Both people's world roles matter for who may file, not for who is
  believed.** The product records who held which role; it takes no view on the
  report's truth.
- **Operators are few and accountable.** This spec makes reports readable by
  operators and records who read them; it does not build a role system for
  moderators below operator. Q2 asks whether it should.
- **The statutory flow is untouched.** Counter-notice, restoration timing,
  repeat-infringer counting and retention stay as spec 015 and spec 039 built
  them. Where this spec lists or names them it is reading, not deciding.
- **Nothing here changes what a world's members can see about each other.**

## Out of Scope

- Any automated judgement of a report: no classifier, no scoring, no automatic
  action. An operator decides.
- Moderation of live speech — voice, text chat at the table — which the product
  does not carry today. A report describes it; the product does not record it.
- A public transparency report.
- Appeals against a conduct decision. Spec 039's standing surfaces already tell
  an account where it stands; an appeals process is its own feature, and Q1
  touches its edge.
- Rewriting the takedown flow of spec 015 or the ladder of spec 039.

## Dependencies

- **Spec 015**: the takedown flow this lists and names.
- **Spec 026**: content collections, the type FR-050 adds.
- **Spec 037**: in-app feedback — named as the path a report must *not* take.
- **Spec 039**: sharing attestation, standing and termination, which the
  account record reads.
- **Spec 042**: the admin portal this screen lives in.
- **Spec 051**: pausing a world's play — the action FR-032 raises, and the
  intake its FR-037 was built to accept.
- **ADR-099**: the world roles that decide who may report whom.

## Decisions (owner, 2026-09-15)

1. **Content and conduct, in the first version.** Not content now and conduct
   later. A moderation screen that covers only copyright is a screen that says
   the instance cares about publishers and not about people at its tables.

2. **Words, not GUIDs.** Every row names the world, the account and the thing.
   The owner's words: "It's just GUIDs. It's not really like anything that I
   can actually track."

3. **History per account.** Repeat offenders are a pattern over time, and the
   record has to be readable before the third strike, not after it.

4. **Conduct reports only within a shared table.** A player may report the Game
   Master of a world they are in; a Game Master may report a player at their
   table; both directions; nothing instance-wide. The owner's reason, in spirit:
   keep it about real tables, not a reporting weapon.

5. **Sexual harassment and comparable conduct are named categories.** The
   product says the words, because a person in that position should not have to
   invent the category while writing the report.

## Questions for the owner

1. **Q1 — Does repeated misconduct carry a consequence the way repeated
   infringement does?** Spec 039 has a ladder for copyright: warn, suspend
   publishing, disable, terminate. Conduct has none, and FR-044 keeps the two
   apart.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | No ladder. Every conduct decision is an operator's own | Nothing to build; consistent across small instances where the operator knows everybody. |
   | B | **A separate ladder with its own thresholds and its own consequences** | Repeat offenders are handled consistently and visibly; costs a second ladder, its own published policy, and an appeals question. |
   | C | One ladder for both | Simplest to read, and wrong: a copyright strike and a harassment finding are not the same currency. |

   **Recommendation: A for this spec, and B as its own feature.** Make the
   pattern legible now (FR-011 shows the cases), and decide the consequence
   deliberately, with a published policy, rather than inventing thresholds in
   a spec about seeing things.

2. **Q2 — Is there a moderator who is not an operator?** Today everything admin
   is `is_admin` with a mandatory second factor. On an instance with many
   tables, the person who should read a harassment report may not be the person
   who should hold the keys to the settings.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | **No. Operators only, for now** | Nothing new to model; the account that can read reports can also change everything else. |
   | B | A moderator role that can read and resolve cases but change no settings | Right for a larger instance; a new role in a product that has kept its role list deliberately short (ADR-099). |

   **Recommendation: A.** Add the role when an instance exists that needs it,
   not before; the spec's requirements are written in terms of "operators" so
   that a later role slots in without rewriting them.

3. **Q3 — How long is a resolved conduct report kept?** FR-043 requires a stated
   period and does not pick one. Recommendation: **two years** from resolution,
   with anything forming part of an account's standing record kept as long as
   that record — long enough to show a pattern across a campaign or two, short
   enough that an instance is not accumulating accounts of people's worst days
   indefinitely.
