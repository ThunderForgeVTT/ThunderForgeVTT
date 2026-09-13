# Feature Specification: Pausing a World's Play

**Feature Branch**: `051-operator-playfield-shutdown`

**Created**: 2026-09-13

**Status**: Draft

**Input**: User description: "I think we should build a facet to be able to shutdown a playfield session for the operator its a useful feature in case of ohh shit or dmca and the operator approves the playfield shutdown"

## The problem

Nothing can reach into a live table from the server.

When a takedown lands on a scene that a table is playing at that moment, the
scene disappears for anybody who loads it afterwards. The table already playing
it keeps everything it has loaded: the map, the tokens, the interactives, and a
live event stream that keeps flowing. Spec 015's T042 made scenes something a
takedown can reach, and found this in doing so. Today "taken down" means "gone
on your next page load".

The same gap turns up in the other cases an operator worries about. There is no
way to stop abuse happening at a table right now, or a compromised account
running a session, except to wait for everyone to leave.

This spec gives an instance operator one lever: **pause a world's live play**.
Every live session in that world ends, the people in it are told plainly that
play has been paused, and the world cannot start playing again until an operator
lifts the pause.

## Decisions (owner, 2026-09-13)

1. **The scope is one world's live play.** Every live session in the world
   ends, whichever scene it was on. Not a single scene, which leaves the rest of
   the world playing the thing that prompted it. Not a panic button for the
   whole instance, which punishes every table for one.

2. **Proposed, then approved by an operator.** Triggers raise a **request** to
   pause a world, and an operator approves or declines it. An operator may also
   pause a world on their own initiative, **immediately**, when it cannot wait
   for a request. Most pauses are considered decisions; the emergency path
   exists so that the considered path never has to be skipped by a workaround.

3. **Locked until lifted.** When a world is paused, everyone in its live play is
   removed with a plain notice that an operator has paused play. The world
   cannot start a live session again until an operator lifts the pause. The
   Game Master sees *that* play was paused and *when*, but not *why*. The reason
   for an abuse report shown to the table could tip off the person being looked
   into, and most people at a paused table have done nothing wrong. Pausing and
   lifting are both recorded: who, when, and on what grounds.

## Where this sits

- **Operators** run the admin portal (spec 042), and with it moderation and the
  notice-and-takedown programme. Operator authority is site-wide, and it is a
  different thing from the roles inside a world: Owner, Game Master, Trusted
  Player and Player (ADR-099). **No role in a world, the world's own Owner
  included, can lift an operator's pause.**
- **The notice-and-takedown programme** (spec 015) disables content. It reaches
  scenes as of T042. This spec does not change what a takedown withholds. It
  makes a takedown able to prompt a pause of the table playing that content.
- **Account standing** (ADR-077) acts on a person across the instance. A pause
  acts on a world. They are separate levers, and one never implies the other.
- **The constitution's DMCA / Content Moderation Guardrail** concerns features
  that make one world's content reachable outside it. This spec exposes nothing
  beyond a world. It makes the takedown programme the guardrail relies on
  effective at a live table rather than only on the next load. It is recorded
  here because it strengthens condition (a) of that guardrail, not because it
  engages it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pause a world's play at once (Priority: P1)

Something is happening at a table that cannot wait. An operator opens the world
in the admin portal, chooses to pause its play, gives the grounds, and confirms.
Within seconds, everyone playing in that world, on any scene and in any browser,
leaves the playfield and sees a calm notice that an operator has paused play in
this world. Nobody has to reload anything.

**Why this priority**: It is the whole point. Every other story is a way of
getting to this moment more carefully, or of coming back from it. On its own it
closes the gap T042 found.

**Independent Test**: Two browsers are playing different scenes of one world.
An operator pauses the world. Both leave the playfield without reloading, and
both see the notice.

**Acceptance Scenarios**:

1. **Given** two people are playing different scenes of one world, **When** an
   operator pauses that world, **Then** both are removed from the playfield
   within seconds, without reloading.
2. **Given** a world has been paused, **When** a removed person looks at their
   screen, **Then** they see a plain, unaccusing notice that an operator has
   paused play in this world. It does not say why.
3. **Given** a world is being paused, **When** other worlds on the instance are
   being played, **Then** their play is untouched.
4. **Given** a person who is not an operator, **When** they try to pause a
   world, including their own, **Then** they are refused.

---

### User Story 2 - A pause holds (Priority: P2)

A paused world stays paused. Nobody at the table can start play again from the
page, by calling the server directly, by reconnecting a browser that was
offline when the pause landed, or by sending changes that browser queued while
offline.

**Why this priority**: A pause that holds only for honest clients is not a
pause. The server decides who may play (constitution Principle III), so the
lock has to be enforced wherever live play starts or continues, not only by
hiding a button.

**Independent Test**: With a world paused, try to join its play from the page,
through the server directly, and from a browser that went offline before the
pause and reconnects after it. Every attempt is refused, and nothing the
offline browser queued is applied.

**Acceptance Scenarios**:

1. **Given** a paused world, **When** anyone with a role in it tries to start or
   join live play from the page, **Then** they are refused, and told play is
   paused.
2. **Given** a paused world, **When** a modified client asks the server directly
   to join its play, **Then** the server refuses.
3. **Given** a browser that was offline when a world was paused, **When** it
   reconnects, **Then** it is refused live play and shown the notice.
4. **Given** changes that browser queued while offline, **When** it reconnects,
   **Then** none of them is applied to the paused world, and the person is told
   they were not.
5. **Given** a paused world, **When** its Owner or a Game Master tries to lift
   the pause, **Then** they are refused; only an operator can lift it.

---

### User Story 3 - A takedown asks for a pause (Priority: P3)

A takedown notice lands on content that a table is playing at that moment. The
takedown does what it already does, and on top of that it raises a request to
pause that world's play. The request waits for an operator. The operator sees
which world, what prompted it, and whether the world is being played now, and
approves or declines.

**Why this priority**: This is how the gap T042 found gets closed for the
normal case: through a decision an operator makes with the facts in front of
them, not through an automatic kill switch.

**Independent Test**: With a table playing a scene, file a takedown against that
scene. A pause request appears for operators, naming the world and the trigger.
Approve it, and the table is paused as in Story 1. Decline a second request,
and the table sees nothing at all.

**Acceptance Scenarios**:

1. **Given** a table is playing a scene, **When** a takedown lands on that scene,
   **Then** a request to pause the world is raised for operators, naming the
   world and the takedown.
2. **Given** a pending request, **When** an operator approves it, **Then** the
   world is paused exactly as in Story 1.
3. **Given** a pending request, **When** an operator declines it, **Then** nothing
   visible to anyone in the world changes, and the decision is recorded.
4. **Given** a takedown lands on content that nobody is playing, **When** it
   takes effect, **Then** no request is raised. The takedown alone withholds the
   content from the next load.
5. **Given** several takedowns land on the same world's live content in quick
   succession, **When** requests are raised, **Then** an operator sees one
   request for that world, carrying every trigger, not one request per notice.

---

### User Story 4 - Lift a pause (Priority: P4)

The problem has been dealt with. An operator lifts the pause, and the world can
be played again the way it could before, with nothing else about it changed.

**Why this priority**: A pause with no way back would be a deletion that nobody
chose. It comes after the others because nothing has to be lifted until
something has been paused.

**Independent Test**: Pause a world, lift the pause, and start play again from
the page. Content that was separately taken down stays withheld after the lift.

**Acceptance Scenarios**:

1. **Given** a paused world, **When** an operator lifts the pause, **Then** its
   Game Master and players can start and join live play again.
2. **Given** a world was paused because of a takedown, **When** the pause is
   lifted, **Then** the taken-down content is still withheld. Lifting a pause
   restores play, not content.
3. **Given** a takedown is reversed by a counter-notice, **When** the content is
   restored, **Then** the world's pause stays in place until an operator lifts
   it. Restoring content does not lift a pause.

---

### User Story 5 - Know what happened, and nothing more (Priority: P5)

The Game Master of a paused world can see that play was paused and when, and
that an operator did it. They cannot see why. Operators can see the complete
record for every pause and every declined request: who, when, on what grounds,
and when it was lifted.

**Why this priority**: The Game Master needs to know their table was not broken
by a bug. The table must not be told the reason, because the reason can tip off
the person being looked into. Operators must be able to account for every pause,
because a lever this strong without a record is one nobody can check.

**Independent Test**: Pause a world and lift the pause. The Game Master sees
both events with their times and no reason. An operator sees who did each, when,
and the grounds given, plus any declined request for that world.

**Acceptance Scenarios**:

1. **Given** a paused world, **When** its Game Master opens the world, **Then**
   they see that an operator paused play, and when, with no reason given.
2. **Given** a pause, **When** an operator reads the record, **Then** it names
   who paused it, when, the grounds, the trigger if there was one, and who
   lifted it and when.
3. **Given** a declined request, **When** an operator reads the record, **Then**
   it is there; **When** anyone in the world looks, **Then** there is no trace of
   it.

---

### Edge Cases

- **The browser already has the content.** A pause stops the server sending
  anything more, and refuses every further request. It cannot erase what a
  browser has already downloaded, or what the offline world cache (spec 028)
  holds on a person's own device. The spec does not promise erasure. It promises
  that nothing more is sent, nothing is accepted, and play cannot continue with
  the server.
- **An offline table.** A browser that is offline when the pause lands learns of
  it when it reconnects. Until then it holds its cache, the same limit as above.
  On reconnecting it is refused, and its queued changes are not applied.
- **An operator pauses their own world**, or a world they play in. They are
  removed like everyone else. They lift it as an operator, not as a member of
  the world.
- **A request for a world that is already paused.** The new trigger is added to
  the pause's record. No second pause is created and the table sees nothing new.
- **A request for a world nobody is playing any more.** The operator sees that
  the world is not being played at the moment, and can still pause it to stop
  play from starting.
- **A world is deleted while paused.** The world goes as worlds do. The record of
  the pause is kept for operators, and does not keep the world alive.
- **The only operator is unavailable.** A request waits. Nothing is paused
  automatically, because a pause that happens with no decision is exactly what
  decision 2 rules out. The takedown itself still takes effect on the next load.
- **Two operators act on the same request at once.** One decision wins, and the
  other operator is told it was already decided and by whom.
- **The notice at an in-person table.** The map may be on a shared screen in a
  room (PRODUCT.md). The notice must be readable across that room, and must not
  shame anyone in front of the table.

## Requirements *(mandatory)*

### Functional Requirements

**Pausing**

- **FR-001**: An operator MUST be able to pause the live play of one world.
- **FR-002**: Pausing a world MUST end every live session in that world,
  whichever scene it was on, and MUST NOT affect any other world.
- **FR-003**: Everyone removed MUST leave the playfield without having to reload,
  within the time stated in SC-001.
- **FR-004**: An operator MUST give the grounds for a pause when making it. A
  pause with no stated grounds MUST be refused.
- **FR-005**: An operator MUST be able to pause a world immediately on their own
  initiative, without a pending request (decision 2).
- **FR-006**: Nobody but an operator MUST be able to pause a world, including the
  world's own Owner, and including by calling the server directly.

**What the table sees**

- **FR-010**: A removed person MUST see a notice that an operator has paused play
  in this world.
- **FR-011**: The notice MUST NOT state or hint at the grounds. It MUST be calm,
  plain and unaccusing, because most people at a paused table have done nothing
  wrong.
- **FR-012**: The notice MUST meet WCAG 2.2 AA, and MUST stay readable on a shared
  screen across a room.
- **FR-013**: The notice MUST tell a person what they can still do: leave, return
  to their worlds, and see that the pause is lifted when an operator lifts it.

**The lock**

- **FR-020**: A paused world MUST refuse the start of any live session, and any
  person joining one, by every path. That includes the page, a direct request to
  the server, a reconnect, and anything that carries live play: the world's
  event stream, live subscriptions, interactives.
- **FR-021**: The refusal MUST be decided by the server. A client hiding a control
  MUST NOT be the only thing standing between a person and a paused world.
- **FR-022**: A browser that reconnects after a world was paused MUST be refused
  live play and shown the notice.
- **FR-023**: Changes a browser queued offline MUST NOT be applied to a paused
  world, and the person MUST be told they were not.
- **FR-024**: The pause MUST NOT stop the world's members from seeing the world
  outside live play: its name, its members, and the fact of the pause. What else
  stays reachable outside play is an assumption recorded below.

**Requests**

- **FR-030**: A takedown that lands on content in a world's live play MUST raise
  a request to pause that world (decision 2).
- **FR-031**: The request MUST name the world, the trigger, and whether the world
  is being played when the operator looks at it.
- **FR-032**: A request MUST pause nothing until an operator approves it.
- **FR-033**: Several triggers for the same world while a request is pending MUST
  be gathered into one request, not raised as several.
- **FR-034**: An operator MUST be able to approve or decline a request. Approving
  pauses the world as in FR-001 to FR-003. Declining changes nothing any member
  of the world can see.
- **FR-035**: When two operators act on one request at once, one decision MUST
  win, and the other operator MUST be told it was already decided and by whom.
- **FR-036**: A trigger for a world that is already paused MUST be added to that
  pause's record, and MUST NOT create a second pause.
- **FR-037**: The request mechanism MUST accept requests from triggers other than
  takedowns, so an abuse-report intake can raise them when one exists. See
  Assumptions: no such intake exists today.

**Lifting**

- **FR-040**: Only an operator MUST be able to lift a pause.
- **FR-041**: Lifting a pause MUST let the world's live play start again, with
  nothing else about the world changed.
- **FR-042**: Lifting a pause MUST NOT restore content that is separately taken
  down. Restoring content after a counter-notice MUST NOT lift a pause. A pause
  and a takedown are independent.

**Knowing what happened**

- **FR-050**: The Game Master of a paused world MUST see that play was paused,
  when, and that an operator did it, and MUST NOT see the grounds or the trigger.
- **FR-051**: Every pause MUST be recorded for operators: who paused it, when, the
  grounds, the trigger if any, and who lifted it and when.
- **FR-052**: Every declined request MUST be recorded for operators, and MUST
  leave no trace visible to anyone in the world.
- **FR-053**: The record MUST outlive the world it describes.

**Proof**

- **FR-060**: An end-to-end test MUST prove that two browsers playing different
  scenes of one world are both removed without reloading when it is paused.
- **FR-061**: An end-to-end test MUST prove a paused world refuses rejoining from
  the page, by a direct request to the server, and after reconnecting from
  offline, and that queued offline changes are not applied.
- **FR-062**: An end-to-end test MUST prove a takedown against content in live
  play raises a request, that approving it pauses the world, and that declining
  another leaves the table seeing nothing.
- **FR-063**: An end-to-end test MUST prove lifting a pause lets play resume, and
  leaves separately taken-down content withheld.
- **FR-064**: Unit tests alone MUST NOT be accepted as proof for any of the above.

### Key Entities

- **Pause**: a world's live play being stopped by an operator. Carries the world,
  who paused it, when, the grounds, the triggers that led to it, and who lifted it
  and when. While a pause is in force, the world's live play is locked.
- **Pause request**: a proposal to pause a world, raised by a trigger and waiting
  for an operator. Carries the world, its triggers, when it was raised, and the
  decision once made: approved, which creates a pause, or declined.
- **Trigger**: what raised a request. A takedown landing on live content today; an
  abuse report once that intake exists; or an operator acting directly.
- **The notice**: what a person removed from a paused world is shown. It says that
  an operator paused play, and nothing about why.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When a world is paused, every connected person playing in it has
  left the playfield within **5 seconds**, without reloading, in 100% of runs.
- **SC-002**: 100% of attempts to rejoin a paused world are refused, by every path
  tested: the page, a direct request to the server, and reconnecting from offline.
- **SC-003**: 0 changes queued offline are applied to a paused world.
- **SC-004**: A paused world's Game Master can tell that and when play was paused
  in one step from the world, and can find no statement of why, verified by
  inspection of every surface a Game Master sees.
- **SC-005**: No surface available to a member of a world shows any trace of a
  declined request.
- **SC-006**: 100% of pauses and declined requests appear in the operator record
  with who, when and grounds.
- **SC-007**: Lifting a pause restores live play in 100% of runs, and leaves
  separately taken-down content withheld in 100% of runs.
- **SC-008**: The notice meets WCAG 2.2 AA, checked with an automated
  accessibility audit and by reading it on a shared screen from across a room.

## Assumptions

- **"Operator" means the instance's site-level administrators** as spec 042
  defines them, not any role inside a world.
- **No abuse-report intake exists yet.** Spec 042 names an abuse report as a kind
  of task an operator handles, and nothing in the product takes one in. Until one
  exists, abuse reported outside the product, by email for example, is handled
  through an operator's immediate pause (FR-005). This spec builds the request
  mechanism so that intake can raise requests later (FR-037), but does not build
  the intake.
- **Only a takedown against content in live play raises a request in this spec.**
  "In live play" means content currently loaded by at least one live session in
  the world.
- **Outside live play, a paused world stays visible to its members** — its name,
  its members and the fact of the pause — so a table is not left wondering where
  its world went. Whether its staging pages, compendium and lore stay editable
  while paused is **left to the plan**, and defaults to *readable, not editable*,
  because editing a paused world is how a problem continues out of sight.
- **A pause does not affect anybody's account standing** (ADR-077), and does not
  file a strike. Those are separate levers an operator may also use.
- **Pauses are not time-limited.** A pause stays until an operator lifts it.
  Automatic expiry would lift a pause with no decision, which decision 3 rules
  out.
- **Notifying the Game Master outside the product** (email) is not in scope. The
  Game Master learns of the pause in the product, in the world.
- **The five-second bound in SC-001** assumes a connected client on the
  instance's stated minimum connection. It will be measured, not assumed, and
  corrected if the measurement disagrees.
