# Feature Specification: Unified World Access Links & Consolidated Permission Resolution

**Feature Branch**: `027-unified-access-links`

**Created**: 2026-08-25

**Status**: Draft

**Input**: User description: "Unified world access links and consolidated permission resolution — two tightly coupled concerns landed together because they share the same root cause: authorization primitives that were copied per-noun instead of generalized."

## Overview

Two problems with one root cause: authorization was built once per content type
instead of once. That produced four hand-maintained copies of the same
permission logic, and it left the world **invite link** and the content **share
link** as two half-finished versions of the same idea — each holding the
lifecycle controls the other lacks.

The user-visible consequence is concrete: **a GM cannot currently stop a leaked
invite link.** There is no revoke, no rotation, and the code is short enough to
be worth guessing. This feature closes that, and removes the duplication that
let a permission-cleanup gap ship unnoticed.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Kill a leaked invite link (Priority: P1)

A GM posts an invite link in a chat channel, then realises it reached people
who were never meant to have it. They open the world's invite panel, hit
refresh on that link, and the old link stops working immediately. A fresh
link takes its place, ready to share with the people who should have it.

**Why this priority**: This is the capability that does not exist today at
all. A GM's only current remedy for a leaked link is to remove each unwanted
member after they have already joined and seen the world. Everything else in
this feature is either a supporting control or an internal correctness fix;
this is the one a GM would actually ask for.

**Independent Test**: Create a link, join with it successfully to prove it
works, rotate it, then attempt to join with the old code and confirm refusal —
and with the new code and confirm success. Deliverable on its own, with no
part of User Stories 2-5 present.

**Acceptance Scenarios**:

1. **Given** an active invite link with uses remaining, **When** the GM rotates
   it, **Then** the previous code is refused on its very next use and a new
   code is issued and displayed.
2. **Given** a link that a GM has just rotated, **When** a player who already
   joined using the old code opens the world, **Then** their existing
   membership is unaffected — rotation governs future joins only, and never
   retroactively removes anyone.
3. **Given** an active invite link, **When** the GM revokes it without
   rotating, **Then** the link becomes permanently unusable and no replacement
   is issued.
4. **Given** a link that is already expired or exhausted, **When** the GM
   rotates it, **Then** a usable new code is issued — rotation is not blocked
   by the old link's state.
5. **Given** a member who is not the world's DM, **When** they attempt to
   rotate or revoke a link, **Then** the attempt is refused.
6. **Given** a link capped at 10 uses that has been used 3 times, **When** the
   GM rotates it, **Then** the replacement is capped at 10 with 0 used — the
   same link, clean, not the 7 that remained.

---

### User Story 2 - Removing a member actually removes their access (Priority: P1)

A GM removes a player from their world. That player's elevated rights on
every kind of world content — characters, items, lore entries, abilities —
are gone. If the GM later re-invites them, they come back as an ordinary
member with no lingering edit rights.

**Why this priority**: This is a live defect, not an enhancement. Removal
today cleans up a player's granted rights on characters, items, and lore, but
**not on abilities** — that content type was added later and its cleanup was
never written. A removed-then-readmitted player silently regains edit or
owner rights on abilities they were once granted. It is a privilege leak in
the removal path, which is exactly the path a GM uses when they want someone's
access gone.

**Independent Test**: Grant a member elevated rights on one of each content
type, remove them, re-add them, and confirm they hold no elevated rights on
any type. Fails today on abilities.

**Acceptance Scenarios**:

1. **Given** a member holding an explicit grant on a character, an item, a
   lore entry, and an ability, **When** the GM removes them from the world,
   **Then** all four grants are gone.
2. **Given** that removed member, **When** they are re-invited and rejoin,
   **Then** they hold no elevated rights on any content, and see exactly what
   an ordinary new member sees.
3. **Given** a member removed from World A who holds grants in World B,
   **When** removal completes, **Then** their World B grants are untouched.
4. **Given** a new kind of permissioned content added in future, **When** a
   member is removed, **Then** their grants on that new type are cleaned up
   without anyone having written cleanup code for it specifically.

---

### User Story 3 - See and control a link's lifetime (Priority: P2)

A GM opens the invite panel and can tell, for each link, whether it works
right now and why: active with uses remaining, expired, used up, or revoked.
They can set how long a link lasts and how many people it admits when they
create it.

**Why this priority**: Expiry and use caps already exist but are barely
surfaced — a GM sees a bare "3/10 uses" string and cannot tell a revoked link
from a live one. Once revocation exists (User Story 1) an unreadable panel
becomes actively misleading, because a revoked link looks identical to a
working one. This makes the control introduced in User Story 1 legible.

**Independent Test**: Create links in each state (active, expired, exhausted,
revoked) and confirm the panel reports each distinctly and correctly.

**Acceptance Scenarios**:

1. **Given** links in each of the four states, **When** the GM views the
   panel, **Then** each link's state and reason are shown distinctly.
2. **Given** an active link with a use cap, **When** the GM views it, **Then**
   remaining uses are shown.
3. **Given** an active link with an expiry, **When** the GM views it, **Then**
   when it expires is shown.
4. **Given** a link that expires while the panel is open, **When** the GM
   refreshes the view, **Then** it now reports as expired.

---

### User Story 4 - An unusable link fails cleanly (Priority: P2)

Someone opens an invite link that no longer works. They get one clear message
telling them to ask the GM for a new link — the same message whether the link
expired, was used up, was revoked, or never existed.

**Why this priority**: Uniform failure keeps a stranger holding a dead code
from learning anything about the world behind it — in particular, whether a
given code was ever real. It also stops a legitimate player from being
confused by four different errors that all mean "ask your GM".

**Independent Test**: Attempt to join with a code in each failure category
plus a never-issued code, and confirm all responses are indistinguishable.

**Acceptance Scenarios**:

1. **Given** codes that are expired, exhausted, revoked, and never-issued,
   **When** each is used to join, **Then** all four fail identically in
   message and in observable response.
2. **Given** a user who is already a member, **When** they open a valid link
   for that same world, **Then** they are told they are already a member
   rather than being shown a failure.
3. **Given** a valid link, **When** two people use its last remaining use at
   the same moment, **Then** exactly one succeeds and the other receives the
   standard unusable-link message.

---

### User Story 5 - One authorization path for every content type (Priority: P3)

A member's rights on a character, an item, a lore entry, and an ability are
decided by the same rule, in one place. Adding a new kind of permissioned
content means declaring it once, and it is automatically covered by both
permission checks and member-removal cleanup.

**Why this priority**: No user asks for this directly, and behaviour must not
change at all — which is precisely why it ranks below the fixes above. Its
value is preventive: it is the reason User Story 2's defect cannot recur. It
ships last because it is the largest change with the least visible effect,
and it is safest to land once the correctness fixes are already proven.

**Independent Test**: Assert that a member's effective rights resolve
identically across all four content types under the same conditions, and that
the set of types covered by removal cleanup is derived from the declarations
rather than written out by hand.

**Acceptance Scenarios**:

1. **Given** identical conditions on each content type, **When** effective
   rights are resolved, **Then** all four produce the same answer.
2. **Given** the world's DM with no explicit grants anywhere, **When** rights
   are resolved on any content, **Then** they hold the highest level on all of
   it, and this cannot be removed.
3. **Given** an ability marked visible to the GM only, **When** rights are
   resolved for an ordinary member, **Then** its hidden-ness is decided
   separately from their permission level — a member with edit rights on a
   hidden ability still cannot see it.
4. **Given** the full set of authorization behaviours that exist before this
   work, **When** the consolidation lands, **Then** every one produces an
   identical outcome.

---

### Edge Cases

- **Rotating a link that is being used at that instant.** A join in flight
  when rotation lands must resolve one way or the other, never both — no
  membership created against a code that was concurrently retired, and no
  use consumed from a link that was replaced.
- **Rotation failing partway.** If issuing the replacement fails, the GM must
  be left with exactly one determinate outcome — never zero usable links
  where they had one, and never two.
- **Codes issued before this change.** Existing short codes keep working
  until they expire, exhaust, or are revoked. They are not force-rotated, and
  a GM is not locked out of a world because their only link was invalidated.
- **A code colliding with an existing one.** Issuing a code must never
  silently fail or hand out a duplicate, including when many links are
  created in the same instant. (This exact defect has occurred before, from
  deriving codes from time-ordered values.)
- **A world whose only link is revoked.** The GM must always be able to issue
  a new one; revocation must never leave a world unreachable to its own GM.
- **Rotating repeatedly to defeat a use cap.** Because a replacement resets
  the count (FR-014), a DM can rotate a 1-use link indefinitely to admit any
  number of people. This is accepted: only a DM can rotate, and a DM can
  already create unlimited links. The cap is a convenience control, not a
  security boundary, and must not be described to GMs as one.
- **Removing a member who holds no grants at all.** Cleanup must succeed
  quietly rather than erroring on an empty set.
- **Removing the last DM.** Out of scope here — existing rank rules already
  govern who may remove whom, and this feature does not change them.
- **A link for a world that has since been disabled by moderation.** The link
  must not become a way around that state.

## Requirements *(mandatory)*

### Functional Requirements — Access Links

- **FR-001**: A world access link MUST carry an optional expiry time, an
  optional maximum use count, and an explicit revoked state. A link is usable
  only when none of those conditions disqualifies it.
- **FR-002**: A world's DM MUST be able to revoke a link explicitly, making it
  permanently unusable without issuing a replacement.
- **FR-003**: A world's DM MUST be able to rotate a link — issuing a
  replacement and retiring the previous code in one action — such that the
  previous code fails on its next use.
- **FR-004**: Rotation MUST be atomic. At no point may both codes be usable,
  and a partial failure MUST leave exactly one determinate outcome.
- **FR-005**: Rotation MUST NOT affect anyone who already joined using the
  retired code; it governs future joins only.
- **FR-006**: A newly issued code MUST be drawn from independent randomness
  with at least the entropy of an existing content share code, and MUST NOT be
  derived from any time-ordered or otherwise predictable value.
- **FR-007**: Codes issued before this change MUST continue to work until they
  expire, are exhausted, or are revoked. No live link may be invalidated by
  the change itself.
- **FR-008**: Only a world's DM (its Owner or a GM-role member) may create,
  revoke, or rotate that world's links.
- **FR-009**: The system MUST NOT offer any way to list or enumerate links
  across worlds or across users. A GM listing their own world's links is
  permitted and unchanged; a link MUST otherwise be reachable only by
  possessing its code.
- **FR-010**: A world's DM MUST be able to see, per link, whether it is
  currently usable and why not if it is not — distinguishing active, expired,
  exhausted, and revoked — along with remaining uses and expiry where set.
- **FR-011**: A join attempt with a code that is unknown, expired, exhausted,
  or revoked MUST fail uniformly, with no observable difference between those
  cases.
- **FR-012**: A successful join MUST grant the Player role and MUST consume
  exactly one use, including when attempts are concurrent.
- **FR-013**: A retired or revoked code MUST NOT become usable again, whether
  by later rotation, later link creation, or code reissue.
- **FR-014**: On rotation, the replacement link MUST inherit the retired
  link's configured use cap with its use count reset to zero, and MUST retain
  the retired link's expiry setting. A rotated link is therefore a clean
  instance of the same link, not a continuation of its remaining budget.

### Functional Requirements — Permission Resolution

- **FR-015**: Effective rights on any permissioned world content MUST resolve
  by one rule for every content type: the world's DM holds the highest level
  implicitly and un-removably; otherwise an explicit grant applies; otherwise
  the lowest level applies by default.
- **FR-016**: The world-level DM determination MUST be reachable from a
  location that belongs to no single content type.
- **FR-017**: Introducing a new permissioned content type MUST require
  declaring it in exactly one place, and that declaration MUST by itself
  supply both permission resolution and member-removal cleanup.
- **FR-018**: Removing a member from a world MUST delete every explicit grant
  that member holds on every permissioned content type in that world, with no
  type omitted and no other world affected.
- **FR-019**: Whether content is visible to a viewer at all MUST remain
  separate from what rights they hold on it. Hidden-ness MUST NOT be expressed
  through the permission ladder.
- **FR-020**: Differences in how individual content types store their grants
  MUST be absorbed by the declaration and MUST NOT produce any difference in
  authorization outcome.
- **FR-021**: The consolidation MUST NOT change any existing authorization
  outcome. Every case that resolves a given way before MUST resolve the same
  way after.

### Key Entities

- **World Access Link**: A code that admits its bearer to one world as a
  member. Carries who issued it, when, an optional expiry, an optional use
  cap, how many times it has been used, and whether it has been retired.
  Supersedes the current invite record.
- **Link State**: The derived answer to "does this work right now" — active,
  expired, exhausted, or revoked — together with the reason. Derived from the
  link, never stored as an independent truth that could drift.
- **Permissioned Content Type**: A declaration that a kind of world content
  participates in the permission model, naming where its grants live and how
  they tie back to a world. The single place a new content type is registered.
- **Permission Grant**: One member's explicit rights on one piece of content.
  Absent by default.
- **Effective Rights**: The resolved answer for a member and a piece of
  content, combining DM status, any grant, and the default.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can make a leaked link stop working in under 15 seconds
  from opening the invite panel, and the retired code fails on its very next
  use with no delay or caching window.
- **SC-002**: Every permissioned content type is covered by member-removal
  cleanup, verified by a check that derives the list of types from their
  declarations rather than restating it — so a future type cannot be missed.
- **SC-003**: Authorization outcomes are unchanged by the consolidation: the
  entire existing authorization test suite passes without modification, and no
  test's expected outcome is edited to accommodate the change.
- **SC-004**: A GM can determine why a link is not working from the panel
  alone, without trying the link and without contacting support.
- **SC-005**: Attempts to join with unusable codes are indistinguishable from
  one another, so possession of a dead code reveals nothing about whether it
  was ever valid or what world it belonged to.
- **SC-006**: 100% of invite codes valid immediately before the change remain
  valid immediately after it.
- **SC-007**: Issued codes are drawn from a space large enough that guessing a
  valid one is infeasible — matching the strength already used for content
  share links, and a substantial increase on the current invite code.
- **SC-008**: A member removed and then re-admitted holds zero elevated rights
  on any content type, measured across all four types.

## Assumptions

- **A world has many links, not one.** The existing model already allows
  multiple invite links per world and the GM panel lists them. Rotation
  therefore acts on one specific link, replacing it in place, rather than
  reissuing the world's entire set.
- **Rotation resets the use count.** Decided 2026-08-25: a rotated link is a
  clean instance of the same link (FR-014), on the reading that a refresh
  button means "give me my link back, new". The consequence — that a cap can
  be reset by rotating — is accepted and recorded under Edge Cases.
- **Revocation is recorded, not erased.** A retired link is marked unusable
  and kept, matching how content share links already behave, so that a code
  can never be resurrected and so past use remains accountable.
- **Expiry stays optional.** Links continue to be creatable with no expiry,
  preserving current behaviour. This feature does not impose a maximum
  lifetime on new links.
- **Old short codes are grandfathered, not upgraded.** Existing codes keep
  working as-is (FR-007). They are not force-rotated, since doing so would
  break links GMs have already distributed.
- **What a link grants is unchanged.** Joining still confers the Player role;
  promoting someone remains a separate action.
- **The lore content type's differing internal storage is absorbed, not
  migrated.** Its grants are stored under a differently-named user reference
  than the other three; that difference is handled by the declaration rather
  than by altering live data.
- **Visibility rules are untouched.** GM-only content stays hidden by its own
  mechanism; this feature only ensures the consolidation does not entangle
  that with permission level.
- **No new content-sharing surface is created.** Access links admit people to
  a world; they do not expose one world's content to another. The platform's
  content-moderation guardrail and the existing determination on record for
  share links therefore apply unchanged, and no new determination is required
  for this feature.
- **Existing rank rules govern who may remove whom.** This feature changes
  what removal cleans up, not who is allowed to remove.
