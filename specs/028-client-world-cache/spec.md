# Feature Specification: Client-Side World Cache with Content-Addressed Delta Sync

**Feature Branch**: `028-client-world-cache`

**Created**: 2026-08-26

**Status**: Draft

**Input**: User description: "Client-side scene and asset persistence with content-addressed delta sync. Give each ThunderForge client a durable, per-world local store so that reopening a world or switching scenes loads from the local machine instead of refetching everything from the server, and so that only what actually changed crosses the wire."

## Overview

Today, every time a player or GM opens a world, switches scenes, or reloads
the page, the client throws away everything it knew and fetches the whole
scene again — all scene state, and every byte of every map background and
token image. Nothing is kept on the local machine between visits. A group
that plays in the same three maps every week re-downloads those same three
maps every week, and every player pays that cost again on every reload
mid-session.

This feature gives each client a durable local store of the worlds it has
been in, identified so that many worlds can coexist and any one of them can
be cleared on its own. Each stored item carries a fingerprint of its
contents. On opening a world, the client tells the server which fingerprints
it already holds; the server replies with only the items that differ, plus
anything the client should discard. What has not changed is never sent
again.

The server remains the authority of record: whenever a client is connected,
the server decides what is true and the client reconciles to it. A client
that loses its connection may continue to work from what it holds and
reconcile when it returns — see User Story 7, which deliberately extends
this feature beyond a pure read-through cache. Clients may also fetch bytes
from each other rather than from the server, but only bytes the server has
already told them to want: content addressing means peer-supplied data is
verified against the server's fingerprint before it is trusted, so
distribution can be shared without authority being shared.

## Clarifications

### Session 2026-08-26

- Q: When the server cannot be reached, what should a client that already holds a cached world do? → A: Open fully and queue changes for later (offline authoring), rather than refusing to open or opening read-only.
- Q: Offline authoring reopens the local-first direction previously ruled out for this feature. Confirm scope? → A: Deliberately widen this feature to include offline authoring. This amends the original "read-through cache only" framing and requires a new ADR, since it modifies the server-authoritative posture recorded in ADR-046.
- Q: When two people edit the same item while both are offline, which change wins on reconnect? → A: The GM's change wins over a player's. For conflicts between two users of the same role, the tiebreak is first-to-reconnect (derived default, not explicitly chosen — see FR-040a).
- Q: How strongly must cached world content be protected on a shared machine after sign-out? → A: Encrypted at rest under a key tied to the user's session; sign-out drops the key, rendering the stored data inert immediately rather than relying on deletion completing.
- Q: How much disk should the local store be allowed to use by default? → A: A share of the quota the browser reports for this origin (proportion and absolute ceiling to be set in planning), rather than a fixed number or unlimited. Recomputed when the browser's reported quota changes.
- Q: How should cache effectiveness be observable after release? → A: A client-side diagnostics view (hit rate, bytes saved, peer vs server, repair events) for the current session. No telemetry leaves the machine; no server-side aggregation in this feature.
- Q: How should the server defend against a change attributed to one user arriving over another user's connection? → A: It should not, in the GM's case. The GM is the trusted party — the software's relationship is *with* them, and a GM acting on a player's behalf, or even overriding them outright, is their prerogative at their own table. The server verifies only that the submitter genuinely holds the GM role. The real concern is a *player* disconnecting to fabricate outcomes, and the answer there is not prevention but **disclosure**: detect the discrepancy, flag it, and tell the GM. The GM decides.
- Q: Should peer-to-peer transfer be on by default? → A: Yes, on by default. Peer-to-peer with server adjudication is the intended model. No telemetry, for now.
- Q: Which entities may be edited while disconnected? → A: Token position, rotation and scale only. Creation and deletion are refused offline, because precedence cannot resolve those conflicts without destroying work. Full offline authoring is explicitly post-MVP.
- Q: What happens when a client loses the server but still has all its peers? → A: A third state, "server-isolated". Play continues, with peer-adjudicated movement and the GM as authority, but only while the client is connected to *every* peer in the session. Losing peers as well drops to fully-offline, which reports a likely internet problem. This further amends FR-034 and must be covered by ADR-052.
- Q: Is peer-to-peer transfer between clients permitted? → A: Yes — WebRTC/peer-to-peer is allowed as a distribution transport, with the server remaining the authority. Peers may supply bytes; only the server says which bytes are current, and peer-supplied content is verified against the server's fingerprint before use.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Returning to a world I have already visited (Priority: P1)

A GM who ran a session last week opens the same world again. The maps,
token art, and scene layout they already have are read from their own
machine. The only thing that crosses the network is whatever actually
changed since they were last there — a token that moved, a map that was
replaced, a new NPC's portrait.

**Why this priority**: This is the whole point of the feature and the
largest single cost in a real session. It is also the smallest slice that
delivers value on its own: even with no other story built, repeat visits get
dramatically faster. Every other story is a refinement of, or a safety rail
around, this one.

**Independent Test**: Open a world, close it, open it again, and confirm the
second open completes substantially faster while transferring only a small
fraction of the first visit's bytes — and that what is displayed is
identical to what the server holds.

**Acceptance Scenarios**:

1. **Given** a client that has previously loaded a world and nothing about
   that world has changed since, **When** the user opens that world again,
   **Then** the scene renders with no scene state or asset bytes
   re-downloaded, and what is shown matches the server's current state
   exactly.
2. **Given** a client holding a cached world in which the GM has since
   replaced one map background, **When** the user opens that world, **Then**
   only the replaced background is transferred; all other assets and scene
   state come from the local store.
3. **Given** a client that has never seen a world before, **When** the user
   opens it, **Then** the world loads correctly by fetching everything, and
   the client retains it for next time.
4. **Given** a user with several worlds cached, **When** they open one of
   them, **Then** only that world's data is consulted and no other world's
   cached data is read, transferred, or disturbed.

---

### User Story 2 - Losing access to content I used to be able to see (Priority: P1)

A player is removed from a world, or a GM revokes their access to a
particular actor or scene. The content that player had cached locally must
stop being readable to them. Cached data must never become a back door to
material the person is no longer permitted to see.

**Why this priority**: Equal priority to Story 1 and non-negotiable
alongside it. A cache that outlives a permission grant is a disclosure bug,
and the codebase has already had to close two of those. Shipping Story 1
without this would create exactly the leak the existing access-link and
permission work was done to prevent. It must land in the same release, not a
later one.

**Independent Test**: Cache a world as a member, have the world owner revoke
that membership, then confirm the previously-cached content is no longer
retrievable or renderable on the revoked user's machine.

**Acceptance Scenarios**:

1. **Given** a user whose membership in a world has been revoked, **When**
   they next attempt to open that world, **Then** they are denied access and
   the locally-held data for that world is discarded rather than displayed.
2. **Given** a user whose permission on a specific actor or scene has been
   downgraded, **When** they open the world, **Then** the content they may
   no longer see is neither displayed from cache nor retained locally, while
   content they still have rights to continues to load from cache.
3. **Given** a user who signs out, **When** a different person signs in on
   the same machine, **Then** the second person can read none of the first
   person's cached world content.

---

### User Story 3 - Recovering from a damaged or incomplete local store (Priority: P2)

A client's local store can be truncated by the browser reclaiming space,
interrupted mid-write by a closed tab, or left inconsistent by a crash. The
client must notice this on its own and repair itself, without the user
knowing what a cache is or being told to clear one.

**Why this priority**: A cache that can silently serve wrong or partial data
is worse than no cache, because the failure looks like a product bug — a
missing map, a token with the wrong art. But it is a safety rail on Story 1
rather than a precondition for it, so it can follow immediately after.

**Independent Test**: Deliberately corrupt or partially delete a client's
stored world data, then open that world and confirm it renders correctly
without manual intervention.

**Acceptance Scenarios**:

1. **Given** a locally-stored item whose contents no longer match its
   recorded fingerprint, **When** the client loads the world, **Then** it
   discards that item, re-fetches it, and renders correctly.
2. **Given** a local store that is missing items its own index claims to
   hold, **When** the client loads the world, **Then** the missing items are
   re-fetched and the index is repaired.
3. **Given** a local store that cannot be opened or read at all, **When** the
   user opens a world, **Then** the world loads correctly by falling back to
   fetching everything, and the user sees no error.

---

### User Story 4 - Staying within the space the machine can spare (Priority: P2)

Local storage is finite and shared with everything else the browser keeps
for this site. A user who has been in many large worlds must not have their
storage silently consumed without limit, and the client must behave
predictably when space runs out or the browser reclaims it.

**Why this priority**: Without this, the feature degrades over time into an
unbounded disk consumer, and its failure mode under pressure is undefined —
which is how a cache turns into a support burden. It is a rail on Story 1,
not a precondition, so it lands alongside Story 3.

**Independent Test**: Fill a client's cache past its budget with several
worlds' data and confirm that older, less-used content is released, recently
used worlds remain fast, and nothing breaks.

**Acceptance Scenarios**:

1. **Given** a cache that has reached its budget, **When** the client needs
   room for newly-fetched content, **Then** it releases least-recently-used
   content first and the active world is never the thing evicted.
2. **Given** a write that fails because no space is available, **When** the
   client is loading a world, **Then** the world still loads correctly by
   fetching what it needs, and the failure is not surfaced as an error to
   the user.
3. **Given** a user who wants their disk space back, **When** they clear a
   specific world's stored data, **Then** only that world's data is removed
   and other worlds remain cached.

---

### User Story 5 - Seeing what is stored and taking it back (Priority: P3)

A user can see how much space ThunderForge is using on their machine, which
worlds account for it, and clear any of it.

**Why this priority**: Real, but a convenience on top of behaviour that
already works correctly without it. Story 4 guarantees the system stays
within bounds automatically; this story is about giving the user visibility
and manual control over that.

**Independent Test**: With several worlds cached, open the storage view and
confirm reported figures match what is actually held, and that clearing a
world frees the reported amount.

**Acceptance Scenarios**:

1. **Given** several cached worlds, **When** the user opens the storage
   view, **Then** they see total space used and a per-world breakdown.
2. **Given** the storage view, **When** the user clears one world, **Then**
   that world's figure drops to zero, other worlds are untouched, and the
   next visit to the cleared world still loads correctly.

---

### User Story 6 - Knowing the app is loading, not broken (Priority: P2)

The canvas engine is a substantial program that must be downloaded and
started before anything can be drawn. On a first visit, a slow connection,
or after a cache clear, that wait is long enough that a blank screen reads
as a broken page. The user must be shown that something is happening, how
far along it is, and roughly how much is left.

**Why this priority**: This is the one moment the rest of this feature
cannot help with — on a first visit there is nothing cached yet, so the wait
is unavoidable and the only thing that can improve is whether the user
understands it. It is independent of every other story here: it delivers
value even if no caching ships at all, and caching delivers value even if
this does not. P2 rather than P1 only because a returning user, the case
Story 1 optimises, spends the least time in this state.

**Independent Test**: Load the application with an empty cache on a
throttled connection and confirm the user sees continuous, accurate progress
from first paint through to an interactive canvas, and is told plainly if it
fails.

**Acceptance Scenarios**:

1. **Given** a user opening the application for the first time, **When** the
   canvas engine is being downloaded and started, **Then** they see a
   loading state that appears promptly, reports progress that advances, and
   gives way to the canvas when ready.
2. **Given** a download whose total size is known in advance, **When** it is
   in progress, **Then** the reported progress reflects actual bytes
   received rather than an animation unconnected to real work.
3. **Given** a download whose total size is not known in advance, **When**
   it is in progress, **Then** the user still sees an indication that work
   is ongoing, without a false or stalled percentage.
4. **Given** the engine has been downloaded on a previous visit, **When** the
   user returns, **Then** the loading state either does not appear or
   resolves quickly enough not to register as a wait.
5. **Given** the engine fails to download or start, **When** the failure
   occurs, **Then** the user is shown a plain explanation and a way to
   retry, rather than an indefinite loading state or a blank screen.
6. **Given** the engine is being started after download, **When** that
   startup takes perceptible time, **Then** the loading state distinguishes
   "downloading" from "starting" rather than appearing stalled at complete.

---

### User Story 7 - Playing on through a lost connection (Priority: P3)

A group is mid-session when the GM's connection drops, or a player is on
hotel wifi that comes and goes. Rather than the world becoming unusable,
each affected client keeps working from what it holds — moving tokens,
editing scenes — and the changes they made are reconciled with the server
when the connection returns.

**Why this priority**: This is the largest and riskiest story here, and the
only one that changes what the product *is* rather than how fast it loads.
It depends on Stories 1 and 3 already working (there is nothing to play from
without a populated, trustworthy local store), and it introduces a class of
problem — two people changing the same thing while neither can see the other
— that none of the other stories have. It should be built last and may
reasonably be split into its own release.

**Independent Test**: Load a world, sever the connection, make changes,
restore the connection, and confirm the changes reach the server and the
resulting state is one both parties can agree on.

**Acceptance Scenarios**:

1. **Given** a client with a populated local store, **When** the connection
   to the server is lost, **Then** the user is clearly told they are
   disconnected and may continue working with what they have.
2. **Given** a user who has made changes while disconnected, **When** the
   connection is restored, **Then** their changes are sent to the server and
   the user is told whether each was accepted.
3. **Given** two users who changed the same thing while both were
   disconnected, **When** both reconnect, **Then** the system resolves to a
   single defined outcome by a stated rule, and any change that was
   discarded is reported to the user who made it rather than silently
   dropped.
4. **Given** a user whose queued change is rejected on reconnect because
   they no longer have permission to make it, **When** reconnection
   completes, **Then** the change is discarded, the user is told why, and
   the content reverts to the server's state.
5. **Given** a user who was disconnected long enough for the server state to
   have moved substantially, **When** they reconnect, **Then** the client
   converges on the server's current state without requiring a reload.
6. **Given** a user who never reconnects, **When** they next open the
   application, **Then** their unsent changes are either still pending or
   clearly reported as lost — never silently discarded.

---

### Edge Cases

- **Two tabs, one world**: the same user has the world open in two tabs and
  both try to update the local store at once. Neither may corrupt the store
  nor serve the other a half-written item.
- **Change mid-session**: an asset is replaced by the GM while a player is
  already connected and watching. The player's cached copy must not win over
  the live update.
- **Fingerprint collision with a stale entry**: the server reports a
  fingerprint the client already holds, but under a different identifier.
  The client must not confuse the two items.
- **Clock skew and ordering**: two updates to the same item arrive out of
  order. The client must converge on whatever the server currently holds,
  not on whichever arrived last.
- **Very large single asset**: one map background is larger than the entire
  remaining budget. It must either be handled or declined cleanly, never
  half-written.
- **Private browsing / storage denied**: the browser refuses persistent
  storage entirely. Everything must still work, just without the speedup.
- **Server rollback**: content is reverted server-side to a fingerprint the
  client held two versions ago. The client must recognise it already has
  those exact bytes and not re-fetch them.
- **Shared machine**: two people use the same browser profile. Neither may
  read the other's cached world content.
- **Partial permission**: a user may see a scene but not one actor on it.
  The cache must be able to hold and serve the permitted part without the
  unpermitted part.
- **Hostile peer**: a peer supplies bytes that do not match the promised
  fingerprint, or supplies content it was never asked for. Both must be
  rejected without affecting the requesting client.
- **GM leaves while server-isolated**: the adjudicating authority
  disappears. Peer-adjudicated play must stop rather than promote another
  peer, since no one else holds that authority.
- **Peer partition**: two subsets of the session can each see themselves but
  not the other. Requiring full connectivity (FR-058) means neither may
  proceed — the state that must be tested is "both halves stop", not "one
  half wins".
- **Server returns mid-adjudication**: the server becomes reachable while a
  peer-adjudicated change is in flight. The change must not be applied twice
  nor lost between the two paths.
- **Non-GM attributed submission**: a player submits a change claiming
  another player originated it. Rejected — only a GM may submit on another's
  behalf (FR-061a).
- **Player fabricates an outcome offline**: a disconnected player reports a
  roll the server would not have produced. Not blocked — recorded, flagged,
  and shown to the GM with both values (FR-064, FR-065). The GM decides
  whether it was a bug, a sync artefact, or cheating.
- **GM overrides a player's result**: permitted by design, generates no
  flag, and is not reported to anyone (FR-061b).
- **Repeated discrepancies from one player**: still no automatic action. The
  GM sees the pattern and handles it as they would at a physical table.
- **Clock-free ordering among peers**: peers must agree on the order of
  adjudicated moves without trusting each other's clocks, for the same
  reason the conflict rule does not.
- **Peer loses permission mid-transfer**: a peer's access is revoked while
  it is serving content to another client. The transfer must not become a
  way for either party to retain what they may no longer hold.
- **Disconnected on both sides of a conflict**: a GM and a player both edit
  the same token offline, and the *player* reconnects first. GM precedence
  must still apply when the GM reconnects, meaning an already-applied player
  change may need to be superseded and its author informed.
- **Queued change against deleted content**: a user edits a token offline
  that the GM deleted server-side in the meantime. The queued change must be
  discarded with an explanation rather than resurrecting the entity.
- **Reconnect during reconnect**: the connection drops again while queued
  changes are being submitted. Partial submission must not double-apply or
  silently drop the remainder.
- **Key lost with changes pending**: the user's session ends while offline
  edits are still queued and the store is inert. Those changes must not be
  silently destroyed without the user being told.

## Requirements *(mandatory)*

### Functional Requirements

#### Local store and identity

- **FR-001**: The client MUST retain scene state and asset bytes on the
  local machine across page reloads and across browser sessions.
- **FR-002**: Stored content MUST be organised under stable identifiers for
  the world and the scene it belongs to, such that content from different
  worlds cannot collide and any single world's content can be located and
  removed independently.
- **FR-003**: The local store MUST be scoped to the authenticated user, such
  that a different user on the same machine cannot read it.
- **FR-004**: The client MUST function correctly, with no user-visible
  error, when durable local storage is unavailable or denied — falling back
  to fetching everything, as today.

#### Content addressing and delta sync

- **FR-005**: Every cacheable item MUST carry a fingerprint derived from its
  contents, such that identical contents always produce the same fingerprint
  and different contents effectively never do.
- **FR-006**: The server MUST be the authority on the current fingerprint of
  every item; a client's disagreement with the server is always resolved in
  the server's favour.
- **FR-007**: On opening a world or scene, the client MUST be able to tell
  the server which items it already holds, and the server MUST respond with
  only those items whose contents differ, together with the set of items the
  client should discard.
- **FR-008**: Items whose fingerprints match between client and server MUST
  NOT be re-transferred.
- **FR-009**: Delta sync MUST cover asset bytes — map backgrounds and token
  images — as well as scene state, since asset bytes are the majority of the
  payload.
- **FR-010**: The client MUST verify that received content matches the
  fingerprint it was promised, and MUST reject and re-request content that
  does not.

#### Authority and live updates

- **FR-011**: Cached content MUST NOT override or delay live updates
  delivered over the existing live-sync channel; a change arriving while the
  user is connected takes precedence over anything held locally.
- **FR-012**: A client's local copy MUST NOT become authoritative over the
  server's. Client-to-client communication is permitted for content
  distribution only (FR-044 to FR-050); it MUST NOT carry state, authority,
  or knowledge of what is current.
- **FR-013**: A client whose local content is stale MUST converge on the
  server's current state on its next sync, regardless of the order in which
  earlier updates arrived.

#### Permission and disclosure

- **FR-014**: Access to locally-held world content MUST be contingent on the
  user's current permission for that content, re-established on each attempt
  to open the world rather than assumed from the fact that it is cached.
- **FR-015**: When a user's access to a world, scene, actor, or asset is
  revoked or downgraded, the affected content MUST be discarded from the
  local store and MUST NOT be rendered from cache.
- **FR-016**: Locally-stored world content MUST be encrypted at rest under a
  key bound to the user's authenticated session, such that the stored bytes
  are unreadable without it.
- **FR-016a**: On sign-out, the key MUST be discarded, rendering that user's
  stored content inert immediately and independently of whether the data has
  finished being deleted.
- **FR-016b**: Reclaiming the disk space of inert content MAY happen lazily
  in the background. Failure or interruption of that reclamation MUST NOT
  make the content readable again.
- **FR-016c**: Loss of the key (sign-out, expiry, a different user) MUST be
  indistinguishable from a cold cache: the client re-fetches, and the user
  sees a slower load rather than an error.
- **FR-017**: A user MUST be able to hold and use the portion of a scene
  they are permitted to see when they are not permitted to see all of it.

#### Integrity and repair

- **FR-018**: The client MUST detect locally-held content that does not
  match its recorded fingerprint, and MUST repair itself by discarding and
  re-fetching it.
- **FR-019**: The client MUST detect and repair an inconsistency between its
  index of what it holds and what it actually holds.
- **FR-020**: Repair MUST require no user action and MUST NOT surface as an
  error; the user experiences only a slower load.
- **FR-021**: Concurrent access from more than one tab MUST NOT corrupt the
  local store nor allow a partially-written item to be read as complete.

#### Space management

- **FR-022**: The local store MUST operate within a bounded space budget,
  derived as a proportion of the storage quota the browser reports for this
  origin and subject to a stated absolute ceiling.
- **FR-022a**: The budget MUST be recomputed when the browser's reported
  quota changes, and the store MUST reduce itself to fit if the new budget
  is smaller.
- **FR-022b**: The budget MUST leave headroom within the reported quota
  rather than consuming all of it, so that this feature does not starve
  other storage the application needs.
- **FR-023**: When the budget is reached, the client MUST release
  least-recently-used content first, and MUST NOT release content belonging
  to the world currently open.
- **FR-024**: A failure to write locally MUST degrade to fetching from the
  server, never to a failed load.
- **FR-025**: Users MUST be able to see how much space is in use, broken
  down per world.
- **FR-026**: Users MUST be able to clear a single world's stored content,
  or all of it, without affecting their account or server-side data.

#### Connectivity states and peer-adjudicated play

- **FR-055**: The system MUST distinguish three connectivity states and make
  the current one visible to the user: **connected** (server reachable),
  **server-isolated** (server unreachable, every peer in the session
  reachable), and **offline** (neither).
- **FR-056**: In the **connected** state, behaviour is exactly as today: the
  server adjudicates and no peer adjudication occurs.
- **FR-057**: In the **server-isolated** state, play MAY continue for token
  movement, with changes adjudicated by peers rather than by the server.
- **FR-058**: Server-isolated play MUST require connectivity to **every**
  participant in the session. Losing any peer MUST end peer-adjudicated play
  immediately and drop the client to the offline state. Partial peer
  connectivity MUST NOT permit play, because two disjoint groups could
  otherwise both make progress and diverge irreconcilably.
- **FR-059**: During server-isolated play, the Game Master's client MUST be
  the adjudicating authority among peers. If the GM is not among the
  reachable peers, peer-adjudicated play MUST NOT proceed.
- **FR-060**: Peer-adjudicated changes MUST be confined to the same entity
  scope permitted offline — token position, rotation and scale. Creation,
  deletion, and permission changes MUST NOT be adjudicated by peers under
  any circumstances.
- **FR-061**: A peer-adjudicated change submitted on another user's behalf
  MUST be accepted only when the server independently confirms the submitter
  holds the Game Master role in that world, using the same role check that
  governs every other GM-only operation. No additional cryptographic
  attestation is required, because the GM is a trusted party by design.
- **FR-061a**: A submitter who is **not** the Game Master MUST NOT be able
  to submit a change attributed to anyone but themselves.
- **FR-061b**: The system MUST NOT attempt to prevent a Game Master from
  acting on a player's behalf, overriding a player's result, or determining
  an outcome themselves. This is a legitimate exercise of table authority,
  not an attack, and the software's role is to make running the game easier
  rather than to police the person running it.
- **FR-062**: On the server becoming reachable again, all peer-adjudicated
  changes MUST be submitted for confirmation and re-authorized against
  current permissions. The server MAY reject them, and its decision is
  final — peer adjudication is provisional, never authoritative.
- **FR-063**: In the **offline** state the user MUST be told their
  connection appears to be down, and MUST be shown that the application is
  attempting to reconnect, in the manner players already expect from online
  games.

#### Discrepancy detection and disclosure

- **FR-064**: Where a client reports an outcome the server can independently
  determine — notably dice results, which are already server-authoritative
  — the system MUST compare the two.
- **FR-065**: On a mismatch, the affected result MUST be rendered
  differently in the Game Master's view — visually distinguishable from an
  ordinary result — and MUST be inspectable to show the user, the value
  claimed, and the value the server determined.
- **FR-065a**: The system MUST NOT provide a resolution workflow,
  escalation path, dispute process, or any mechanism for acting on a
  discrepancy. It presents the information and stops. What a discrepancy
  means, and what to do about it, is the Game Master's business.
- **FR-066**: A discrepancy MUST NOT automatically reject the change, ban
  the user, interrupt play, or alter the outcome. It changes how something
  is displayed to one person and nothing else.
- **FR-067**: Discrepancy display MUST be visible to the Game Master only,
  never to other players, and MUST NOT be transmitted off the deployment
  (consistent with FR-052's no-telemetry rule).
- **FR-067a**: Detection MUST be accurate. A false positive accuses a real
  person in front of the one player who can act on it, so a discrepancy MUST
  be reported only where the server genuinely determined a different value —
  never on a timeout, a parse failure, a version mismatch, or any other
  ambiguity. When in doubt, report nothing.
- **FR-068**: Where the server has no independent basis for an outcome —
  ordinary token movement, for instance — no discrepancy exists to detect
  and none MUST be reported. Absence of evidence is not a flag.

#### Peer-assisted content distribution

- **FR-044**: Clients MAY obtain cacheable content directly from other
  clients in the same session rather than from the server, to reduce server
  bandwidth and speed up distribution when several players need the same
  large asset at once.
- **FR-045**: The server MUST remain the sole authority on which
  fingerprints are current. A peer may supply bytes; only the server says
  which bytes are the right ones.
- **FR-046**: Content received from a peer MUST be verified against the
  server-published fingerprint before being used or stored, and MUST be
  discarded if it does not match. A client MUST NOT render or persist
  unverified peer content under any circumstances.
- **FR-047**: A client MUST NOT serve a peer any content that peer is not
  independently permitted to see. Peer transfer MUST NOT become a way to
  obtain content the requester's own permissions would deny.
- **FR-048**: Peer transfer MUST be a strict optimization: if no peer is
  available, if a peer is slow, or if peer-supplied content fails
  verification, the client MUST fall back to the server and the user MUST
  see no difference in outcome.
- **FR-049**: Peer transfer is **enabled by default**. Because direct peer
  connections can reveal network address information between participants,
  users MUST be informed that peer transfer is in use and MUST be able to
  disable it, falling back to server-only transfer. Disabling it also
  forfeits server-isolated play (FR-057), and the user MUST be told so.
- **FR-050**: Peer connections MUST be confined to participants of the same
  world session and MUST NOT persist beyond it.

#### Disconnected operation and reconciliation

- **FR-035a**: Changes permitted while disconnected MUST be limited to
  token position, rotation and scale. Creation and deletion MUST be refused
  with a clear explanation, because precedence cannot resolve a
  create/delete conflict without destroying work. Broader offline authoring
  is explicitly deferred beyond this feature.
- **FR-036**: When the connection to the server is lost, the client MUST
  make the disconnected state visible to the user and MUST allow continued
  work against locally-held content.
- **FR-037**: Changes made while disconnected MUST be recorded durably, such
  that closing the browser before reconnecting does not lose them without
  the user being told.
- **FR-038**: On reconnection, the client MUST submit queued changes to the
  server and MUST report the outcome of each to the user.
- **FR-039**: The server MUST remain the authority on the accepted outcome.
  A queued change is a request, never a fact: the server MAY reject it, and
  the client MUST accept that rejection.
- **FR-040**: Where two clients changed the same thing while disconnected,
  the system MUST resolve to a single outcome by a stated, deterministic
  rule, and MUST NOT leave the two clients showing different results. The
  rule is: **a Game Master's change takes precedence over a player's.**
- **FR-040a**: Where conflicting offline changes come from two users of the
  same role (GM vs GM, or player vs player), the change belonging to the
  user who reconnects first MUST win. Resolution MUST NOT depend on
  client-supplied timestamps, which cannot be trusted.
- **FR-040b**: Conflict resolution MUST be evaluated server-side. A client
  MUST NOT decide that it has won a conflict.
- **FR-041**: Any queued change that is discarded during reconciliation MUST
  be reported to the user who made it. Silent loss of user work is not
  acceptable.
- **FR-042**: Queued changes MUST be re-authorized against the user's
  permissions at the time of reconnection, not the time they were made. A
  change the user is no longer permitted to make MUST be rejected.
- **FR-043**: A client MUST converge on the server's current state after
  reconnection without requiring a page reload.

#### Engine load feedback

- **FR-028**: While the canvas engine is being downloaded or started, the
  application MUST display a loading state rather than a blank or
  apparently-broken screen.
- **FR-029**: The loading state MUST appear promptly enough that the user
  never sees an unexplained blank screen while work is in progress.
- **FR-030**: Where the total download size is known, reported progress MUST
  reflect actual work completed. Where it is not known, the system MUST
  indicate ongoing activity without displaying a fabricated percentage.
- **FR-031**: The loading state MUST distinguish downloading from starting,
  so that a perceptible startup phase does not appear as a stall at 100%.
- **FR-032**: If the engine fails to download or start, the user MUST be
  shown a plain explanation and offered a retry, and MUST NOT be left in an
  indefinite loading state.
- **FR-033**: On a return visit where the engine is already held by the
  browser, the loading state MUST NOT introduce a delay that would not
  otherwise exist.

#### Observability

- **FR-051**: The client MUST expose a diagnostics view reporting, for the
  current session: proportion of requested items served locally versus
  fetched, bytes transferred versus bytes avoided, how much content came
  from a peer versus the server, and any integrity repairs performed.
- **FR-052**: Diagnostic information MUST remain on the user's machine. This
  feature MUST NOT transmit cache statistics or usage telemetry to the
  server or to any third party.
- **FR-053**: The diagnostics view MUST be sufficient to verify the stated
  performance outcomes against a real session, not only against test
  fixtures.

#### Explicitly out of scope

- **FR-054**: This feature MUST NOT introduce usage telemetry or
  server-side aggregation of client cache behaviour. Observability here is
  local to the user (FR-052).
- **FR-034**: Peer-to-peer is permitted for content *distribution* at all
  times (FR-044 to FR-050), and additionally for *movement adjudication*
  while a client is server-isolated (FR-055 to FR-063). Outside the
  server-isolated state, a client MUST NOT learn the world's current state
  from a peer.
- **FR-035**: This feature MUST NOT be held responsible for reducing the
  size of the engine program itself. Build-profile and code-splitting work
  is separate; what is in scope here is only how the wait is presented.

### Key Entities

- **Cached World**: the unit of organisation and of clearing. Identified by
  the world it mirrors and the user who holds it. Knows what it contains and
  when it was last used.
- **Cached Item**: one addressable thing the client holds — a scene's state,
  a map background, a token image. Carries its identifier, its content
  fingerprint, its size, and when it was last read.
- **Item Manifest**: the client's account of what it currently holds for a
  world — identifiers paired with fingerprints. This is what the client
  offers to the server to begin a delta sync.
- **Sync Response**: the server's answer — the items whose contents the
  client must take, and the items it must discard. Silence about an item
  means the client's copy is current.
- **Space Budget**: the bound on total local usage, and the accounting that
  decides what is released when the bound is reached.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Reopening a previously-visited, unchanged world transfers at
  most 5% of the bytes the first visit transferred.
- **SC-002**: Reopening a previously-visited, unchanged scene reaches an
  interactive canvas at least 3x faster than the same scene loads today.
- **SC-003**: When a single asset has changed in an otherwise-unchanged
  world, the bytes transferred are within 10% of that one asset's size.
- **SC-004**: 100% of revocation cases — world membership, scene access,
  actor permission, sign-out — result in the affected content being
  unreadable from the local store, verified by automated test.
- **SC-004a**: After sign-out, locally-stored content is unreadable
  immediately, verified by automated test that inspects the store directly
  rather than through the application, and that asserts unreadability before
  any background cleanup has run.
- **SC-005**: A client whose local store has been deliberately corrupted,
  truncated, or partially deleted loads the world correctly, with no user
  action and no user-visible error, in 100% of tested cases.
- **SC-006**: Local storage stays within its computed budget across a
  session that visits more worlds than the budget can hold, on machines
  whose reported quota differs by at least an order of magnitude.
- **SC-007**: No test in the suite demonstrates a client rendering content
  that differs from what the server currently holds.
- **SC-008**: Switching between two recently-visited scenes in the same
  world is perceived as immediate, with no visible loading state.
- **SC-009**: On a first visit, a loading state is visible within 1 second
  of navigation, and no blank or unexplained screen is shown at any point
  before the canvas is interactive.
- **SC-010**: Reported progress never moves backwards, never stalls at a
  fixed value for more than 5 seconds while work is ongoing, and never
  reaches its maximum before the canvas is actually interactive.
- **SC-011**: 100% of engine download and startup failures produce an
  explanatory message and a retry affordance, verified by automated test
  against simulated failures.
- **SC-012**: 100% of content obtained from a peer is verified against the
  server-published fingerprint before use, and content failing verification
  is never rendered or stored, verified by automated test including
  deliberately corrupted peer responses.
- **SC-013**: With peer transfer disabled, or with no peers available, every
  outcome is identical to server-only operation; only timing differs.
- **SC-014**: No user can obtain content from a peer that the server would
  have denied them, verified by automated test against a peer holding
  content the requester lacks permission for.
- **SC-015**: 100% of changes made while disconnected are either applied on
  reconnection or reported to the user as rejected; none are silently lost.
- **SC-016**: Two clients that made conflicting offline edits to the same
  item converge on the same final state after both reconnect, in 100% of
  tested cases.
- **SC-019**: A client that loses the server while retaining every peer
  continues play without interruption, and the state it reaches is accepted
  by the server on reconnection in 100% of tested cases where permissions
  are unchanged.
- **SC-020**: A client that loses any peer while server-isolated stops
  peer-adjudicated play immediately, in 100% of tested cases — verified
  including the case where the lost peer is the Game Master.
- **SC-021**: No change attributed to one user is accepted from a submitter
  who does not hold the Game Master role, verified by automated test against
  a non-GM attempting attributed submission.
- **SC-021a**: 100% of outcomes the server independently determined and that
  a client reported differently are rendered distinctly in the Game Master's
  view, with both values inspectable, verified by automated test.
- **SC-021b**: No discrepancy display interrupts play, rejects a change,
  alters an outcome, or is visible to any player other than the Game Master.
- **SC-021c**: Zero false positives across the ambiguity cases — timeout,
  parse failure, version mismatch, missing server determination — verified by
  automated test that each produces no discrepancy rather than a spurious
  one.
- **SC-022**: In every disconnection scenario the user is told which state
  they are in within 5 seconds, and reconnection attempts are visible.
- **SC-017**: The performance outcomes SC-001 through SC-003 can be
  confirmed from the diagnostics view during an ordinary session, without
  developer tooling or a test harness.
- **SC-018**: No network request carrying cache statistics or usage
  telemetry leaves the client, verified by automated test.

## Assumptions

- **Server-authoritative is preserved in substance, amended in form.** The
  server remains the authority of record on state and on which fingerprints
  are current. Two things now happen off the server that did not before:
  clients may keep working while disconnected and reconcile later (User
  Story 7), and clients may fetch bytes from each other (FR-044 to FR-050).
  Neither transfers authority. Both, however, amend the posture recorded in
  ADR-046, and **this feature requires a new ADR** stating the amended model
  before implementation begins — Constitution Principle IV makes that a
  precondition, not a follow-up.
- **Authority is separable from distribution.** The reason peer transfer is
  safe here is content addressing: a peer supplies bytes, the server
  supplies the fingerprint those bytes must match, and unverified bytes are
  discarded. This is what distinguishes the design from peer-to-peer state
  replication, which is not adopted.
- **Conflict resolution follows table authority.** Offline authoring makes
  concurrent conflicting edits possible for the first time. The chosen rule
  is GM-over-player (FR-040), which mirrors how authority already works at a
  real table and is easy to explain to the person who loses. The same-role
  tiebreak (FR-040a) is first-to-reconnect: it was derived rather than
  explicitly chosen, and is worth confirming during planning. Timestamps
  were considered and rejected — client clocks are wrong often enough, and
  forgeable enough, that a skewed clock would silently overwrite other
  people's work.
- **Which entities may be edited offline at all is open.** GM-over-player
  says who wins a conflict, not what is permitted to conflict. Whether
  structural changes (creating scenes, deleting tokens) are editable offline
  or refused outright is a planning decision.
- **Existing asset storage is reused.** Server-side assets continue to live
  where they live today; this feature adds fingerprints and a delta protocol
  in front of them rather than relocating them.
- **Fingerprints are computed server-side.** The server produces and
  publishes the authoritative fingerprint for each item. Clients verify
  against it but do not define it. Whether fingerprints are computed on
  write or on demand is a design decision for planning.
- **Where the cache lives is a design decision, not a given.** The client is
  split between a web application and a separately-compiled engine. Which
  side owns the local store, and how it is reached from the other, is
  deliberately left to planning; the requirements above are written to be
  satisfiable from either side.
- **Whether local state needs a relational store is open.** Whether the
  index of held items warrants a full local database or something
  substantially lighter is a planning decision, to be made against the
  measured cost of each.
- **Engine size and engine load feedback are separated deliberately.** The
  client's own program download is a large and known cost, but it is not
  world content. Reducing it — release build profile, compression,
  code-splitting — is out of scope. Presenting the wait honestly (User Story
  6) is in scope, because that wait is exactly the case caching cannot
  improve: on a first visit there is nothing cached yet. The current figure
  cited internally comes from an unoptimised development build and should
  not be treated as the shipping size; the loading experience must be
  correct regardless of what that size turns out to be, which is why it is
  specified independently of it.
- **Engine load progress must not be conflated with world load progress.**
  They are distinct waits with distinct causes, and measuring this feature's
  caching outcomes (SC-001 through SC-003) must exclude engine download
  time or the numbers become meaningless.
- **Single-user-per-session.** Each browser session is one authenticated
  user at a time; concurrent multi-user sessions in one browser profile are
  not supported and are not a goal.
- **The Game Master is a trusted party, not a threat.** This is the
  assumption the whole adjudication design rests on. The software's
  relationship is with the person running the game; a GM who acts on a
  player's behalf, overrides a result, or simply decides an outcome is
  exercising the authority the role already carries at any table. Building
  defences against them would add real complexity to prevent something that
  is not a wrong. Notably this removes the need for per-user cryptographic
  attestation of adjudicated changes: verifying the submitter holds the GM
  role is sufficient, and reuses authorization the codebase already has.
- **The software informs; the Game Master decides.** Where a player's client
  reports something the server can check and disagrees with — a dice result
  above all, already server-authoritative under ADR-044 — the response is to
  show it differently in the GM's view and let them dig in. Not to block,
  punish, interrupt, or escalate.
- **A discrepancy is a social problem, not a technical one — provided the
  detection is right.** Once the GM can see that a claimed value and a
  determined value differ, the question of what that means belongs to the
  table: it could be a stale client, an artefact of a reconnect, a bug of
  ours, or someone fudging a roll. Distinguishing those requires knowing the
  people involved, which the software never will and the GM already does.
  Building a dispute workflow would be inventing a technical answer to a
  human question, and would get it wrong in exactly the cases that matter.
  Our obligation stops at detecting correctly and displaying honestly — and
  because a false positive would put an innocent player under suspicion,
  detecting correctly is the part that has to be right (FR-067a).
- **Encryption is for machine-sharing, not for hiding data from its owner.**
  The threat being addressed is a second person on the same computer reading
  the first person's world content. A determined user with their own valid
  session can of course read their own cache; that is not a threat this
  feature attempts to counter. Deletion alone was rejected because a large
  store cannot be wiped instantly and an interrupted wipe leaves readable
  bytes behind — encryption makes the data inert at the moment of sign-out
  regardless of how long cleanup takes.
- **Where the key comes from is a planning decision.** FR-016 requires the
  key be bound to the authenticated session; how it is derived, where it is
  held, and how it survives (or deliberately does not survive) a browser
  restart are design questions with real trade-offs between security and
  keeping the cache warm across restarts.
- **The budget is proportional, not a shipped constant.** Deriving it from
  the browser's reported quota avoids picking a number that is simultaneously
  too large for a low-storage laptop and absurdly small for a workstation.
  The proportion and the absolute ceiling are planning decisions; what the
  spec fixes is that the budget is computed, bounded, re-evaluated when
  quota changes, and leaves headroom for the application's other storage.
- **Observability stays local, deliberately.** Client-side diagnostics were
  chosen over server-side aggregation because this is self-hosted,
  AGPL-licensed software where operators and players reasonably expect their
  play data not to be reported anywhere. Local diagnostics answer "is the
  cache working for me" without creating a telemetry surface, an opt-out
  obligation, or a privacy disclosure. The trade-off accepted is that
  fleet-wide effectiveness across deployments will not be visible.
- **Scope is scenes and their assets.** Compendium content, system packs,
  and world-level documents are not cached by this feature, though the
  mechanism is expected to generalise later.

## Dependencies

- The existing permission and access model, which this feature must consult
  on every world open rather than bypass.
- The existing live-sync channel, which continues to deliver in-session
  changes and takes precedence over cached content.
- The existing server-side asset store and its transcode path, which becomes
  the source of the bytes and fingerprints this feature distributes.
