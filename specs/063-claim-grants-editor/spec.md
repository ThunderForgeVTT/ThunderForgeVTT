# Feature Specification: A Claim Grants Editor

**Feature Branch**: `063-claim-grants-editor`

**Created**: 2026-09-23

**Status**: Draft

**Input**: User description: "Claiming a character grants that player Editor access on it, in the same transaction as the claim, and releasing the claim takes that access back."

## Why This Exists

Claiming a character is how a player says "this one is mine". Today it says only
that. It records who is playing whom and hands over nothing: the player can open
the sheet, watch it, roll on it — and every write is refused.

The refusal is not cosmetic. In Roll for Shoes, failing a roll is how a character
earns experience, and experience is the whole of progression. A player claims
their character, fails a roll, earns the point, and the point is thrown away into
a small error badge. The play-dock end-to-end suite only proves the rules work
because the test has the Game Master hand the player Editor by hand first, a
workaround recorded in spec 062 (FR-045) and deferred to this spec.

Every ruleset has this shape. A claimed character that cannot be written to is a
character that cannot be played.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A claimed character can be played (Priority: P1)

A player joins a world, opens the list of characters the Game Master has offered,
and claims one. From that moment they can do everything playing it requires:
change its hit points, spend its resources, record what it just learnt, edit its
sheet. No one has to grant them anything, and there is no moment between claiming
and playing in which the character is theirs but refuses to be written to.

**Why this priority**: This is the feature. Without it a claim is a label, and
every ruleset in the product has a progression path that silently fails for
players. Everything else in this spec exists to keep the grant from taking more
than it should.

**Independent Test**: Claim an unclaimed character as a player, then change
something on its sheet, and see the change persist across a reload — with no
permission granted by the Game Master at any point.

**Acceptance Scenarios**:

1. **Given** a world with a character the Game Master has marked available, and a
   player who is a member of that world with no access to that character,
   **When** the player claims it,
   **Then** the player may edit that character, and the change is kept.
2. **Given** the same world,
   **When** the Game Master instead binds that player to that character from the
   players roster,
   **Then** the player may edit it exactly as if they had claimed it themselves.
3. **Given** a player who creates their own character in a world that allows it,
   **When** the character is created and claimed in one step,
   **Then** the player may edit it immediately.
4. **Given** a player who has just claimed a character,
   **When** they look at that character,
   **Then** nothing about it is offered to them as available for claiming by
   anyone else, and no other character has become available because of their
   access.

---

### User Story 2 - The Game Master's control over art still holds (Priority: P2)

A Game Master has drawn or bought the art for a character and does not want it
replaced. They lock the character's art, or they turn off player art for the
whole world. A player claims that character. The lock still holds: they can play
the character in every other respect and cannot change its picture.

**Why this priority**: Two switches exist specifically to constrain the people
who hold characters. Handing claimants the same standing as a Game-Master-granted
editor would make both switches do nothing for exactly the people they were
written for — and a lock that quietly stops working is worse than no lock.

**Independent Test**: Lock a character's art, claim it as a player, attempt to
change its portrait, and see the attempt refused — while editing its sheet
succeeds in the same session.

**Acceptance Scenarios**:

1. **Given** a claimed character whose art is locked,
   **When** the claiming player tries to change its portrait or token,
   **Then** the attempt is refused with a reason, and the sheet remains editable.
2. **Given** a world in which player-changed art is turned off,
   **When** the claiming player tries to change their character's art,
   **Then** the attempt is refused.
3. **Given** a world that permits player art and a character whose art is not
   locked,
   **When** the claiming player changes its portrait,
   **Then** the change is accepted.
4. **Given** a character whose art is locked and a co-Game-Master the Game Master
   has granted Editor by hand,
   **When** that person changes its art,
   **Then** the change is accepted — the lock binds claimants, not people the
   Game Master has deliberately trusted.

---

### User Story 3 - Releasing a character takes back what the claim gave, and only that (Priority: P2)

A Game Master releases a player from a character — the player has left the
campaign, or swapped characters, or the Game Master is reassigning. The player
loses the ability to write to that character. Anything the Game Master granted
that player by hand survives untouched, including access granted before the claim
was ever made.

**Why this priority**: A grant that cannot be taken back is not a grant, it is a
handover. And a release that sweeps up a deliberate grant would make the Game
Master's own decisions unreliable.

**Independent Test**: Claim a character, release the claim, and confirm the player
can no longer edit it; then repeat with a character the Game Master had granted
the player Editor on by hand beforehand, and confirm that access survives the
release.

**Acceptance Scenarios**:

1. **Given** a player whose only access to a character came from claiming it,
   **When** the claim is released,
   **Then** the player can no longer edit that character.
2. **Given** a player the Game Master had granted Editor on a character by hand,
   who then claimed it,
   **When** the claim is released,
   **Then** the player keeps Editor.
3. **Given** a claimed character on which the Game Master has since raised the
   claiming player to Owner,
   **When** the claim is released,
   **Then** the player keeps Owner.
4. **Given** a claimed character on which the Game Master has since lowered the
   claiming player below Editor,
   **When** the claim is released,
   **Then** the player's access is exactly what the Game Master set, and the
   release changes nothing further.
5. **Given** a character that was released and is claimed again — by the same
   player or another,
   **Then** the new claimant may edit it.

---

### User Story 4 - Leaving a world leaves nothing behind (Priority: P3)

A player is removed from a world, or leaves it. Their hold on any character ends,
and so does their ability to write to it. They do not remain able to edit a
character in a world they are no longer part of.

**Why this priority**: It is a small path and an uncommon one, but it is the one
place where the access outlives the thing that justified it, and stale write
access to someone else's campaign is the worst failure this feature could have.

**Independent Test**: Have a player claim a character, remove them from the world,
and confirm they hold no access to that character afterwards.

**Acceptance Scenarios**:

1. **Given** a player who has claimed a character,
   **When** their membership of the world ends,
   **Then** they hold no access to that character that the claim gave them.
2. **Given** that same player being re-admitted to the world afterwards,
   **Then** they hold no access to that character until they claim it again.

---

### Edge Cases

- **A hand grant already exists when the claim is made.** If the player already
  holds Editor or Owner, the claim changes nothing and takes nothing over: that
  access is the Game Master's and survives the release. If the player holds less
  than Editor, or nothing, the claim raises them to Editor and that raise is what
  the release takes back.
- **The Game Master edits access while the claim is live.** Whatever they set is
  what stands, and the claim stops being the reason the player holds it. The
  release afterwards leaves it alone. A claim sets a floor on the day it is made;
  it does not keep re-asserting itself against a Game Master who has decided
  otherwise.
- **The character is deleted while claimed.** Everything about it goes, including
  the claim and the access — already the case and unchanged.
- **The same player claims a second character.** The rules for one claim apply to
  each independently; releasing one does not touch the other.
- **The Game Master claims a character.** Still refused. Game Masters already
  hold Owner on everything in their world and have nothing to gain from a claim.
- **A character not offered for claiming, or a non-player character.** Still
  refused, for the same reasons as today; nothing here changes what may be
  claimed.
- **Two players race for the same character.** One wins, the other is told it is
  already claimed; the loser gains no access.
- **A claim that is refused.** Nothing is granted. A refused claim and a granted
  access must never both happen.

## Requirements *(mandatory)*

### Functional Requirements

**The grant**

- **FR-001**: Claiming a character MUST give the claiming player the ability to
  edit that character.
- **FR-002**: The access MUST be granted as part of the same operation as the
  claim, so that a claim never succeeds without it and the access is never
  granted without a claim.
- **FR-003**: The access MUST be granted identically however the claim came
  about: a player claiming an offered character, a player creating and claiming
  a character in one step, or the Game Master binding a player to a character
  from the players roster. A claim means the same thing and carries the same
  rights by every route.
- **FR-004**: The level granted MUST be Editor — enough to play the character,
  short of the ability to hand it to other people.
- **FR-005**: Where the player already holds Editor or higher on the character,
  the claim MUST leave their access exactly as it is.
- **FR-006**: The grant MUST be enforced where the data is written, not arranged
  by the interface. A client that never calls the interface MUST get the same
  answer.

**Provenance**

- **FR-007**: Access created by a claim MUST be distinguishable from access a
  Game Master granted by hand.
- **FR-008**: When a Game Master sets a player's access on a character by hand,
  that access MUST from then on count as granted by hand, whatever created it
  before. The Game Master has taken the decision over.

**Art**

- **FR-009**: A player whose access came from a claim MUST still be subject to
  the world's player-art setting and the character's art lock, exactly as a
  claimant is today.
- **FR-010**: A player whose access was granted by hand, a Game Master, and an
  administrator MUST keep the unconditional ability to change a character's art
  that they have today.
- **FR-011**: The existing player-art setting and art lock MUST keep their
  current meaning. This feature MUST NOT retire, weaken or bypass either.

**Release**

- **FR-012**: Releasing a claim MUST remove the access that claim created.
- **FR-013**: Releasing a claim MUST NOT change access the Game Master granted by
  hand, whether that grant was made before or after the claim.
- **FR-014**: Access created by a claim MUST NOT outlive that claim by any route
  the claim can end — released by the Game Master, ended because the player's
  membership of the world ended, or ended because the character was deleted.
- **FR-015**: After release, a player whose only access came from the claim MUST
  hold the same access as any other member of the world who never claimed it.

**What must not change**

- **FR-016**: Being able to edit a character MUST NOT make that character
  available for claiming. A Game Master MUST still be able to grant a co-Game-
  Master Editor on a player's character without offering it up to be claimed.
  Claiming implies the ability to edit; the ability to edit implies nothing about
  claiming. These are opposite directions and both hold.
- **FR-017**: Who may claim what MUST be unchanged: only characters the Game
  Master has marked available, never non-player characters, never the world's
  Game Master, one live claim per player and one per character, with the existing
  refusals for an already-claimed character and for a claim that changed
  underneath the request.
- **FR-018**: The Game Master MUST remain able to raise a claiming player to
  Owner, lower them, or remove their access entirely, at any time, and this
  feature MUST NOT undo or fight that.
- **FR-019**: The permission levels themselves MUST be unchanged — the same three
  levels, meaning the same things.

**Cleaning up after the workaround**

- **FR-020**: The end-to-end tests that granted a player Editor by hand purely
  because a claim did not MUST stop doing so where the claim now covers it, so
  the workaround does not survive as precedent for how claiming works.
- **FR-021**: Spec 062's record of the gap MUST point at this spec as the thing
  that closed it.

### Key Entities

- **Claim**: the record that a particular member of a world is playing a
  particular character. One per character, one per member. Ends when the Game
  Master releases it, when the member's membership ends, or when the character is
  deleted.
- **Access grant**: the record that a particular person may view, edit or own a
  particular character. Gains one new property in this feature: whether a claim
  created it, or a person granted it by hand.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A player can claim a character and immediately play it — record
  damage, spend a resource, record what it learnt — with no action by anyone else
  in between, in 100% of attempts.
- **SC-002**: Zero of the product's rulesets lose progression to a refused write
  after a claim. Specifically, the Roll for Shoes play-dock test earns and keeps
  experience with no permission handed over by the Game Master.
- **SC-003**: A Game Master who locks a character's art finds the lock still
  holds against the player holding that character, in 100% of attempts, before
  and after the claim.
- **SC-004**: Releasing a claim removes the claim's access in 100% of cases, and
  removes a hand-granted access in 0% of cases.
- **SC-005**: After a player's membership of a world ends, they hold no access to
  any character in it that came from a claim, in 100% of cases.
- **SC-006**: The end-to-end suite contains no remaining hand-granted-Editor step
  whose only purpose is to work around a claim granting nothing.
- **SC-007**: The feature's slice runs green and finishes in about ten minutes on
  one shard, per Principle VI.

## Constitution Alignment

- **Principle III — ownership at the data boundary.** The grant and its removal
  are decided where the data is written. The interface may show the result; it
  MUST NOT be what produces it (FR-006).
- **Principle IV — an ADR in the same change set.** This moves an ownership
  boundary: until now, holding a character and being able to write to it were
  independent, and this makes one imply the other. The decision, the reason
  provenance is required rather than convenient, and the reason the art switches
  keep binding claimants, all belong in an ADR landing with the change. It
  should be read beside ADR-050 (the permission ladder) and ADR-105 (the
  claim-based art exception it preserves).
- **Principle VI — proven by its own slice.** The proving slice is **`actors`**,
  which already owns the claim, ownership and art specs this feature moves
  between. The membership-removal path (User Story 4) is the one seam that
  reaches outside it; it MUST be exercised from within an `actor-`prefixed spec
  so the proof stays in one slice rather than pulling the accounts slice in
  behind it. The `accounts` slice MUST be confirmed not to regress, since the
  membership path it owns now has a consequence for character access.

## Assumptions

- **A claim sets a floor, not a leash.** It grants on the day it is made and
  releases what it granted. It does not continuously re-assert Editor against a
  Game Master who has since decided otherwise. This is what makes FR-018 and
  FR-012 compatible.
- **Editor, not Owner.** Owner carries the ability to hand the character to other
  people and to change who may claim it. That is the Game Master's to give
  deliberately, so a claim stops short of it.
- **Removing the claim's access entirely, rather than restoring some earlier
  level.** Where the claim raised a player from an explicitly-recorded Viewer,
  removing the access on release leaves them able to view it — the same answer
  that explicit Viewer gave. Nothing is lost by not tracking what was there
  before.
- **Provenance is a property of the access record**, not a separate history. This
  feature needs to answer one question — did a claim create this? — and FR-008
  makes a hand edit the single event that changes the answer.
- **Existing claims at the time this ships** hold no access created by a claim,
  because none existed. Whether to grant access retroactively to characters
  already claimed is a migration question for the plan; the safe default is to
  grant it, since a claimed character that cannot be written to is the defect
  being fixed, and any hand grant already in place is left alone by FR-005.
- **The interface needs no new control.** The Game Master's existing access
  control on a character keeps working as it does; what changes is what it reads
  after a claim. Whether the interface should say *why* someone holds access is
  a presentation question, not a requirement here.

## Out of Scope

- Changing the permission ladder itself, or what each level may do.
- Ownership of items, lore, or anything other than characters.
- Letting a player switch their own character. Releasing and reassigning stays a
  Game Master action.
- Telling a Game Master that someone is waiting to be given a character.
- A general world-settings surface for the art switches (see ADR-108).
