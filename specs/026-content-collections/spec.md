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

## Clarifications

### Session 2026-09-04

- Q: When a lore entry restricted to only some of a world's members is put into a shared collection, who should be able to read it? → A: Refuse restricted members entirely — an artifact restricted to a subset of world members cannot be added to a collection, and the interface says why.
- Q: Should scenes be shareable in a collection from the first delivery? → A: Yes — scenes are in from the start, images included. The interim storage behaviour (share the stored path, delete nothing) makes the reference-counting dependency bind on deletion rather than on copying.
- Q: Does someone need an account to open a collection's link? → A: No — anyone with the link may view it signed out, matching spec 025's existing shares. Copying still requires an account and authority in a destination world.
- Q: Should a collection have a size limit? → A: A member count of 100, refused on adding with a clear message rather than silently truncated. Bytes are not separately bounded because scene images are shared rather than duplicated.
- Q: Who owns the copies, and may a recipient re-share them? → A: The person who performed the copy owns them outright in their own world, with the same rights as anything they authored, and may put them into a collection of their own.

### Session 2026-09-05

Asked after US1 and US2 shipped, so every question below is grounded in
something implementation surfaced rather than in re-reading the prose.

- Q: How should a collection's owner get back to an active share link in order to revoke it later? → A: Show the owner their own collection's link — an owner may retrieve the active share code for a collection they own.
- Q: Should a copied actor's portrait and a copied item's icon come across with the copy? → A: Yes — all image assets travel with the copy, on the same terms as a scene's background.
- Q: When a copied actor's own scene was not part of the collection, where should that actor be placed in the destination world? → A: The destination world's active scene, with the displacement declared.
- Q: A new world's first scene is named after the world, so sharing that scene discloses the world's name. What should change? → A: Seed new worlds with a neutral named starter scene carrying a base map, instead of an empty grid named after the world.
- Q: US4 says a recipient should see "roughly how large it is" — what should the preview show? → A: Nothing; drop the size claim. Counts by type are what SC-009 measures.

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
world: how many of each kind of thing, and what it will not bring with it. After copying, they are told what arrived and what did not,
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
- **A member that becomes restricted after being added** (FR-001b), and the
  reverse: a restriction lifted while the collection is shared, which must
  return the member without the owner rebuilding anything.
- **Two members of the same type with the same name.** Copying must produce two
  records, not one.
- **A member whose name discloses its world**, because the platform chose that
  name rather than the author (FR-009f).
- **A scene whose background image is shared, by content, with another world's
  scene.** Copying must not create a second copy of bytes the platform already
  holds, and revoking must not make another world's scene lose its background.
- **A collection copied into the world it came from.** Legitimate — duplicating
  one's own work — and must not collide with the originals.
- **A collection at its limit**: exactly 100 members, one of them a scene with a
  large image, copied over a slow connection.
- **An anonymous visitor opening a collection repeatedly**, or walking codes.
  FR-009c's rate limit is what stands between an unguessable code and an
  attacker allowed to guess indefinitely.
- **A recipient re-shares what they received**, and their recipient re-shares in
  turn. Each link is its own collection with its own owner and its own
  revocation; revoking the first does not reach the copies made from it, because
  those copies are their owners' own content (FR-012, FR-017a).
- **The same collection copied twice into one world**, deliberately or by a
  double-click.
- **A member that is itself a copy** taken from someone else's share.

## Requirements *(mandatory)*

### Functional Requirements

**Gathering**

- **FR-001**: A user with authority over a world MUST be able to create a named
  collection and add artifacts from that world to it.
- **FR-001a**: An artifact whose visibility is restricted to a subset of its
  world's members MUST NOT be addable to a collection, and the refusal MUST say
  why.

  **This is a refusal rather than a warning, deliberately.** A collection is
  read by anyone holding its link, so a restricted artifact placed in one is
  published to strangers — and that is the single failure in this feature its
  owner cannot undo. A notice warns; a refusal prevents, and nothing here forces
  the choice: an owner who wants to share something restricted may loosen the
  restriction first, which is a deliberate act rather than a side effect of
  adding to a list.

  Spec 034 answered the same question with a mandatory acknowledged notice
  (FR-037), and the difference is worth naming: there, mirroring *cannot*
  preserve per-entry permissions because a repository has one access list, so
  disclosure was the only option available. Here nothing is forced, so the
  stronger answer is the right one.

  **What this covers in practice, found by implementing it.** The clarification
  was asked about "a lore entry restricted to only some of a world's members".
  **No such category exists.** The permission ladder cannot express a
  restriction: `Viewer` is both its floor and its default, and
  `queries/lore.rs` states that "every caller — member or not — defaults to
  `Viewer` when no explicit row exists". A grant row *elevates* one member; it
  never withholds from the others. Verified against the schema, exactly one
  member type carries a genuine restriction:

  | Type | Axis | Restricts? |
  |---|---|---|
  | ability | `world_abilities.gm_only` | **Yes** — defaults false, so setting it is a deliberate act |
  | scene | `scenes.hidden` | **No** — defaults *true*; see FR-001c |
  | item, lore, actor | none | No — every member sees them all |

  FR-001a therefore binds on GM-only abilities today. It is still implemented
  exhaustively across all five types, so that adding a restriction axis later
  lands in a function that already has a place for it.
- **FR-001c**: A scene MUST NOT be refused for being `hidden`. That flag is
  **play-staging state, not a permission** — it defaults to true, so every
  scene is hidden when created, and refusing on it would refuse nearly every
  scene in the product while FR-002 puts scenes in scope precisely because
  sharing a *place* is the flagship case.

  It would also force a worse outcome than it prevents: to share a scene, an
  owner would first have to unhide it **in their own world**, revealing the room
  to their players mid-campaign as a side effect of sharing it with a friend.
  The deliberate act is adding the scene and sharing the collection, which is
  the same standard every other member type is held to.
- **FR-001b**: An artifact already in a collection that *becomes* restricted MUST
  be treated as withheld from that point (the same behaviour FR-021 gives a
  moderated member): absent from the collection, not copied, and its absence
  visible without naming it. A restriction applied after the fact must take
  effect, or the refusal in FR-001a is a gate with a way around it.
- **FR-002**: A collection MUST be able to hold members of more than one type:
  items, actors, abilities, lore entries **and scenes**.

  Scenes were ambiguous in the first draft — absent from this list while Story 1
  and FR-018/FR-019 assumed them — and are confirmed in scope (clarified
  2026-09-04). They are also the point: the flagship use is sharing a *place*,
  and a haunted manor without its rooms is a list of nouns. They are the only
  member type carrying binary assets, which is where FR-019's storage question
  comes from, and deferring them would only mean answering it later anyway.
- **FR-003**: A collection MUST only hold artifacts from the world it belongs
  to. A collection spanning worlds is a different feature and is out of scope.
- **FR-004**: Adding an artifact to a collection MUST NOT alter that artifact,
  and removing it MUST NOT delete it.
- **FR-005**: A collection MUST be editable — members added and removed — for as
  long as its owner holds authority over its world.
- **FR-005a**: A collection MUST hold at most **100 members**. An attempt to
  exceed that MUST be refused when adding, with a message naming the limit —
  never silently truncated, and never accepted-then-failed at copy time.

  A count rather than a byte ceiling, because a count is what a person can
  reason about and what decides whether copying stays one action they wait out
  rather than a background job with progress and resumption. Bytes are not
  separately bounded: they are dominated by scene images, which the platform
  already stores once however many rows refer to them, so a second limit would
  mostly refuse copies that cost nothing.

  A hundred is enough for a substantial adventure module. If it ever binds in
  practice, raising it is a decision with evidence behind it; starting
  unbounded would mean designing for eight hundred members on the strength of
  no evidence at all.

**Sharing**

- **FR-006**: A collection MUST NOT be reachable outside its world until its
  owner explicitly shares it.
- **FR-007**: A shared collection MUST be reachable only by possessing its link.
- **FR-008**: The share code MUST be unguessable and MUST NOT encode when it was
  created. (A v7-style identifier front-loads a timestamp, which both narrows a
  search and leaks creation time.)
- **FR-009**: Opening a collection's link MUST NOT grant any access to the world
  it came from, nor reveal that world's other content.
- **FR-009a**: A collection MUST be viewable by anyone holding its link, without
  an account (clarified 2026-09-04). It is what makes sharing with someone who
  has not joined possible — which is most of the point — and what protects the
  content is that the code is unguessable and the share is revocable, not a
  login wall.

  **Correction, recorded rather than quietly fixed.** When this was clarified it
  was justified as matching how single-artifact shares already behave. It does
  not. `sharedAbility`, `sharedItem` and `sharedActor` each call
  `authenticated_user(ctx)?` before resolving — deliberately skipping the
  *membership* check while still requiring a session. So a share link today
  reaches any signed-in user, not any user at all, and this requirement is a
  **divergence from shipped behaviour**, not an inheritance of it. The decision
  stands on its own merits above; the plan must budget for building an
  unauthenticated read path rather than reusing one.
- **FR-009e**: Aligning the three existing single-artifact shares to the same
  anonymous rule is **out of scope for this delivery** and is a follow-up. The
  argument for anonymity applies to them equally, so leaving them authenticated
  makes the product briefly inconsistent — but relaxing authentication on three
  shipped, live share paths is a security-relevant change to features this spec
  is not otherwise touching, and it deserves its own decision rather than
  arriving as a side effect of a collections build.
- **FR-009b**: **Copying still requires an account** and authority in a
  destination world (FR-016). Viewing and copying are different acts with
  different requirements, and conflating them would either lock out the
  recipients this feature exists for or let an anonymous caller write into a
  world.
- **FR-009c**: Because the view is unauthenticated, requests for collections MUST
  be rate limited per caller. Without it an unguessable code is only unguessable
  until someone is allowed to guess indefinitely.

  The existing limiter does not cover this. `rate_limit_auth_requests` keys on
  the request **path** and returns early unless the path contains
  `/authentication/`; every GraphQL operation in the product arrives at one
  path. So this is a new limiter over a GraphQL operation, not a configuration
  of the current one.
- **FR-009d**: An unauthenticated view MUST reveal nothing about the source world
  beyond the collection's own members — not its name, its other content, its
  members, nor whether a given collection code exists as distinct from being
  revoked in a way that could be probed.
- **FR-009f**: The platform MUST NOT put a world's name into an artifact's own
  name by default, because a member's title is shown in full to anonymous
  viewers and FR-009d cannot redact it (clarified 2026-09-05).

  This is not hypothetical and was found by running the end-to-end test: world
  creation names a new world's first scene **after the world**, so the very
  first scene most Game Masters own discloses their world's name the moment it
  is shared — and nothing tells them. FR-009d is satisfied by the preview, which
  sends no world field at all; the disclosure arrives through data the author is
  presumed to have chosen and did not.

  The remedy is at the source: new worlds get a neutral named starter scene
  carrying a base map instead of an empty grid named after the world. See
  Assumptions. An author who *chooses* to name a scene after their world is
  making their own decision and is not this requirement's concern.
- **FR-010**: A collection's owner MUST be able to revoke it, after which its
  link reports it as no longer available — a distinct state from a link that
  never existed and from an error.
- **FR-010a**: A collection's owner MUST be able to retrieve the active share
  code for a collection they own, so that revoking it does not depend on still
  having the browser session that created it (clarified 2026-09-05).

  **FR-020 permits this and does not conflict with it.** That requirement
  forbids browsing, searching or counting collections "beyond a user's own" —
  this is squarely a user's own, scoped to one collection they already have
  authority over, and adds no surface from which anything can be enumerated.

  Recorded because implementing FR-010 without it produced a revoke that only
  worked inside the page that minted the link: with no read path, closing the
  tab permanently removed the owner's ability to revoke. The three shipped
  single-artifact shares have the same defect today; fixing them is FR-009e's
  follow-up, not this one.
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
- **FR-015a**: An actor whose own scene was not in the collection MUST be placed
  in the destination world's **active** scene, and the displacement MUST be
  declared (clarified 2026-09-05).

  An actor requires a scene, so unlike other lost references this one cannot
  simply be reported — somewhere has to be chosen. The first implementation took
  whichever scene the database returned first, which made the same copy into the
  same world land differently on different runs. The active scene is the one its
  new owner is looking at, so a displaced actor turns up where they will see it
  rather than somewhere they must go hunting.
- **FR-016**: A recipient MUST have authority to author in the destination world
  before a copy may be made there.
- **FR-017**: Copying a collection twice MUST produce two independent sets, not
  a merge or a conflict.
- **FR-017a**: The copies MUST be owned by **the person who performed the copy**,
  in the destination world, carrying the same rights they would have over
  anything they authored there — editing, deleting, and granting as usual
  (clarified 2026-09-04). "Independent records" in FR-012 left ownership unsaid,
  which would have let a copy land owned by nobody and leave a Game Master unable
  to delete content they had just imported into their own world.
- **FR-017b**: A recipient MAY put their copies into a collection of their own.
  Re-sharing follows from ownership rather than being a separate grant. The
  alternative — marking copies as derived and refusing to share them — would have
  to survive editing to mean anything, and would not survive a recipient
  retyping the text by hand, so it would restrict the honest and inconvenience
  nobody else.
- **FR-018**: A member's image assets — a scene's background, an actor's
  portrait imagery, an item's icon — MUST travel with the copy and MUST be
  reachable from it without the copy depending on the source world continuing
  to exist (widened 2026-09-05).

  Originally written about scenes alone, which is how it was built: item icons
  were dropped with a fidelity note and actor portraits were dropped with **no
  note at all**, which FR-015 forbids outright. The narrow reading also made
  every copied actor arrive faceless, which is the most visible way a copy can
  disappoint someone. Images are shared by content rather than duplicated
  (FR-019), so carrying them costs a row and not a stored file.
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
- **SC-002a**: A collection at the 100-member limit copies within one action the
  recipient waits out, rather than requiring a background job they return to.
- **SC-003**: 100% of a collection's non-disabled members appear in the
  destination after a copy, and 0% of its disabled members do.
- **SC-003a**: Zero artifacts restricted to a subset of their world's members
  are reachable through any shared collection, verified across every artifact
  type rather than sampled.
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
- **SC-007a**: An unauthenticated caller can obtain, from any number of requests,
  nothing about a world beyond the members of collections whose codes they
  already hold.
- **SC-008**: Copying a collection containing a scene whose image the platform
  already stores adds no additional stored bytes for that image.
- **SC-008a**: A copied scene renders in the destination world with its
  background, its walls and its lighting, without the source world existing.
- **SC-009**: 90% of recipients shown a collection's preview can correctly state
  what it will add to their world before copying it.
- **SC-010**: A takedown against one member is reflected in the collection
  within the same window spec 015 commits to for the artifact itself.

## Assumptions

- **The unit changes; most of the machinery does not.** Spec 025 built the share
  code, the read-only preview, the transactional deep copy and revocation for
  single artifacts. This generalises that rather than inventing a second
  mechanism, and a plan that budgets for building sharing from scratch has
  misread what exists. Three things are genuinely new and are not generalisations
  of anything shipped: the **unauthenticated** read path (FR-009a), a
  **GraphQL-level rate limiter** (FR-009c), and **scene copying** — no code in
  this product duplicates a scene today, and a scene carries walls, lighting,
  shapes, fog and its background asset row.

- **The starter scene changes, and the change is not confined to this spec.**
  FR-009f is satisfied by world creation seeding a neutral, named starter scene
  with a base map (`examples/maps/` already ships several) rather than an empty
  100×100 grid named after the world. That touches spec 008's onboarding flow
  rather than anything here, and it stands on its own merits there — a new Game
  Master landing on something playable is a better first run than blank squares.
  It is recorded in this spec because this is where the cost of the current
  behaviour shows up. **Existing worlds are not renamed**: a scene already
  named after its world stays that way, so FR-009f binds on what the platform
  creates from now on, not retroactively.

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
