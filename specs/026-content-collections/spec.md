# Feature Specification: Content Collections

**Feature Branch**: `026-content-collections`

**Created**: 2026-08-25 (as a stub) · **Specified**: 2026-09-04

**Status**: Draft

**Input**: Direction recorded during spec 025's DMCA determination
(`docs/adrs/20260825-049-share_link_dmca_repository_determination.md`): "for now
content is shared in singletons or in packs (packs are a new concept we haven't
designed for) — the idea is that users can correlate items, actors, scenes, the
works into a pack and share the pack with versioning, but that's later work."

## Context

Today a share link points at exactly one artifact — one ability, one item, one
actor. Spec 025 built that machinery in full: an unguessable code, a read-only
preview, a transactional deep copy into a destination world, and revocation.

A **collection** is that mechanism with a different unit. A Game Master gathers
many artifacts they authored — items, actors, abilities, lore entries, scenes —
into one named bundle and shares it as a unit. A recipient copies the whole
thing into a world of their own in one action, instead of a dozen links one at
a time.

**It is not called a pack.** Spec 032 gave that word a closed, two-member
definition enforced by a directory: a pack is a system pack or an interface
pack, it lives under `packs/`, and it is compiled into the product. A collection
is authored by a user, inside a world, at runtime, and lives in the database.
Every property that makes a pack safe to execute is one a collection does not
have. "Bundle" was rejected too: `bundle` appears in this codebase only as
"bundled", carrying ADR-029's distinction between code compiled into the product
and code that is not.

### What this is not

A **public registry or browsable marketplace**. ADR-049 records that as a future
consideration only, gated on demonstrated demand and a fresh review. Collections
are shared by link, exactly like singletons, and FR-020 below forbids the
enumeration that would turn one into the other by increment.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master gathers their work and shares it once (Priority: P1)

A Game Master has built a haunted manor: the manor's rooms as scenes, the ghosts
that haunt it as actors, the cursed objects as items, and the lore explaining
why. They gather these into one collection, name it, and share the link with a
friend running their own game. The friend sees what is in it before deciding,
copies it into their world in one action, and finds every piece present and
independently theirs — free to rename the ghosts and rewrite the lore without
either world affecting the other.

**Why this priority**: It is the whole of the request and the only story that
delivers value alone. Everything else in this spec either protects it or refines
it.

**Independent Test**: Build a world with several artifacts of different types,
gather them into a collection, open its link as a different user in a different
world, copy it, and confirm every member arrived and that editing a copy leaves
the original untouched.

**Acceptance Scenarios**:

1. **Given** a world containing items, actors, abilities and lore entries,
   **When** their owner gathers some of them into a named collection, **Then**
   the collection lists exactly those members and nothing else.
2. **Given** a shared collection's link, **When** someone who is not a member of
   the source world opens it, **Then** they see what the collection contains
   without gaining any access to the world it came from.
3. **Given** a collection open in a recipient's browser, **When** they copy it
   into a world where they may author, **Then** every member is created there as
   an independent record.
4. **Given** a copied collection, **When** the recipient edits a copy or the
   original owner edits the source, **Then** neither change appears in the
   other.
5. **Given** a collection whose members include an actor that knows an ability,
   **When** it is copied, **Then** the relationship between the copies is
   preserved within the destination world.
6. **Given** a collection containing a scene with a background image, **When**
   it is copied, **Then** the destination scene renders that image.

---

### User Story 2 - The owner stays in control of what they shared (Priority: P1)

The Game Master changes their mind, or the friend they shared with is no longer
someone they want holding it. They revoke the collection. The link stops working
and says so plainly rather than failing obscurely. Copies already made stay with
whoever made them, and the owner is told that plainly too, because a revocation
that implies otherwise is a promise the platform cannot keep.

**Why this priority**: Equal to Story 1, and deliberately not deferred behind
it. ADR-049's determination rests on collections being owner-controlled and
revocable; a build that shares before it can revoke has shipped the half that
creates the liability without the half that answers it.

**Independent Test**: Share a collection, confirm the link works, revoke it,
confirm the link now reports the collection as no longer available, and confirm
a copy taken beforehand is unaffected.

**Acceptance Scenarios**:

1. **Given** a shared collection, **When** its owner revokes it, **Then** the
   link reports it as no longer available rather than as an error or a missing
   page.
2. **Given** a revoked collection, **When** anyone opens its link, **Then** no
   member's content is visible, including to someone who had opened it before.
3. **Given** a collection copied before revocation, **When** it is revoked,
   **Then** the copies are untouched, and the owner is told this is what
   revocation does.
4. **Given** a collection, **When** its owner deletes it entirely, **Then** the
   link behaves as revoked and no member's content remains reachable through it.

---

### User Story 3 - A takedown reaches a member without hiding the rest (Priority: P2)

Someone files a copyright notice about one item inside a collection that has
been shared. That item is disabled. The collection remains usable and the other
members are still there — but the disabled item is not reachable through it, is
not copied by anyone who copies the collection afterwards, and its absence is
visible rather than silent.

**Why this priority**: This is the constraint ADR-049 named as needing genuine
design thought rather than copying, because a collection is a *set* and
"one member is disabled" has no obvious answer. It is P2 rather than P1 only
because it cannot be built before Story 1 exists to be moderated.

**Independent Test**: Share a collection of several members, file a valid
takedown against one, and confirm the collection still opens, the disabled
member is absent and shown as withheld, the remaining members still copy, and a
copy made afterwards does not contain the disabled member.

**Acceptance Scenarios**:

1. **Given** a shared collection, **When** one member is disabled by a
   moderation action, **Then** opening the collection shows the remaining
   members and shows that something has been withheld, without naming what.
2. **Given** a collection with a disabled member, **When** someone copies it,
   **Then** the disabled member is not created in the destination.
3. **Given** a collection whose every member has been disabled, **When** it is
   opened, **Then** it reports that nothing is available rather than presenting
   an empty collection as though it were complete.
4. **Given** a disabled member whose takedown is later reversed, **When** the
   collection is opened, **Then** that member is present again without the owner
   having to rebuild the collection.

---

### User Story 4 - The recipient understands what they are taking (Priority: P3)

Before copying, the recipient can see what the collection will add to their
world: how many of each kind of thing, roughly how large it is, and what it will
not bring with it. After copying, they are told what arrived and what did not,
so a collection that referenced something it did not contain does not leave them
hunting for a ghost.

**Why this priority**: It makes the feature usable rather than merely
functional, and it is the part most safely deferred — a collection that copies
correctly but explains itself poorly is a worse product, not a broken one.

**Independent Test**: Open a collection containing a mix of types, confirm the
preview states what it will add, copy it, and confirm the result names anything
that could not be brought across.

**Acceptance Scenarios**:

1. **Given** a collection, **When** a recipient opens its link, **Then** they see
   what kinds of thing it contains and how many of each before copying.
2. **Given** a collection whose member references something outside it, **When**
   it is copied, **Then** the recipient is told that reference could not be
   brought across, and by what.
3. **Given** a copy in progress, **When** any part of it fails, **Then** nothing
   is left half-created in the destination world.

---

### Edge Cases

- **A member is deleted from its world after being added to a collection.** The
  collection must not become unopenable, and must not resurrect the deleted
  artifact.
- **A member's world is deleted.** Same question, larger blast radius.
- **The collection's owner loses authority over the source world**, or is
  removed from it, while the collection is shared.
- **A member the owner never had the right to share.** Ownership of what is in a
  collection is not established by putting it in one.
- **Two members of the same type with the same name.** Copying must produce two
  records, not one.
- **A scene whose background image is shared, by content, with another world's
  scene.** Copying must not create a second copy of bytes the platform already
  holds, and revoking must not make another world's scene lose its background.
- **A collection copied into the world it came from.** Legitimate — duplicating
  one's own work — and must not collide with the originals.
- **A very large collection**: hundreds of members, or a scene with a large
  image, copied over a slow connection.
- **The same collection copied twice into one world**, deliberately or by a
  double-click.
- **A member that is itself a copy** taken from someone else's share.

## Requirements *(mandatory)*

### Functional Requirements

**Gathering**

- **FR-001**: A user with authority over a world MUST be able to create a named
  collection and add artifacts from that world to it.
- **FR-002**: A collection MUST be able to hold members of more than one type —
  at minimum items, actors, abilities and lore entries.
- **FR-003**: A collection MUST only hold artifacts from the world it belongs
  to. A collection spanning worlds is a different feature and is out of scope.
- **FR-004**: Adding an artifact to a collection MUST NOT alter that artifact,
  and removing it MUST NOT delete it.
- **FR-005**: A collection MUST be editable — members added and removed — for as
  long as its owner holds authority over its world.

**Sharing**

- **FR-006**: A collection MUST NOT be reachable outside its world until its
  owner explicitly shares it.
- **FR-007**: A shared collection MUST be reachable only by possessing its link.
- **FR-008**: The share code MUST be unguessable and MUST NOT encode when it was
  created. (A v7-style identifier front-loads a timestamp, which both narrows a
  search and leaks creation time.)
- **FR-009**: Opening a collection's link MUST NOT grant any access to the world
  it came from, nor reveal that world's other content.
- **FR-010**: A collection's owner MUST be able to revoke it, after which its
  link reports it as no longer available — a distinct state from a link that
  never existed and from an error.
- **FR-011**: Revocation MUST NOT affect copies already made, and the interface
  MUST say so at the moment of revoking rather than implying a reach the
  platform does not have.

**Copying**

- **FR-012**: Copying MUST be a one-time deep copy producing records independent
  of the source, with no referential link back to it.
- **FR-013**: A copy MUST be all-or-nothing: a failure part-way MUST leave
  nothing behind in the destination world.
- **FR-014**: Relationships *between members of the same collection* MUST be
  preserved among the copies — an actor that knows an included ability must
  still know the copy of it.
- **FR-015**: A reference from a member to something **not** in the collection
  MUST NOT be silently dropped. It MUST be reported to the recipient as a
  declared loss.
- **FR-016**: A recipient MUST have authority to author in the destination world
  before a copy may be made there.
- **FR-017**: Copying a collection twice MUST produce two independent sets, not
  a merge or a conflict.
- **FR-018**: A scene's image assets MUST be reachable from the copy without the
  copy depending on the source world continuing to exist.
- **FR-019**: Copying a scene MUST NOT duplicate stored image bytes the platform
  already holds. **This requires reference-counted deletion to exist first** —
  see Assumptions.

**Discovery and moderation**

- **FR-020**: No query MUST list collections — by world, by user, or globally.
  There MUST be no surface from which collections can be browsed, searched, or
  counted beyond a user's own.
- **FR-021**: A member disabled by a moderation action MUST NOT be reachable
  through the collection and MUST NOT be created by a copy made afterwards.
- **FR-022**: A disabled member's absence MUST be visible — the collection says
  something has been withheld — without naming the withheld artifact or
  reproducing its content.
- **FR-023**: Disabling one member MUST NOT disable the collection or hide its
  other members.
- **FR-024**: A collection whose every member is disabled MUST report that
  nothing is available, rather than presenting an empty collection as complete.
- **FR-025**: Restoring a member after a reversed takedown MUST return it to the
  collection without the owner rebuilding anything.
- **FR-026**: The terms shown when sharing MUST state that the person sharing is
  responsible for having the right to share what is in the collection, and that
  a copy taken by someone else is theirs and cannot be recalled.

**Governance**

- **FR-027**: An explicit determination under spec 015's FR-012 — whether
  link-shared collections constitute a "centralized public repository" — MUST be
  recorded and accepted by an accountable owner **before implementation begins**.
  Spec 025's determination for single artifacts is not pre-approval: bundling
  changes the unit of distribution, which is what that review exists to assess.

### Key Entities

- **Collection**: A named set of artifacts belonging to one world. Holds its
  name, its world, who created it, and its current share state.
- **Collection Member**: The association between a collection and one artifact,
  recording which artifact and of what type. Removing a member does not delete
  the artifact.
- **Collection Share**: The unguessable code by which a collection is reachable,
  and its state — active or revoked. Distinct from the collection so that a
  collection may be shared, revoked, and shared again without losing its
  identity.
- **Copy Record**: What a recipient received and when, sufficient to tell them
  what arrived and what could not be brought across. Not a link between the
  copies and the source — FR-012 forbids that — but a receipt for the person who
  took it.
- **Fidelity Note**: A recorded instance of something a copy could not bring
  across, surfaced to the recipient rather than discovered later.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master can gather ten artifacts of mixed types into a
  collection and share it in under three minutes, without reading documentation.
- **SC-002**: A recipient can go from opening a link to having the content in
  their own world in a single confirmed action.
- **SC-003**: 100% of a collection's non-disabled members appear in the
  destination after a copy, and 0% of its disabled members do.
- **SC-004**: Editing any copy produces no observable change in the source
  world, and vice versa, across every member type — verified for all of them,
  not sampled.
- **SC-005**: A revoked link stops serving content within one page load, and
  100% of copies taken beforehand remain intact.
- **SC-006**: Zero instances, across all tested failure modes, of a partial copy
  remaining in a destination world.
- **SC-007**: No query available to any caller returns a list of collections
  beyond those the caller owns — verified by inspection of every read path, not
  by sampling.
- **SC-008**: Copying a collection containing a scene whose image the platform
  already stores adds no additional stored bytes for that image.
- **SC-009**: 90% of recipients shown a collection's preview can correctly state
  what it will add to their world before copying it.
- **SC-010**: A takedown against one member is reflected in the collection
  within the same window spec 015 commits to for the artifact itself.

## Assumptions

- **The unit changes; the machinery does not.** Spec 025 built the share code,
  the read-only preview, the transactional deep copy and revocation for single
  artifacts. This generalises that rather than inventing a second mechanism, and
  a plan that budgets for building sharing from scratch has misread what exists.

- **Reference-counted object deletion is a hard dependency of FR-019.**
  `storage/dedupe.rs` stores one copy of any given image however many rows refer
  to it, and its safety argument is explicit: *nothing in this product deletes
  stored objects, so a reference cannot dangle.* Collections make that
  assumption load-bearing in a new way — copying a scene should share the bytes,
  and revoking or deleting a collection is exactly the feature that invites
  someone to add deletion. **Reference counting must land before anything here
  deletes an object.** Until it does, FR-019 is satisfied by sharing the path
  and deleting nothing.

- **Versioning is out of scope for this delivery, and the omission is
  deliberate.** The original note asked for versioned collections. A new version
  means nothing to someone who already copied v1 — copies are independent, which
  is ADR-049's one-time-deep-copy invariant — or it means an update path, which
  is a genuinely new distribution model and needs its own review. Shipping
  "versions" that do nothing would be worse than not shipping them.

- **Partial copy is out of scope.** A collection is copied whole. Letting a
  recipient take three of ten members is a reasonable want and a separate
  decision; all-or-nothing is the simpler contract and the one FR-013 already
  needs for failure handling.

- **Cross-type references are preserved within a collection and declared lost
  outside it** (FR-014, FR-015). Automatically pulling in an actor's abilities
  because the actor was added would make the collection's contents something the
  owner did not choose, which is worse than a declared loss.

- **This is not called a pack, and not called a bundle.** Settled 2026-09-04.
  "Pack" is closed by spec 032's FR-002 to two compiled-in kinds; "bundle"
  collides with "bundled", ADR-029's word for code compiled into the product.
  See `docs/adrs/20260504-026-pack_architecture_and_pack_type_standard.md`.

- **ADR-049's constraints are inherited in full and are not defaults to be
  revisited.** Non-shared by default, non-discoverable by default, no
  enumeration, unguessable non-timestamped codes, owner-controlled and
  revocable, takedown-effective, one-time deep copy. They are the conditions the
  platform's DMCA determination rests on, and FR-006 through FR-025 are their
  restatement rather than an independent design.

- **The constitution's DMCA guardrail applies and gates implementation.** This
  is precisely the feature category it names — content from one world made
  accessible outside it. FR-027 is that gate, and it is a signature from an
  accountable owner rather than anything a plan can produce.

## Prior art in this repository

- `specs/025-world-abilities-compendium/contracts/ability-share.md` — the
  single-artifact share contract this generalises.
- `src/server/src/graphql/mutations_item_shares.rs` — the shipped
  implementation, including the share code, the `CopyError` orphan-rule
  workaround, and the transactional deep-copy path.
- `docs/adrs/20260825-049-share_link_dmca_repository_determination.md` — the
  governing determination.
- `src/server/src/storage/dedupe.rs` — read its header before designing scene
  copying or revocation; it states plainly why a shared `storage_path` is safe
  only while nothing deletes objects.
- `specs/034-lore-git-sync/` — the most recent feature to pass the same
  guardrail, and a worked example of what FR-027's determination looks like
  (`docs/adrs/20260904-067-...`).
